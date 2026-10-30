use crate::domain::ids::{DriveId, GameId};
use crate::domain::status::{
    DiagnosticStatus, GameStatus, MountStatus, SettingsLoadStatus, ThemePreference,
};
use crate::domain::units::{Bytes, DisplayName, MegabitsPerSecond};
use serde::{Deserialize, Serialize};

/// Current schema version for settings.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Cache configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Root path for the cache directory.
    pub root_path: String,
    /// Maximum cache size in bytes.
    pub budget_bytes: Bytes,
    /// Minimum free space to maintain on the cache drive in bytes.
    pub minimum_free_space_bytes: Bytes,
}

impl CacheConfig {
    pub fn new(budget: Bytes, minimum_free_space: Bytes) -> Self {
        Self {
            root_path: dirs::data_local_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("Cerberus")
                .join("cache")
                .to_string_lossy()
                .into_owned(),
            budget_bytes: budget,
            minimum_free_space_bytes: minimum_free_space,
        }
    }

    pub fn default_budget() -> Bytes {
        Bytes::new(Bytes::ONE_GIB * 20)
    }

    pub fn default_reserve() -> Bytes {
        Bytes::new(Bytes::ONE_GIB * 10)
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self::new(Self::default_budget(), Self::default_reserve())
    }
}

/// Network configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Estimated network speed in megabits per second.
    pub estimated_mbps: MegabitsPerSecond,
}

impl NetworkConfig {
    pub fn new(mbps: MegabitsPerSecond) -> Self {
        Self {
            estimated_mbps: mbps,
        }
    }

    pub fn default_estimate() -> MegabitsPerSecond {
        MegabitsPerSecond::new(100.0)
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self::new(Self::default_estimate())
    }
}

/// Streaming configuration (for future phases).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    /// Enable predictive prefetching.
    pub predictive_prefetch: bool,
    /// Prefetch window in seconds.
    pub prefetch_window_secs: u32,
    /// Maximum concurrent streams.
    pub max_concurrent_streams: u32,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            predictive_prefetch: true,
            prefetch_window_secs: 30,
            max_concurrent_streams: 3,
        }
    }
}

/// Application settings - the complete user configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Schema version for migration.
    pub schema_version: u32,
    /// Cache configuration.
    pub cache: CacheConfig,
    /// Network configuration.
    pub network: NetworkConfig,
    /// Streaming configuration (future phase).
    pub streaming: StreamingConfig,
    /// Theme preference.
    pub theme_preference: ThemePreference,
    /// First-run setup completion flag.
    pub first_run_completed: bool,
    /// Defined cloud drives (future phase).
    pub drives: Vec<DriveDefinition>,
    /// Restore drives on startup (future phase).
    pub restore_drives_on_startup: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            cache: CacheConfig::default(),
            network: NetworkConfig::default(),
            streaming: StreamingConfig::default(),
            theme_preference: ThemePreference::Dark,
            first_run_completed: false,
            drives: Vec::new(),
            restore_drives_on_startup: true,
        }
    }
}

impl AppSettings {
    /// Check if settings are using defaults (first run).
    pub fn is_first_run(&self) -> bool {
        !self.first_run_completed
    }

    /// Get cache budget in GiB for UI display.
    pub fn cache_budget_gib(&self) -> f64 {
        self.cache.budget_bytes.as_gib()
    }

    /// Get minimum free space in GiB for UI display.
    pub fn min_free_space_gib(&self) -> f64 {
        self.cache.minimum_free_space_bytes.as_gib()
    }

    /// Get network estimate in Mbps.
    pub fn network_mbps(&self) -> f64 {
        self.network.estimated_mbps.value()
    }
}

/// Represents a cloud drive definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveDefinition {
    pub id: DriveId,
    pub display_name: DisplayName,
    pub remote_name: String,
    pub remote_path: String,
    pub drive_letter: Option<String>,
    pub auto_mount: bool,
    pub permanent: bool,
}

impl DriveDefinition {
    pub fn new(
        display_name: DisplayName,
        remote_name: String,
        remote_path: String,
        drive_letter: Option<String>,
    ) -> Self {
        Self {
            id: DriveId::new(),
            display_name,
            remote_name,
            remote_path,
            drive_letter,
            auto_mount: false,
            permanent: false,
        }
    }
}

/// Represents the current state of a mounted drive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveSession {
    pub id: DriveId,
    pub display_name: DisplayName,
    pub drive_letter: Option<String>,
    pub status: MountStatus,
    /// Current cache usage (bytes cached out of budget).
    pub cached_bytes: Bytes,
    /// Total cache size (budget).
    pub cache_budget: Bytes,
    /// Root path on the local filesystem.
    pub root: String,
    /// Error message if status is Failed.
    pub error_message: Option<String>,
    /// Last error timestamp if status is Failed or Degraded.
    pub error_timestamp: Option<String>,
}

/// Represents a game entry in the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEntry {
    pub id: GameId,
    pub name: String,
    pub mount_id: DriveId,
    /// Path relative to the drive root.
    pub root_path: String,
    /// Executable path relative to the root.
    pub executable: Option<String>,
    /// Recommended execution mode.
    pub launch_mode: ExecutionMode,
    /// Status of the game.
    pub status: GameStatus,
    /// Last played timestamp.
    pub last_played: Option<String>,
    /// Launch result message.
    pub launch_result: Option<String>,
    /// ID of the system drive (for compatibility checks).
    pub system_drive: Option<String>,
}

impl GameEntry {
    pub fn new(name: String, mount_id: DriveId, root_path: String) -> Self {
        Self {
            id: GameId::new(),
            name,
            mount_id,
            root_path,
            executable: None,
            launch_mode: ExecutionMode::CloudDirect,
            status: GameStatus::Ready,
            last_played: None,
            launch_result: None,
            system_drive: None,
        }
    }
}

/// Execution modes for games.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    /// Run directly from the cloud drive (use prediction and prefetching).
    #[default]
    CloudDirect,
    /// Use a hybrid local cache for frequently accessed files.
    HybridOptimized,
    /// Cache the entire game locally before launching.
    FullLocal,
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::CloudDirect => write!(f, "Cloud Direct"),
            ExecutionMode::HybridOptimized => write!(f, "Hybrid Optimized"),
            ExecutionMode::FullLocal => write!(f, "Full Local"),
        }
    }
}

/// Diagnostics information for the application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDiagnostics {
    pub version: String,
    pub settings_load_status: SettingsLoadStatus,
    pub data_root_path: std::path::PathBuf,
    pub cache_root_path: Option<std::path::PathBuf>,
    pub settings_file_path: Option<std::path::PathBuf>,
    pub log_directory: Option<std::path::PathBuf>,
    pub rclone_available: DiagnosticStatus,
    pub winfsp_available: DiagnosticStatus,
    pub dependencies: Vec<DependencyInfo>,
}

impl Default for AppDiagnostics {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            settings_load_status: SettingsLoadStatus::Loading,
            data_root_path: std::path::PathBuf::new(),
            cache_root_path: None,
            settings_file_path: None,
            log_directory: None,
            rclone_available: DiagnosticStatus::NotChecked,
            winfsp_available: DiagnosticStatus::NotChecked,
            dependencies: Vec::new(),
        }
    }
}

/// Information about a system dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInfo {
    pub name: String,
    pub status: DiagnosticStatus,
    pub version: Option<String>,
    pub path: Option<String>,
    pub message: Option<String>,
}

/// Library diagnostics (for future phases).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryDiagnostics {
    pub games_count: usize,
    pub total_size: Bytes,
    pub cached_size: Bytes,
    pub errors: Vec<String>,
}

impl Default for LibraryDiagnostics {
    fn default() -> Self {
        Self {
            games_count: 0,
            total_size: Bytes::ZERO,
            cached_size: Bytes::ZERO,
            errors: Vec::new(),
        }
    }
}
