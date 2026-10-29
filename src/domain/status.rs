use serde::{Deserialize, Serialize};

/// Mount status for cloud drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MountStatus {
    /// Drive is mounted and accessible.
    Mounted,
    /// Drive is being mounted.
    Mounting,
    /// Drive is being unmounted.
    Unmounting,
    /// Drive is not mounted.
    #[default]
    Unmounted,
    /// Mount operation failed.
    Failed,
    /// Drive is in a degraded state (e.g., cache issues).
    Degraded,
}

impl MountStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, MountStatus::Mounted | MountStatus::Mounting)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, MountStatus::Failed | MountStatus::Degraded)
    }
}

impl std::fmt::Display for MountStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MountStatus::Mounted => write!(f, "Mounted"),
            MountStatus::Mounting => write!(f, "Mounting"),
            MountStatus::Unmounting => write!(f, "Unmounting"),
            MountStatus::Unmounted => write!(f, "Unmounted"),
            MountStatus::Failed => write!(f, "Failed"),
            MountStatus::Degraded => write!(f, "Degraded"),
        }
    }
}

/// Game status for library entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GameStatus {
    /// Game is ready to launch.
    Ready,
    /// Game files are being prepared/cached.
    Preparing,
    /// Game is currently launching.
    Launching,
    /// Game is running.
    Running,
    /// Game launch or preparation failed.
    Error,
    /// Game is not available (e.g., drive not mounted).
    #[default]
    Unavailable,
}

impl GameStatus {
    pub fn is_playable(&self) -> bool {
        matches!(self, GameStatus::Ready | GameStatus::Running)
    }
}

impl std::fmt::Display for GameStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameStatus::Ready => write!(f, "Ready"),
            GameStatus::Preparing => write!(f, "Preparing"),
            GameStatus::Launching => write!(f, "Launching"),
            GameStatus::Running => write!(f, "Running"),
            GameStatus::Error => write!(f, "Error"),
            GameStatus::Unavailable => write!(f, "Unavailable"),
        }
    }
}

/// Diagnostic status for system checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticStatus {
    /// Status has not been checked in this phase.
    #[default]
    NotChecked,
    /// Dependency is available.
    Available,
    /// Dependency is not available.
    NotAvailable,
    /// Check failed.
    Error,
}

/// Outcome of reading and validating the persisted application settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "status", content = "detail", rename_all = "snake_case")]
pub enum SettingsLoadStatus {
    /// Startup has not finished reading the settings file yet.
    #[default]
    Loading,
    /// A valid settings file for the current schema was loaded.
    Loaded,
    /// No settings file existed, so defaults were used for first run.
    FirstRunDefaults,
    /// An old schema was migrated and loaded.
    Migrated { from_version: u32 },
    /// An old schema was migrated for this run but could not be rewritten.
    MigratedNotPersisted { from_version: u32 },
    /// The settings file uses a newer schema and was preserved.
    UnsupportedSchema { version: u32 },
    /// The schema version field is invalid; the original file was preserved.
    InvalidSchema,
    /// A known migration path is not available; the original file was preserved.
    MigrationUnavailable { version: u32 },
    /// Settings were readable JSON but failed model validation.
    InvalidSettings,
    /// Corrupt settings were backed up and defaults were used.
    CorruptRecovered,
    /// Corrupt settings could not be backed up; defaults were used.
    CorruptRecoveryFailed,
    /// The file could not be read; defaults were used.
    ReadError,
}

impl std::fmt::Display for SettingsLoadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Loading => write!(f, "Loading settings"),
            Self::Loaded => write!(f, "Settings loaded"),
            Self::FirstRunDefaults => write!(f, "First run defaults (no settings file yet)"),
            Self::Migrated { from_version } => {
                write!(f, "Migrated from schema version {from_version}")
            }
            Self::MigratedNotPersisted { from_version } => write!(
                f,
                "Migrated from schema version {from_version}, but could not rewrite the file"
            ),
            Self::UnsupportedSchema { version } => write!(
                f,
                "Unsupported schema version {version}; original file preserved"
            ),
            Self::InvalidSchema => write!(f, "Invalid schema version; original file preserved"),
            Self::MigrationUnavailable { version } => write!(
                f,
                "No migration from schema version {version}; original file preserved"
            ),
            Self::InvalidSettings => write!(f, "Settings failed validation; defaults used"),
            Self::CorruptRecovered => write!(f, "Corrupt settings backed up; defaults used"),
            Self::CorruptRecoveryFailed => {
                write!(f, "Corrupt settings could not be backed up; defaults used")
            }
            Self::ReadError => write!(f, "Settings could not be read; defaults used"),
        }
    }
}

impl std::fmt::Display for DiagnosticStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiagnosticStatus::NotChecked => write!(f, "Not checked"),
            DiagnosticStatus::Available => write!(f, "Available"),
            DiagnosticStatus::NotAvailable => write!(f, "Not available"),
            DiagnosticStatus::Error => write!(f, "Error"),
        }
    }
}

/// Theme preference for UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    #[default]
    Dark,
    Light,
    System,
}

impl std::fmt::Display for ThemePreference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemePreference::Dark => write!(f, "Dark"),
            ThemePreference::Light => write!(f, "Light"),
            ThemePreference::System => write!(f, "System"),
        }
    }
}
