use crate::app::state::AppState;
use crate::domain::ids::DriveId;
use crate::domain::models::{AppDiagnostics, AppSettings, DriveDefinition, LibraryDiagnostics};
use crate::domain::status::{DiagnosticStatus, SettingsLoadStatus};
use crate::domain::ThemePreference;
use crate::error::{CerberusError, CerberusResult};
use crate::mount::MountSupervisor;
use crate::persistence::{AppPaths, SettingsLoadReport, SettingsStore};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread;
use tokio::sync::oneshot;

/// Commands for the settings worker thread.
enum SettingsCommand {
    Save {
        settings: AppSettings,
        result_tx: oneshot::Sender<CerberusResult<()>>,
    },
    Load {
        result_tx: oneshot::Sender<SettingsLoadReport>,
    },
}

/// Application service layer.
///
/// This coordinates all operations and owns the core application state.
#[derive(Clone)]
pub struct AppService {
    paths: AppPaths,
    settings_store: SettingsStore,
    state: Arc<RwLock<AppState>>,
    settings_cmd_tx: std::sync::mpsc::Sender<SettingsCommand>,
    settings_ready: Arc<(Mutex<bool>, Condvar)>,
    mount_supervisor: Arc<MountSupervisor>,
    winfsp_status: Arc<Mutex<DiagnosticStatus>>,
}

impl AppService {
    /// Create a new application service.
    pub fn new(data_root: Option<PathBuf>) -> Self {
        let paths = if let Some(root) = data_root {
            AppPaths::with_data_root(root)
        } else {
            AppPaths::new()
        };

        let settings_store = paths.settings_store();
        let state = Arc::new(RwLock::new(AppState::default()));
        let settings_ready = Arc::new((Mutex::new(false), Condvar::new()));
        let mount_supervisor = Arc::new(MountSupervisor::new());
        let winfsp_status = Arc::new(Mutex::new(DiagnosticStatus::NotChecked));

        // Channel for settings commands
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<SettingsCommand>();

        // Spawn worker thread for settings operations
        let state_clone = Arc::clone(&state);
        let ready_clone = Arc::clone(&settings_ready);
        let settings_path = paths.settings_path.clone();
        let settings_store_for_worker = SettingsStore::new(settings_path.clone());
        let mount_supervisor_clone = Arc::clone(&mount_supervisor);
        let paths_clone = paths.clone();
        let winfsp_status_clone = Arc::clone(&winfsp_status);

        std::thread::spawn(move || {
            // Initialize rclone on background thread
            let _ = mount_supervisor_clone.init_rclone();
            if let Ok(status) = crate::cloud::winfsp::WinFspChecker::check() {
                if let Ok(mut cached) = winfsp_status_clone.lock() {
                    *cached = match status {
                        crate::cloud::winfsp::WinFspStatus::Available => {
                            DiagnosticStatus::Available
                        }
                        crate::cloud::winfsp::WinFspStatus::NotFound => {
                            DiagnosticStatus::NotAvailable
                        }
                    };
                }
            }
            // Set paths for mount supervisor
            mount_supervisor_clone.set_paths(paths_clone);
            // Load settings off the UI thread, then publish them atomically with
            // the readiness flag. Views must not infer first-run state before this.
            let load_report = settings_store_for_worker.load_with_status();
            if let Ok(mut state_guard) = state_clone.write() {
                state_guard.settings = load_report.settings.clone();
                state_guard.settings_loaded = true;
                state_guard.settings_load_status = load_report.status;
            }
            let (ready_lock, ready_cv) = &*ready_clone;
            if let Ok(mut ready) = ready_lock.lock() {
                *ready = true;
                ready_cv.notify_all();
            }
            tracing::info!("Settings loaded from {}", settings_path.display());

            // Handle auto-mount if enabled
            if load_report.settings.restore_drives_on_startup {
                for drive in &load_report.settings.drives {
                    if drive.auto_mount {
                        let drive_clone = drive.clone();
                        let supervisor_clone = mount_supervisor_clone.clone();
                        thread::spawn(move || {
                            let _ = supervisor_clone.mount(drive_clone);
                        });
                    }
                }
            }

            // Process persistence commands on this worker thread.
            while let Ok(cmd) = cmd_rx.recv() {
                match cmd {
                    SettingsCommand::Save {
                        settings,
                        result_tx,
                    } => {
                        let result =
                            Self::save_settings_static(&settings_store_for_worker, &settings);
                        let _ = result_tx.send(result);
                    }
                    SettingsCommand::Load { result_tx } => {
                        let report = settings_store_for_worker.load_with_status();
                        let _ = result_tx.send(report);
                    }
                }
            }
        });

        Self {
            paths,
            settings_store,
            state,
            settings_cmd_tx: cmd_tx,
            settings_ready,
            mount_supervisor,
            winfsp_status,
        }
    }

    fn wait_for_settings_loaded(ready: &Arc<(Mutex<bool>, Condvar)>) {
        let (lock, condition) = &**ready;
        let mut loaded = lock.lock().expect("settings readiness lock poisoned");
        while !*loaded {
            loaded = condition
                .wait(loaded)
                .expect("settings readiness lock poisoned");
        }
    }

    /// Save settings statically (used by worker thread).
    fn save_settings_static(store: &SettingsStore, settings: &AppSettings) -> CerberusResult<()> {
        store.validate(settings)?;
        store.save(settings)?;
        Ok(())
    }

    /// Get the shared application state.
    pub fn state(&self) -> Arc<RwLock<AppState>> {
        Arc::clone(&self.state)
    }

    /// Get a read lock on the shared state.
    pub fn read_state(&self) -> std::sync::RwLockReadGuard<'_, AppState> {
        self.state.read().expect("Failed to lock state for reading")
    }

    /// Get a write lock on the shared state.
    pub fn write_state(&self) -> std::sync::RwLockWriteGuard<'_, AppState> {
        self.state
            .write()
            .expect("Failed to lock state for writing")
    }

    /// Get the settings store.
    pub fn settings_store(&self) -> &SettingsStore {
        &self.settings_store
    }

    /// Get application paths.
    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    /// Update settings with validation and save (blocking).
    ///
    /// This takes a snapshot of settings under the write lock, releases the lock,
    /// then validates and persists. The in-memory state is updated only on success.
    pub fn update_settings<F>(&self, updater: F) -> CerberusResult<AppSettings>
    where
        F: FnOnce(&mut AppSettings) -> CerberusResult<()> + Send + 'static,
    {
        Self::wait_for_settings_loaded(&self.settings_ready);
        // Take a snapshot of current settings under the lock
        let mut settings = {
            let state = self.write_state();
            state.settings.clone()
        };

        // Apply the updater to the snapshot
        updater(&mut settings)?;

        // Validate locally first (no I/O)
        self.settings_store.validate(&settings)?;

        // Persist on the worker thread. This method waits for completion, so UI
        // callers must use `update_settings_async` instead.
        let (result_tx, result_rx) = oneshot::channel();
        let cmd = SettingsCommand::Save {
            settings: settings.clone(),
            result_tx,
        };
        self.settings_cmd_tx
            .send(cmd)
            .map_err(|_| CerberusError::config("Settings worker thread unavailable"))?;

        // Wait for the result. This is intentionally the synchronous API.
        result_rx
            .blocking_recv()
            .map_err(|_| CerberusError::config("Settings worker thread failed"))??;

        // On success, commit to in-memory state
        {
            let mut state = self.write_state();
            state.settings = settings.clone();
            state.settings_load_status = SettingsLoadStatus::Loaded;
        }

        Ok(settings)
    }

    /// Update settings asynchronously with a callback for the result.
    /// The UI callback is never run on the eframe thread; callers should forward
    /// its result through a channel and request a repaint when it arrives.
    pub fn update_settings_async<F, C>(&self, updater: F, callback: C)
    where
        F: FnOnce(&mut AppSettings) -> CerberusResult<()> + Send + 'static,
        C: FnOnce(CerberusResult<AppSettings>) + Send + 'static,
    {
        let ready = Arc::clone(&self.settings_ready);
        let settings_store = self.settings_store.clone();
        let state = Arc::clone(&self.state);

        // tokio::task::spawn requires an entered Tokio runtime. UI callers do not
        // have one, so do blocking persistence on an ordinary worker thread.
        std::thread::spawn(move || {
            Self::wait_for_settings_loaded(&ready);
            let mut settings = match state.read() {
                Ok(guard) => guard.settings.clone(),
                Err(_) => {
                    callback(Err(CerberusError::config(
                        "Application state lock is poisoned",
                    )));
                    return;
                }
            };
            let update_result = updater(&mut settings);
            let final_result = match update_result {
                Ok(()) => {
                    if let Err(e) = settings_store.validate(&settings) {
                        Err(e)
                    } else {
                        match settings_store.save(&settings) {
                            Ok(()) => {
                                // Commit only after the durable save succeeds. Never
                                // hold the state lock across validation or file I/O.
                                match state.write() {
                                    Ok(mut guard) => {
                                        guard.settings = settings.clone();
                                        guard.settings_load_status = SettingsLoadStatus::Loaded;
                                        Ok(settings)
                                    }
                                    Err(_) => Err(CerberusError::config(
                                        "Application state lock is poisoned",
                                    )),
                                }
                            }
                            Err(e) => Err(e),
                        }
                    }
                }
                Err(e) => Err(e),
            };
            callback(final_result);
        });
    }

    /// Request settings reload from disk (non-blocking).
    pub fn reload_settings_async<C>(&self, callback: C)
    where
        C: FnOnce(AppSettings) + Send + 'static,
    {
        let settings_cmd_tx = self.settings_cmd_tx.clone();
        let state = Arc::clone(&self.state);

        std::thread::spawn(move || {
            let (result_tx, result_rx) = oneshot::channel();
            let report = settings_cmd_tx
                .send(SettingsCommand::Load { result_tx })
                .ok()
                .and_then(|()| result_rx.blocking_recv().ok());
            if let Some(report) = report {
                if let Ok(mut state_guard) = state.write() {
                    state_guard.settings = report.settings.clone();
                    state_guard.settings_loaded = true;
                    state_guard.settings_load_status = report.status;
                }
                callback(report.settings);
            }
        });
    }

    /// Get diagnostics information.
    pub fn get_diagnostics(&self) -> AppDiagnostics {
        // Note: Can't access private fields directly, so we check via mount status
        let rclone_available = match self.mount_supervisor.get_rclone_status() {
            Some(true) => DiagnosticStatus::Available,
            Some(false) => DiagnosticStatus::NotAvailable,
            None => DiagnosticStatus::NotChecked,
        };

        let winfsp_available = self
            .winfsp_status
            .lock()
            .map(|status| *status)
            .unwrap_or(DiagnosticStatus::Error);

        AppDiagnostics {
            version: env!("CARGO_PKG_VERSION").to_string(),
            settings_load_status: self.read_state().settings_load_status.clone(),
            data_root_path: self.paths.data_root.clone(),
            cache_root_path: Some(self.paths.cache_root.clone()),
            settings_file_path: Some(self.paths.settings_path.clone()),
            log_directory: Some(self.paths.log_directory.clone()),
            rclone_available,
            winfsp_available,
            dependencies: vec![],
        }
    }

    /// Get library diagnostics (not implemented in Phase 1).
    pub fn get_library_diagnostics(&self) -> LibraryDiagnostics {
        LibraryDiagnostics {
            games_count: 0,
            total_size: crate::domain::units::Bytes::ZERO,
            cached_size: crate::domain::units::Bytes::ZERO,
            errors: vec![],
        }
    }

    /// Complete first-run setup.
    pub fn complete_first_run(&self) -> CerberusResult<()> {
        self.update_settings(|settings| {
            settings.first_run_completed = true;
            Ok(())
        })?;
        tracing::info!("First run setup completed");
        Ok(())
    }

    /// Reset to first-run state (for testing/debug).
    pub fn reset_to_first_run(&self) -> CerberusResult<()> {
        self.update_settings(|settings| {
            settings.first_run_completed = false;
            settings.theme_preference = ThemePreference::Dark;
            settings.drives.clear();
            settings.cache = AppSettings::default().cache;
            settings.network = AppSettings::default().network;
            Ok(())
        })?;
        tracing::info!("Reset to first-run state");
        Ok(())
    }

    // Phase 2 - Drive Management

    /// List all saved drive definitions from settings.
    pub fn list_saved_drives(&self) -> Vec<DriveDefinition> {
        self.read_state().settings.drives.clone()
    }

    /// Add a new drive definition to settings.
    pub fn add_drive_definition(&self, definition: DriveDefinition) -> CerberusResult<()> {
        self.update_settings(|settings| {
            settings.drives.push(definition);
            Ok(())
        })
        .map(|_| ())
    }

    /// Update an existing drive definition.
    pub fn update_drive_definition(
        &self,
        id: DriveId,
        mut updated: DriveDefinition,
    ) -> CerberusResult<()> {
        updated.id = id;
        self.update_settings(move |settings| {
            if let Some(existing) = settings.drives.iter_mut().find(|d| d.id == id) {
                *existing = updated;
            }
            Ok(())
        })
        .map(|_| ())
    }

    /// Remove a drive definition from settings.
    pub fn remove_drive_definition(&self, id: DriveId) -> CerberusResult<()> {
        self.update_settings(move |settings| {
            settings.drives.retain(|d| d.id != id);
            Ok(())
        })
        .map(|_| ())
    }

    /// Update the restore drives on startup setting.
    pub fn set_restore_drives_on_startup(&self, enabled: bool) -> CerberusResult<()> {
        self.update_settings(move |settings| {
            settings.restore_drives_on_startup = enabled;
            Ok(())
        })
        .map(|_| ())
    }

    // Mount operations

    /// Initialize rclone client
    pub fn init_rclone(&self) -> CerberusResult<()> {
        self.mount_supervisor.init_rclone()
    }

    pub fn refresh_dependencies(&self) -> CerberusResult<()> {
        self.mount_supervisor.init_rclone()?;
        let status = match crate::cloud::winfsp::WinFspChecker::check() {
            Ok(crate::cloud::winfsp::WinFspStatus::Available) => DiagnosticStatus::Available,
            Ok(crate::cloud::winfsp::WinFspStatus::NotFound) => DiagnosticStatus::NotAvailable,
            Err(error) => return Err(error),
        };
        *self
            .winfsp_status
            .lock()
            .map_err(|_| CerberusError::config("Dependency status lock poisoned"))? = status;
        Ok(())
    }

    /// Mount a drive
    pub fn mount_drive(&self, definition: DriveDefinition) -> CerberusResult<()> {
        self.mount_supervisor.mount(definition)
    }

    pub fn list_rclone_remotes(&self) -> CerberusResult<Vec<crate::cloud::rclone::RemoteInfo>> {
        self.mount_supervisor.list_remotes()
    }

    pub fn test_rclone_remote(&self, name: &str, path: &str) -> CerberusResult<()> {
        self.mount_supervisor.test_remote(name, path)
    }

    pub fn launch_rclone_config(&self) -> CerberusResult<()> {
        self.mount_supervisor.launch_rclone_config()
    }

    /// Unmount a drive
    pub fn unmount_drive(&self, id: &DriveId) -> CerberusResult<()> {
        self.mount_supervisor.unmount(id)
    }

    /// Get mount status for a drive
    pub fn get_mount_status(&self, id: &DriveId) -> Option<crate::domain::status::MountStatus> {
        self.mount_supervisor.get_status(id)
    }

    /// Get all active mount sessions
    pub fn active_mount_sessions(&self) -> Vec<crate::domain::models::DriveSession> {
        self.mount_supervisor.active_sessions()
    }

    /// Shutdown all mounts
    pub fn shutdown(&self) -> CerberusResult<()> {
        self.mount_supervisor.shutdown()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::units::Bytes;
    use std::time::{Duration, Instant};
    use tempfile::tempdir;

    fn wait_until_loaded(service: &AppService) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if service.read_state().settings_loaded {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("settings worker did not finish initial load in time");
    }

    #[test]
    fn startup_publishes_saved_settings_before_marking_state_loaded() {
        let dir = tempdir().unwrap();
        let paths = AppPaths::with_data_root(dir.path());
        let store = paths.settings_store();
        let defaults = AppSettings::default();
        let saved = AppSettings {
            first_run_completed: true,
            cache: crate::domain::models::CacheConfig {
                budget_bytes: Bytes::new(42 * Bytes::ONE_GIB),
                ..defaults.cache.clone()
            },
            ..defaults
        };
        store.save(&saved).unwrap();

        let service = AppService::new(Some(dir.path().to_path_buf()));
        assert!(
            !service.read_state().settings_loaded
                || service.read_state().settings.first_run_completed
        );
        wait_until_loaded(&service);

        let state = service.read_state();
        assert!(state.settings_loaded);
        assert!(state.settings.first_run_completed);
        assert_eq!(state.settings.cache.budget_bytes, saved.cache.budget_bytes);
        assert_eq!(state.settings_load_status, SettingsLoadStatus::Loaded);
        assert_eq!(
            service.get_diagnostics().settings_load_status,
            SettingsLoadStatus::Loaded
        );
    }

    #[test]
    fn async_settings_save_reports_success_and_updates_state() {
        let dir = tempdir().unwrap();
        let service = AppService::new(Some(dir.path().to_path_buf()));
        wait_until_loaded(&service);

        let (tx, rx) = std::sync::mpsc::channel();
        service.update_settings_async(
            |settings| {
                settings.cache.budget_bytes = Bytes::new(36 * Bytes::ONE_GIB);
                settings.cache.minimum_free_space_bytes = Bytes::new(8 * Bytes::ONE_GIB);
                settings.first_run_completed = true;
                Ok(())
            },
            move |result| tx.send(result).unwrap(),
        );

        let saved = rx.recv_timeout(Duration::from_secs(3)).unwrap().unwrap();
        assert_eq!(saved.cache.budget_bytes, Bytes::new(36 * Bytes::ONE_GIB));
        assert!(service.read_state().settings.first_run_completed);
        assert_eq!(
            service.settings_store().load().cache.budget_bytes,
            saved.cache.budget_bytes
        );
    }

    #[test]
    fn async_settings_save_reports_validation_errors_without_committing() {
        let dir = tempdir().unwrap();
        let service = AppService::new(Some(dir.path().to_path_buf()));
        wait_until_loaded(&service);
        let original = service.read_state().settings.clone();

        let (tx, rx) = std::sync::mpsc::channel();
        service.update_settings_async(
            |settings| {
                settings.cache.budget_bytes = Bytes::ZERO;
                Ok(())
            },
            move |result| tx.send(result).unwrap(),
        );

        assert!(rx.recv_timeout(Duration::from_secs(3)).unwrap().is_err());
        assert_eq!(
            service.read_state().settings.cache.budget_bytes,
            original.cache.budget_bytes
        );
    }
}
