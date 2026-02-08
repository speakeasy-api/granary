use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{GranaryError, Result};
use crate::services::global_config_service;

const WORKSPACES_DIR: &str = "workspaces";
const REGISTRY_FILE: &str = "registry.json";
const DB_FILE: &str = "granary.db";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRegistry {
    pub roots: HashMap<PathBuf, String>,
    pub workspaces: HashMap<String, WorkspaceMetadata>,
}

impl WorkspaceRegistry {
    /// Load the registry from ~/.granary/workspaces/registry.json.
    /// Returns an empty registry if the file does not exist.
    pub fn load() -> Result<Self> {
        let path = Self::registry_path()?;
        if !path.exists() {
            return Ok(Self {
                roots: HashMap::new(),
                workspaces: HashMap::new(),
            });
        }
        let content = std::fs::read_to_string(&path)?;
        let registry: Self = serde_json::from_str(&content)?;
        Ok(registry)
    }

    /// Write the registry back to disk, creating parent directories as needed.
    pub fn save(&self) -> Result<()> {
        let path = Self::registry_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Path to ~/.granary/workspaces/registry.json
    pub fn registry_path() -> Result<PathBuf> {
        Ok(global_config_service::config_dir()?
            .join(WORKSPACES_DIR)
            .join(REGISTRY_FILE))
    }

    /// Path to ~/.granary/workspaces/<name>/granary.db
    pub fn workspace_db_path(name: &str) -> Result<PathBuf> {
        Ok(global_config_service::config_dir()?
            .join(WORKSPACES_DIR)
            .join(name)
            .join(DB_FILE))
    }

    /// Find the workspace for a directory by exact match or deepest ancestor.
    /// Walks up from `path` toward the root; the first match is the most specific.
    pub fn lookup_root(&self, path: &Path) -> Option<&str> {
        let mut current = Some(path);
        while let Some(dir) = current {
            if let Some(workspace) = self.roots.get(dir) {
                return Some(workspace.as_str());
            }
            current = dir.parent();
        }
        None
    }

    /// Add a directory root mapped to a workspace.
    /// Fails if the path is already registered to any workspace.
    pub fn add_root(&mut self, path: PathBuf, workspace: &str) -> Result<()> {
        if let Some(existing) = self.roots.get(&path) {
            return Err(GranaryError::Conflict(format!(
                "{} is already part of workspace \"{}\"",
                path.display(),
                existing
            )));
        }
        if !self.workspaces.contains_key(workspace) {
            return Err(GranaryError::InvalidArgument(format!(
                "Workspace \"{}\" does not exist. Create it first.",
                workspace
            )));
        }
        self.roots.insert(path, workspace.to_string());
        Ok(())
    }

    /// Remove a directory root from the registry.
    /// Returns true if the root was found and removed, false otherwise.
    pub fn remove_root(&mut self, path: &Path) -> Result<bool> {
        Ok(self.roots.remove(path).is_some())
    }

    /// Create a new workspace entry in the registry and its directory on disk.
    pub fn create_workspace(&mut self, name: &str) -> Result<()> {
        if self.workspaces.contains_key(name) {
            return Err(GranaryError::Conflict(format!(
                "Workspace \"{}\" already exists",
                name
            )));
        }

        let ws_dir = global_config_service::config_dir()?
            .join(WORKSPACES_DIR)
            .join(name);
        std::fs::create_dir_all(&ws_dir)?;

        self.workspaces.insert(
            name.to_string(),
            WorkspaceMetadata {
                created_at: chrono::Utc::now().to_rfc3339(),
            },
        );
        Ok(())
    }

    /// List all workspaces with their metadata and associated roots.
    pub fn list_workspaces(&self) -> Vec<(&str, &WorkspaceMetadata, Vec<&Path>)> {
        self.workspaces
            .iter()
            .map(|(name, meta)| {
                let roots: Vec<&Path> = self
                    .roots
                    .iter()
                    .filter(|(_, ws)| ws.as_str() == name.as_str())
                    .map(|(path, _)| path.as_path())
                    .collect();
                (name.as_str(), meta, roots)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn empty_registry() -> WorkspaceRegistry {
        WorkspaceRegistry {
            roots: HashMap::new(),
            workspaces: HashMap::new(),
        }
    }

    #[test]
    fn test_serialization_round_trip() {
        let mut registry = empty_registry();
        registry.workspaces.insert(
            "work".to_string(),
            WorkspaceMetadata {
                created_at: "2026-02-08T10:00:00Z".to_string(),
            },
        );
        registry
            .roots
            .insert(PathBuf::from("/Users/daniel/work"), "work".to_string());

        let json = serde_json::to_string_pretty(&registry).unwrap();
        let deserialized: WorkspaceRegistry = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.workspaces.len(), 1);
        assert_eq!(deserialized.roots.len(), 1);
        assert_eq!(
            deserialized.roots.get(&PathBuf::from("/Users/daniel/work")),
            Some(&"work".to_string())
        );
        assert_eq!(
            deserialized.workspaces["work"].created_at,
            "2026-02-08T10:00:00Z"
        );
    }

    #[test]
    fn test_lookup_root_exact_match() {
        let mut registry = empty_registry();
        registry.workspaces.insert(
            "work".to_string(),
            WorkspaceMetadata {
                created_at: "2026-02-08T10:00:00Z".to_string(),
            },
        );
        registry
            .roots
            .insert(PathBuf::from("/Users/daniel/work"), "work".to_string());

        assert_eq!(
            registry.lookup_root(Path::new("/Users/daniel/work")),
            Some("work")
        );
    }

    #[test]
    fn test_lookup_root_ancestor_match() {
        let mut registry = empty_registry();
        registry.workspaces.insert(
            "work".to_string(),
            WorkspaceMetadata {
                created_at: "2026-02-08T10:00:00Z".to_string(),
            },
        );
        registry
            .roots
            .insert(PathBuf::from("/Users/daniel/work"), "work".to_string());

        // Subdirectory should resolve to ancestor workspace
        assert_eq!(
            registry.lookup_root(Path::new("/Users/daniel/work/project/src")),
            Some("work")
        );
    }

    #[test]
    fn test_lookup_root_deepest_ancestor_wins() {
        let mut registry = empty_registry();
        registry.workspaces.insert(
            "work".to_string(),
            WorkspaceMetadata {
                created_at: "2026-02-08T10:00:00Z".to_string(),
            },
        );
        registry.workspaces.insert(
            "project".to_string(),
            WorkspaceMetadata {
                created_at: "2026-02-08T11:00:00Z".to_string(),
            },
        );
        registry
            .roots
            .insert(PathBuf::from("/Users/daniel/work"), "work".to_string());
        registry.roots.insert(
            PathBuf::from("/Users/daniel/work/myproject"),
            "project".to_string(),
        );

        // The deeper root should win
        assert_eq!(
            registry.lookup_root(Path::new("/Users/daniel/work/myproject/src")),
            Some("project")
        );
        // But a sibling resolves to the parent workspace
        assert_eq!(
            registry.lookup_root(Path::new("/Users/daniel/work/other")),
            Some("work")
        );
    }

    #[test]
    fn test_lookup_root_no_match() {
        let mut registry = empty_registry();
        registry.workspaces.insert(
            "work".to_string(),
            WorkspaceMetadata {
                created_at: "2026-02-08T10:00:00Z".to_string(),
            },
        );
        registry
            .roots
            .insert(PathBuf::from("/Users/daniel/work"), "work".to_string());

        assert_eq!(
            registry.lookup_root(Path::new("/Users/daniel/personal")),
            None
        );
    }

    #[test]
    fn test_add_root_success() {
        let mut registry = empty_registry();
        registry.workspaces.insert(
            "work".to_string(),
            WorkspaceMetadata {
                created_at: "2026-02-08T10:00:00Z".to_string(),
            },
        );

        let result = registry.add_root(PathBuf::from("/Users/daniel/work"), "work");
        assert!(result.is_ok());
        assert_eq!(
            registry.roots.get(&PathBuf::from("/Users/daniel/work")),
            Some(&"work".to_string())
        );
    }

    #[test]
    fn test_add_root_already_registered() {
        let mut registry = empty_registry();
        registry.workspaces.insert(
            "work".to_string(),
            WorkspaceMetadata {
                created_at: "2026-02-08T10:00:00Z".to_string(),
            },
        );
        registry
            .roots
            .insert(PathBuf::from("/Users/daniel/work"), "work".to_string());

        let result = registry.add_root(PathBuf::from("/Users/daniel/work"), "work");
        assert!(result.is_err());
    }

    #[test]
    fn test_add_root_workspace_not_found() {
        let mut registry = empty_registry();

        let result = registry.add_root(PathBuf::from("/Users/daniel/work"), "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_root_exists() {
        let mut registry = empty_registry();
        registry
            .roots
            .insert(PathBuf::from("/Users/daniel/work"), "work".to_string());

        let removed = registry
            .remove_root(Path::new("/Users/daniel/work"))
            .unwrap();
        assert!(removed);
        assert!(registry.roots.is_empty());
    }

    #[test]
    fn test_remove_root_not_found() {
        let mut registry = empty_registry();

        let removed = registry
            .remove_root(Path::new("/Users/daniel/work"))
            .unwrap();
        assert!(!removed);
    }

    #[test]
    fn test_create_workspace_duplicate() {
        let mut registry = empty_registry();
        registry.workspaces.insert(
            "work".to_string(),
            WorkspaceMetadata {
                created_at: "2026-02-08T10:00:00Z".to_string(),
            },
        );

        let result = registry.create_workspace("work");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_workspaces() {
        let mut registry = empty_registry();
        registry.workspaces.insert(
            "work".to_string(),
            WorkspaceMetadata {
                created_at: "2026-02-08T10:00:00Z".to_string(),
            },
        );
        registry.workspaces.insert(
            "personal".to_string(),
            WorkspaceMetadata {
                created_at: "2026-02-08T11:00:00Z".to_string(),
            },
        );
        registry
            .roots
            .insert(PathBuf::from("/Users/daniel/work"), "work".to_string());
        registry
            .roots
            .insert(PathBuf::from("/Users/daniel/contracts"), "work".to_string());
        registry.roots.insert(
            PathBuf::from("/Users/daniel/personal"),
            "personal".to_string(),
        );

        let list = registry.list_workspaces();
        assert_eq!(list.len(), 2);

        for (name, _meta, roots) in &list {
            match *name {
                "work" => assert_eq!(roots.len(), 2),
                "personal" => assert_eq!(roots.len(), 1),
                _ => panic!("unexpected workspace: {}", name),
            }
        }
    }

    #[test]
    fn test_registry_path() {
        let path = WorkspaceRegistry::registry_path();
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.ends_with("workspaces/registry.json"));
    }

    #[test]
    fn test_workspace_db_path() {
        let path = WorkspaceRegistry::workspace_db_path("work");
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.ends_with("workspaces/work/granary.db"));
    }
}
