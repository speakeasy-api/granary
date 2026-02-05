use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const MAX_RECENT_WORKSPACES: usize = 10;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SiloConfig {
    pub recent_workspaces: Vec<PathBuf>,
}

impl SiloConfig {
    /// Get the config file path (~/.granary/silo/recent_workspaces.json)
    fn config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|home| {
            home.join(".granary")
                .join("silo")
                .join("recent_workspaces.json")
        })
    }

    /// Load config from disk, returning default if not found
    pub fn load() -> Self {
        Self::config_path()
            .and_then(|path| std::fs::read_to_string(&path).ok())
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or_default()
    }

    /// Save config to disk
    pub fn save(&self) -> Result<(), String> {
        let path =
            Self::config_path().ok_or_else(|| "Could not determine home directory".to_string())?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let contents = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        std::fs::write(&path, contents).map_err(|e| format!("Failed to write config: {}", e))
    }

    /// Add a workspace to recent list (moves to front if already present)
    pub fn add_recent_workspace(&mut self, path: PathBuf) {
        // Remove if already present
        self.recent_workspaces.retain(|p| p != &path);

        // Add to front
        self.recent_workspaces.insert(0, path);

        // Trim to max size
        self.recent_workspaces.truncate(MAX_RECENT_WORKSPACES);
    }

    /// Get recent workspaces list
    pub fn recent_workspaces(&self) -> &[PathBuf] {
        &self.recent_workspaces
    }
}
