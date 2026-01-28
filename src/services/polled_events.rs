//! Polled event generation service for task.next and project.next events.
//!
//! Unlike regular events which are persisted to the event log, polled events
//! are generated on-demand by querying for available tasks/projects.
//! Time-gating prevents duplicate emissions when agents start processing
//! but haven't claimed the entity yet.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use sqlx::SqlitePool;

use crate::db;
use crate::error::Result;
use crate::models::Task;
use crate::models::event::{EntityType, Event, EventType};

/// State for tracking polled event emissions per entity.
/// Used to implement time-gating to prevent duplicate events.
#[derive(Debug)]
pub struct PolledEventEmitter {
    /// Map of entity_id -> last emission timestamp
    emitted_at: HashMap<String, Instant>,
    /// Cooldown duration before re-emitting for the same entity
    cooldown: Duration,
}

impl PolledEventEmitter {
    /// Create a new emitter with the given cooldown duration.
    pub fn new(cooldown_secs: i64) -> Self {
        Self {
            emitted_at: HashMap::new(),
            cooldown: Duration::from_secs(cooldown_secs as u64),
        }
    }

    /// Check if an entity is within the cooldown window.
    fn is_in_cooldown(&self, entity_id: &str) -> bool {
        if let Some(last_emitted) = self.emitted_at.get(entity_id) {
            last_emitted.elapsed() < self.cooldown
        } else {
            false
        }
    }

    /// Mark an entity as emitted now.
    fn mark_emitted(&mut self, entity_id: String) {
        self.emitted_at.insert(entity_id, Instant::now());
    }

    /// Clean up old entries that have exceeded the cooldown (memory management).
    pub fn cleanup_stale_entries(&mut self) {
        self.emitted_at
            .retain(|_, instant| instant.elapsed() < self.cooldown * 2);
    }

    /// Generate task.next events for available tasks.
    ///
    /// Queries for all available tasks (via get_all_next) and generates
    /// synthetic events for tasks not in the cooldown window.
    pub async fn poll_task_next(
        &mut self,
        workspace_pool: &SqlitePool,
        project_ids: Option<&[String]>,
    ) -> Result<Vec<Event>> {
        let tasks = db::tasks::get_all_next(workspace_pool, project_ids).await?;
        let mut events = Vec::new();

        for task in tasks {
            if !self.is_in_cooldown(&task.id) {
                events.push(self.create_synthetic_event(
                    EventType::TaskNext,
                    EntityType::Task,
                    task.id.clone(),
                    &task,
                ));
                self.mark_emitted(task.id);
            }
        }

        // Periodic cleanup of stale entries
        if !events.is_empty() || self.emitted_at.len() > 100 {
            self.cleanup_stale_entries();
        }

        Ok(events)
    }

    /// Generate project.next events for available projects.
    ///
    /// Queries for all projects with available tasks (no unmet dependencies)
    /// and generates synthetic events for projects not in the cooldown window.
    pub async fn poll_project_next(&mut self, workspace_pool: &SqlitePool) -> Result<Vec<Event>> {
        // Get projects that have available tasks
        let projects = db::projects::list_with_available_tasks(workspace_pool).await?;
        let mut events = Vec::new();

        for project in projects {
            if !self.is_in_cooldown(&project.id) {
                events.push(self.create_synthetic_project_event(EventType::ProjectNext, &project));
                self.mark_emitted(project.id);
            }
        }

        // Periodic cleanup of stale entries
        if !events.is_empty() || self.emitted_at.len() > 100 {
            self.cleanup_stale_entries();
        }

        Ok(events)
    }

    /// Create a synthetic event (not persisted to DB).
    fn create_synthetic_event(
        &self,
        event_type: EventType,
        entity_type: EntityType,
        entity_id: String,
        task: &Task,
    ) -> Event {
        let now = chrono::Utc::now();
        Event {
            id: 0, // Synthetic events don't have DB IDs
            event_type: event_type.as_str(),
            entity_type: entity_type.as_str().to_string(),
            entity_id,
            actor: None,
            session_id: None,
            payload: serde_json::json!({
                "title": task.title,
                "project_id": task.project_id,
                "priority": task.priority,
                "status": task.status,
            })
            .to_string(),
            created_at: now.to_rfc3339(),
        }
    }

    /// Create a synthetic project event.
    fn create_synthetic_project_event(
        &self,
        event_type: EventType,
        project: &crate::models::Project,
    ) -> Event {
        let now = chrono::Utc::now();
        Event {
            id: 0,
            event_type: event_type.as_str(),
            entity_type: EntityType::Project.as_str().to_string(),
            entity_id: project.id.clone(),
            actor: None,
            session_id: None,
            payload: serde_json::json!({
                "name": project.name,
                "status": project.status,
            })
            .to_string(),
            created_at: now.to_rfc3339(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_new_emitter_with_cooldown() {
        let emitter = PolledEventEmitter::new(60);
        assert_eq!(emitter.cooldown, Duration::from_secs(60));
        assert!(emitter.emitted_at.is_empty());
    }

    #[test]
    fn test_is_in_cooldown_not_emitted() {
        let emitter = PolledEventEmitter::new(60);
        assert!(!emitter.is_in_cooldown("task-1"));
    }

    #[test]
    fn test_is_in_cooldown_recently_emitted() {
        let mut emitter = PolledEventEmitter::new(60);
        emitter.mark_emitted("task-1".to_string());
        assert!(emitter.is_in_cooldown("task-1"));
    }

    #[test]
    fn test_is_in_cooldown_expired() {
        let mut emitter = PolledEventEmitter::new(1); // 1 second cooldown
        emitter.mark_emitted("task-1".to_string());
        sleep(Duration::from_millis(1100)); // Wait for cooldown to expire
        assert!(!emitter.is_in_cooldown("task-1"));
    }

    #[test]
    fn test_different_entities_tracked_separately() {
        let mut emitter = PolledEventEmitter::new(60);
        emitter.mark_emitted("task-1".to_string());

        assert!(emitter.is_in_cooldown("task-1"));
        assert!(!emitter.is_in_cooldown("task-2"));
    }

    #[test]
    fn test_cleanup_stale_entries() {
        let mut emitter = PolledEventEmitter::new(1); // 1 second cooldown
        emitter.mark_emitted("task-1".to_string());
        emitter.mark_emitted("task-2".to_string());

        // Wait for entries to become stale (2x cooldown)
        sleep(Duration::from_millis(2100));

        emitter.cleanup_stale_entries();
        assert!(emitter.emitted_at.is_empty());
    }

    #[test]
    fn test_cleanup_keeps_recent_entries() {
        let mut emitter = PolledEventEmitter::new(60);
        emitter.mark_emitted("task-1".to_string());

        emitter.cleanup_stale_entries();
        assert!(emitter.emitted_at.contains_key("task-1"));
    }
}
