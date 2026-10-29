use crate::domain::models::AppSettings;
use crate::domain::status::SettingsLoadStatus;

/// Application state (not UI-specific).
#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub settings: AppSettings,
    /// False until the background settings load has completed.
    pub settings_loaded: bool,
    pub settings_load_status: SettingsLoadStatus,
    // Future fields will be added in later phases:
    // pub drives: HashMap<DriveId, DriveSession>,
    // pub games: HashMap<GameId, GameEntry>,
}

impl AppState {
    /// Check if first run needs to be completed.
    pub fn needs_first_run(&self) -> bool {
        !self.settings.first_run_completed
    }
}
