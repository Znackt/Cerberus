use crate::domain::ids::DriveId;
use crate::domain::models::{AppSettings, DriveSession};
use crate::domain::status::SettingsLoadStatus;
use std::collections::HashMap;

/// Application state (not UI-specific).
#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub settings: AppSettings,
    /// False until the background settings load has completed.
    pub settings_loaded: bool,
    pub settings_load_status: SettingsLoadStatus,
    pub active_drives: HashMap<DriveId, DriveSession>,
}

impl AppState {
    /// Check if first run needs to be completed.
    pub fn needs_first_run(&self) -> bool {
        !self.settings.first_run_completed
    }
}
