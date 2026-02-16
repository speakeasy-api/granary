use std::collections::HashMap;

use sqlx::SqlitePool;

use crate::db;
use crate::error::Result;
use crate::models::*;

/// Search initiatives, projects, and tasks by query string
/// Uses FTS5 search_all for unified relevance ranking, then hydrates results
pub async fn search(pool: &SqlitePool, query: &str) -> Result<Vec<SearchResult>> {
    let matches = db::search::search_all(pool, query, 50).await?;

    // Group entity IDs by type
    let mut project_ids = Vec::new();
    let mut task_ids = Vec::new();
    let mut initiative_ids = Vec::new();
    let mut order: Vec<(String, String)> = Vec::new();

    for m in &matches {
        order.push((m.entity_type.clone(), m.entity_id.clone()));
        match m.entity_type.as_str() {
            "project" => project_ids.push(m.entity_id.clone()),
            "task" => task_ids.push(m.entity_id.clone()),
            "initiative" => initiative_ids.push(m.entity_id.clone()),
            _ => {}
        }
    }

    // Batch-fetch full rows and build a HashMap for O(1) lookup
    let mut results_map: HashMap<(String, String), SearchResult> = HashMap::new();

    for id in &project_ids {
        if let Some(p) = db::projects::get(pool, id).await? {
            results_map.insert(
                ("project".to_string(), id.clone()),
                SearchResult::Project {
                    id: p.id,
                    name: p.name,
                    description: p.description,
                    status: p.status,
                },
            );
        }
    }

    for id in &task_ids {
        if let Some(t) = db::tasks::get(pool, id).await? {
            results_map.insert(
                ("task".to_string(), id.clone()),
                SearchResult::Task {
                    id: t.id,
                    title: t.title,
                    description: t.description,
                    status: t.status,
                    priority: t.priority,
                    project_id: t.project_id,
                },
            );
        }
    }

    for id in &initiative_ids {
        if let Some(i) = db::initiatives::get(pool, id).await? {
            results_map.insert(
                ("initiative".to_string(), id.clone()),
                SearchResult::Initiative {
                    id: i.id,
                    name: i.name,
                    description: i.description,
                    status: i.status,
                },
            );
        }
    }

    // Reassemble in FTS5 rank order
    let results: Vec<SearchResult> = order
        .into_iter()
        .filter_map(|key| results_map.remove(&key))
        .collect();

    Ok(results)
}
