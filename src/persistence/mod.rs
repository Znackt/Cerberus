pub mod settings;

use crate::error::CerberusResult;
use anyhow::Context;
use std::path::{Path, PathBuf};

/// Application paths for Cerberus.
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_root: PathBuf,
    pub settings_path: PathBuf,
    pub cache_root: PathBuf,
    pub log_directory: PathBuf,
}

impl AppPaths {
    /// Create application paths for the current user.
    pub fn new() -> Self {
        let local_app_data =
            dirs::data_local_dir().expect("Could not determine local application data directory");

        let app_name = "Cerberus";

        let data_root = local_app_data.join(app_name);
        let settings_path = data_root.join("settings.json");
        let cache_root = data_root.join("cache");
        let log_directory = data_root.join("logs");

        Self {
            data_root,
            settings_path,
            cache_root,
            log_directory,
        }
    }

    /// Create application paths with a custom data root (for testing).
    pub fn with_data_root<P: AsRef<Path>>(data_root: P) -> Self {
        let data_root = data_root.as_ref().to_path_buf();
        let cache_root = data_root.join("cache");
        let log_directory = data_root.join("logs");

        Self {
            data_root: data_root.clone(),
            settings_path: data_root.join("settings.json"),
            cache_root,
            log_directory,
        }
    }

    /// Create a settings store for these paths.
    pub fn settings_store(&self) -> SettingsStore {
        SettingsStore::new(&self.settings_path)
    }

    /// Ensure all directories exist.
    pub fn ensure_directories(&self) -> CerberusResult<()> {
        std::fs::create_dir_all(&self.data_root).context("Failed to create data directory")?;
        std::fs::create_dir_all(&self.cache_root).context("Failed to create cache directory")?;
        std::fs::create_dir_all(&self.log_directory).context("Failed to create log directory")?;
        Ok(())
    }
}

impl Default for AppPaths {
    fn default() -> Self {
        Self::new()
    }
}

pub use settings::{SettingsLoadReport, SettingsStore};
