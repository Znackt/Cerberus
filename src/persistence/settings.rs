use crate::domain::models::{AppSettings, CURRENT_SCHEMA_VERSION};
use crate::domain::status::SettingsLoadStatus;
use crate::error::{CerberusError, CerberusResult};
use anyhow::Context;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Persistent settings store using JSON with atomic replacement.
#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
    current_version: u32,
}

/// Settings data plus the outcome that Diagnostics should report.
#[derive(Debug, Clone)]
pub struct SettingsLoadReport {
    pub settings: AppSettings,
    pub status: SettingsLoadStatus,
}

impl SettingsStore {
    /// Create a new settings store at the given path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            current_version: CURRENT_SCHEMA_VERSION,
        }
    }

    /// Load settings, discarding the status for callers that only need values.
    pub fn load(&self) -> AppSettings {
        self.load_with_status().settings
    }

    /// Load settings from disk and report whether they loaded, migrated, or
    /// required recovery/defaults.
    pub fn load_with_status(&self) -> SettingsLoadReport {
        let settings_file = match self.path.exists() {
            true => self.path.clone(),
            false => {
                tracing::info!("Settings file does not exist, using defaults");
                return Self::report(AppSettings::default(), SettingsLoadStatus::FirstRunDefaults);
            }
        };

        match std::fs::File::open(&settings_file) {
            Ok(file) => match serde_json::from_reader::<_, serde_json::Value>(file) {
                Ok(mut value) => {
                    let schema_version = match value.get("schema_version") {
                        None => 0,
                        Some(version) => match version.as_u64().and_then(|v| u32::try_from(v).ok())
                        {
                            Some(version) => version,
                            None => {
                                tracing::error!("Settings schema_version is invalid; preserving the original file");
                                return Self::report(
                                    AppSettings::default(),
                                    SettingsLoadStatus::InvalidSchema,
                                );
                            }
                        },
                    };

                    if schema_version > self.current_version {
                        tracing::error!(
                            "Settings file has unsupported schema version {} (current is {}). Preserving original file.",
                            schema_version,
                            self.current_version
                        );
                        return Self::report(
                            AppSettings::default(),
                            SettingsLoadStatus::UnsupportedSchema {
                                version: schema_version,
                            },
                        );
                    }

                    let migrated = schema_version < self.current_version;
                    if migrated {
                        tracing::info!(
                            "Migrating settings from schema version {} to {}",
                            schema_version,
                            self.current_version
                        );
                        match self.migrate_value(value, schema_version) {
                            Ok(migrated) => value = migrated,
                            Err(error) => {
                                tracing::error!(
                                    "Cannot migrate settings; preserving original file: {error}"
                                );
                                return Self::report(
                                    AppSettings::default(),
                                    SettingsLoadStatus::MigrationUnavailable {
                                        version: schema_version,
                                    },
                                );
                            }
                        }
                    }

                    match serde_json::from_value::<AppSettings>(value) {
                        Ok(settings) => {
                            if let Err(e) = self.validate(&settings) {
                                tracing::warn!("Loaded settings failed validation: {}", e);
                                tracing::info!("Using defaults due to validation failure");
                                return Self::report(
                                    AppSettings::default(),
                                    SettingsLoadStatus::InvalidSettings,
                                );
                            }
                            let status = if migrated {
                                if let Err(error) = self.save(&settings) {
                                    tracing::error!("Migrated settings loaded but could not be persisted: {error}");
                                    SettingsLoadStatus::MigratedNotPersisted {
                                        from_version: schema_version,
                                    }
                                } else {
                                    SettingsLoadStatus::Migrated {
                                        from_version: schema_version,
                                    }
                                }
                            } else {
                                SettingsLoadStatus::Loaded
                            };
                            tracing::info!("Settings loaded from {}", self.path.display());
                            Self::report(settings, status)
                        }
                        Err(error) => {
                            tracing::error!(
                                "Settings JSON does not match the supported schema: {error}"
                            );
                            let recovered = self.backup_corrupt_settings(&settings_file);
                            Self::report(
                                AppSettings::default(),
                                if recovered {
                                    SettingsLoadStatus::CorruptRecovered
                                } else {
                                    SettingsLoadStatus::CorruptRecoveryFailed
                                },
                            )
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Corrupt settings file: {}", e);
                    let recovered = self.backup_corrupt_settings(&settings_file);
                    tracing::info!("Using default settings due to corrupt settings file");
                    Self::report(
                        AppSettings::default(),
                        if recovered {
                            SettingsLoadStatus::CorruptRecovered
                        } else {
                            SettingsLoadStatus::CorruptRecoveryFailed
                        },
                    )
                }
            },
            Err(e) => {
                tracing::error!("Failed to open settings file: {}", e);
                Self::report(AppSettings::default(), SettingsLoadStatus::ReadError)
            }
        }
    }

    fn report(settings: AppSettings, status: SettingsLoadStatus) -> SettingsLoadReport {
        SettingsLoadReport { settings, status }
    }

    /// Apply each versioned JSON migration before deserializing into the current
    /// Rust model. This allows old files to omit fields added in later versions.
    fn migrate_value(
        &self,
        mut value: serde_json::Value,
        mut version: u32,
    ) -> Result<serde_json::Value, String> {
        while version < self.current_version {
            match version {
                0 => {
                    let defaults = serde_json::to_value(AppSettings::default())
                        .map_err(|error| error.to_string())?;
                    Self::fill_missing(&mut value, &defaults);
                    version = 1;
                }
                unsupported => return Err(format!("no migration from schema {unsupported}")),
            }
        }

        let object = value
            .as_object_mut()
            .ok_or_else(|| "settings root must be a JSON object".to_string())?;
        object.insert(
            "schema_version".into(),
            serde_json::Value::from(self.current_version),
        );
        Ok(value)
    }

    fn fill_missing(value: &mut serde_json::Value, defaults: &serde_json::Value) {
        if let (Some(value), Some(defaults)) = (value.as_object_mut(), defaults.as_object()) {
            for (key, default) in defaults {
                match value.get_mut(key) {
                    Some(existing) => Self::fill_missing(existing, default),
                    None => {
                        value.insert(key.clone(), default.clone());
                    }
                }
            }
        }
    }

    fn backup_corrupt_settings(&self, settings_file: &Path) -> bool {
        let backup_path = self
            .path
            .with_extension(format!("json.backup.{}", Uuid::new_v4().simple()));
        if let Err(backup_err) = std::fs::rename(settings_file, &backup_path) {
            tracing::error!("Failed to backup corrupt settings: {}", backup_err);
            false
        } else {
            tracing::info!("Backed up corrupt settings to {}", backup_path.display());
            true
        }
    }

    /// Save settings atomically to avoid corruption.
    pub fn save(&self, settings: &AppSettings) -> CerberusResult<()> {
        let path = &self.path;
        let temp_path = path.with_extension(format!("json.tmp.{}", Uuid::new_v4().simple()));

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create settings directory")?;
        }

        // Serialize with formatting for readability
        let json = serde_json::to_string_pretty(settings)?;
        let json_bytes = json.as_bytes();

        // Write to a temporary file first with explicit flush
        {
            let mut file = std::fs::File::create(&temp_path).with_context(|| {
                format!(
                    "Failed to create temporary settings file at {}",
                    temp_path.display()
                )
            })?;

            use std::io::Write;
            file.write_all(json_bytes).with_context(|| {
                format!(
                    "Failed to write temporary settings to {}",
                    temp_path.display()
                )
            })?;

            // Explicitly flush to OS buffers
            file.flush().with_context(|| {
                format!(
                    "Failed to flush temporary settings to {}",
                    temp_path.display()
                )
            })?;

            // Sync to disk to ensure durability
            file.sync_all().with_context(|| {
                format!(
                    "Failed to sync temporary settings to disk: {}",
                    temp_path.display()
                )
            })?;

            // Drop file to close it before replacement
            drop(file);
        }

        // Atomic replacement - preserve old file if replacement fails
        #[cfg(windows)]
        {
            // Try atomic replacement
            let result = Self::atomic_replace_windows(&temp_path, path);
            if let Err(e) = result {
                // Clean up temp file
                let _ = std::fs::remove_file(&temp_path);
                // Preserve old valid file - don't overwrite it
                tracing::error!("Failed to commit settings, preserving old file: {}", e);
                return Err(CerberusError::Persistence(format!(
                    "Failed to commit settings: {}",
                    e
                )));
            }
        }

        #[cfg(not(windows))]
        {
            // On Unix, rename is atomic
            if let Err(e) = std::fs::rename(&temp_path, path) {
                // Clean up temp file
                let _ = std::fs::remove_file(&temp_path);
                tracing::error!("Failed to commit settings on Unix: {}", e);
                return Err(CerberusError::Persistence(format!(
                    "Failed to commit settings: {}",
                    e
                )));
            }
        }

        tracing::info!("Saved settings to {}", path.display());
        Ok(())
    }

    /// Windows-specific atomic file replacement.
    #[cfg(windows)]
    fn atomic_replace_windows(temp_path: &Path, target_path: &Path) -> std::io::Result<()> {
        use std::os::windows::ffi::OsStrExt;
        use winapi::um::winbase::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH};

        let temp: Vec<u16> = temp_path.as_os_str().encode_wide().chain(Some(0)).collect();
        let target: Vec<u16> = target_path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();

        if target_path.exists() {
            // MOVEFILE_REPLACE_EXISTING performs the same-volume replacement
            // without deleting the previous settings file first.
            let ok = unsafe {
                MoveFileExW(
                    temp.as_ptr(),
                    target.as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            };
            if ok == 0 {
                return Err(std::io::Error::last_os_error());
            }
        } else {
            // First save: move the fully flushed temp file into place.
            let ok = unsafe { MoveFileExW(temp.as_ptr(), target.as_ptr(), MOVEFILE_WRITE_THROUGH) };
            if ok == 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(())
    }

    /// Validate settings before saving.
    pub fn validate(&self, settings: &AppSettings) -> CerberusResult<()> {
        // Schema version
        if settings.schema_version == 0 {
            return Err(CerberusError::Validation(
                "Schema version must be greater than zero".to_string(),
            ));
        }

        // Cache budget must be positive
        if settings.cache.budget_bytes.0 == 0 {
            return Err(CerberusError::Validation(
                "Cache budget must be greater than zero".to_string(),
            ));
        }

        // Minimum free space must be positive
        if settings.cache.minimum_free_space_bytes.0 == 0 {
            return Err(CerberusError::Validation(
                "Minimum free space must be greater than zero".to_string(),
            ));
        }

        // Minimum free space cannot exceed budget (would leave no room for cache)
        if settings.cache.minimum_free_space_bytes > settings.cache.budget_bytes {
            return Err(CerberusError::Validation(
                "Minimum free space cannot exceed cache budget".to_string(),
            ));
        }

        // Cache root path must not be empty
        if settings.cache.root_path.trim().is_empty() {
            return Err(CerberusError::Validation(
                "Cache folder path cannot be empty".to_string(),
            ));
        }

        // Network estimate must be positive and finite
        if settings.network.estimated_mbps.0 <= 0.0 {
            return Err(CerberusError::Validation(
                "Network estimate must be greater than zero".to_string(),
            ));
        }
        if !settings.network.estimated_mbps.0.is_finite() {
            return Err(CerberusError::Validation(
                "Network estimate must be a finite number".to_string(),
            ));
        }

        Ok(())
    }

    /// Check if the settings file exists and is readable.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Get the settings file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::units::Bytes;
    use tempfile::NamedTempFile;

    #[test]
    fn test_roundtrip() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("json");
        // Keep file alive until end of test
        let _file = file;

        let store = SettingsStore::new(&path);
        let original = AppSettings::default();

        // Save
        store.save(&original).unwrap();

        // Load
        let report = store.load_with_status();
        let loaded = report.settings;
        assert_eq!(report.status, SettingsLoadStatus::Loaded);
        assert_eq!(loaded.schema_version, original.schema_version);
        assert_eq!(loaded.cache.budget_bytes, original.cache.budget_bytes);
        assert_eq!(
            loaded.network.estimated_mbps,
            original.network.estimated_mbps
        );
    }

    #[test]
    fn test_corruption_recovery() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().with_extension("json");
        let _file = file;

        // Write invalid JSON
        std::fs::write(&path, "{ invalid json }").unwrap();

        let store = SettingsStore::new(&path);
        let report = store.load_with_status();
        let loaded = report.settings;
        assert_eq!(report.status, SettingsLoadStatus::CorruptRecovered);

        // Should have returned defaults without panicking
        assert_eq!(loaded.schema_version, 1);
    }

    #[test]
    fn test_validation() {
        let store = SettingsStore::new("test.json");

        // Valid settings
        let valid = AppSettings::default();
        assert!(store.validate(&valid).is_ok());

        // Invalid: zero budget
        let mut invalid = AppSettings::default();
        invalid.cache.budget_bytes = crate::domain::units::Bytes::ZERO;
        assert!(store.validate(&invalid).is_err());

        // Invalid: reserve exceeds budget
        let mut invalid = AppSettings::default();
        invalid.cache.minimum_free_space_bytes =
            invalid.cache.budget_bytes + Bytes::new(Bytes::ONE_GIB);
        assert!(store.validate(&invalid).is_err());

        // Invalid: empty cache path
        let mut invalid = AppSettings::default();
        invalid.cache.root_path = "".to_string();
        assert!(store.validate(&invalid).is_err());

        // Invalid: zero network
        let mut invalid = AppSettings::default();
        invalid.network.estimated_mbps = crate::domain::units::MegabitsPerSecond::ZERO;
        assert!(store.validate(&invalid).is_err());
    }

    #[test]
    fn test_migrates_old_json_with_missing_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut old = serde_json::to_value(AppSettings::default()).unwrap();
        old["schema_version"] = serde_json::Value::from(0);
        old["cache"]
            .as_object_mut()
            .unwrap()
            .remove("minimum_free_space_bytes");
        old.as_object_mut().unwrap().remove("streaming");
        old.as_object_mut()
            .unwrap()
            .remove("restore_drives_on_startup");
        std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();

        let report = SettingsStore::new(&path).load_with_status();
        assert_eq!(
            report.status,
            SettingsLoadStatus::Migrated { from_version: 0 }
        );
        let migrated = report.settings;
        assert_eq!(migrated.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(
            migrated.cache.budget_bytes,
            AppSettings::default().cache.budget_bytes
        );
        assert_eq!(
            migrated.cache.minimum_free_space_bytes,
            AppSettings::default().cache.minimum_free_space_bytes
        );
        assert_eq!(
            migrated.streaming.max_concurrent_streams,
            AppSettings::default().streaming.max_concurrent_streams
        );
        assert!(migrated.restore_drives_on_startup);
        let persisted: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted["schema_version"], CURRENT_SCHEMA_VERSION);
        assert!(persisted.get("streaming").is_some());
    }

    #[test]
    fn test_future_schema_is_preserved_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut future = serde_json::to_value(AppSettings::default()).unwrap();
        future["schema_version"] = serde_json::Value::from(CURRENT_SCHEMA_VERSION + 1);
        future["future_option"] = serde_json::Value::String("keep-me".into());
        let bytes = serde_json::to_vec_pretty(&future).unwrap();
        std::fs::write(&path, &bytes).unwrap();

        let report = SettingsStore::new(&path).load_with_status();
        assert_eq!(
            report.status,
            SettingsLoadStatus::UnsupportedSchema {
                version: CURRENT_SCHEMA_VERSION + 1
            }
        );
        let loaded = report.settings;
        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(
            loaded.cache.budget_bytes,
            AppSettings::default().cache.budget_bytes
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn test_save_replaces_existing_settings_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::new(&path);
        let first = AppSettings::default();
        store.save(&first).unwrap();

        let mut second = first.clone();
        second.cache.budget_bytes = Bytes::new(44 * Bytes::ONE_GIB);
        store.save(&second).unwrap();

        let loaded = store.load();
        assert_eq!(loaded.cache.budget_bytes, second.cache.budget_bytes);
    }

    #[test]
    fn missing_settings_report_first_run_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let report = SettingsStore::new(path).load_with_status();
        assert_eq!(report.status, SettingsLoadStatus::FirstRunDefaults);
        assert!(!report.settings.first_run_completed);
    }

    #[test]
    fn invalid_schema_version_is_reported_and_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let bytes = br#"{"schema_version":"future"}"#;
        std::fs::write(&path, bytes).unwrap();
        let report = SettingsStore::new(&path).load_with_status();
        assert_eq!(report.status, SettingsLoadStatus::InvalidSchema);
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}
