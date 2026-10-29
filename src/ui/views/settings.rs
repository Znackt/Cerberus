use crate::app::service::AppService;
use crate::domain::models::AppSettings;
use crate::domain::units::{Bytes, MegabitsPerSecond};
use crate::domain::ThemePreference;
use crate::error::{CerberusError, CerberusResult};
use egui::{Color32, ComboBox, Ui};
use std::sync::mpsc;

/// Settings view.
pub struct SettingsView {
    app_service: AppService,
    // UI state for first-run
    show_first_run: bool,
    settings_initialized: bool,
    // Draft state for settings form (separate from live state)
    draft: SettingsDraft,
    // Save status
    save_status: String,
    save_error: Option<String>,
    is_saving: bool,
    pending_first_run_completion: bool,
    // Async result receiver
    async_result_rx: Option<mpsc::Receiver<CerberusResult<AppSettings>>>,
    repaint_ctx: Option<egui::Context>,
}

/// Draft settings state for UI editing.
#[derive(Debug, Clone)]
struct SettingsDraft {
    base: AppSettings,
    cache_root: String,
    cache_budget: Bytes,
    min_free_space: Bytes,
    network_speed: MegabitsPerSecond,
    theme_preference: ThemePreference,
    restore_drives_on_startup: bool,
}

impl SettingsDraft {
    fn from_settings(settings: &AppSettings) -> Self {
        Self {
            base: settings.clone(),
            cache_root: settings.cache.root_path.clone(),
            cache_budget: settings.cache.budget_bytes,
            min_free_space: settings.cache.minimum_free_space_bytes,
            network_speed: settings.network.estimated_mbps,
            theme_preference: settings.theme_preference,
            restore_drives_on_startup: settings.restore_drives_on_startup,
        }
    }

    fn to_settings(&self) -> AppSettings {
        // Start from the loaded model so editing Phase 1 preferences does not
        // reset first-run state or discard unrelated/future persisted fields.
        let mut settings = self.base.clone();
        settings.cache.root_path = self.cache_root.clone();
        settings.cache.budget_bytes = self.cache_budget;
        settings.cache.minimum_free_space_bytes = self.min_free_space;
        settings.network.estimated_mbps = self.network_speed;
        settings.theme_preference = self.theme_preference;
        settings.restore_drives_on_startup = self.restore_drives_on_startup;
        settings
    }
}

impl SettingsView {
    /// Create a new settings view.
    pub fn new(app_service: &AppService) -> Self {
        let (settings, settings_initialized) = {
            let state = app_service.read_state();
            (state.settings.clone(), state.settings_loaded)
        };
        Self {
            app_service: app_service.clone(),
            // Until the background loader publishes persisted settings, do not
            // assume this is a first run or expose editable defaults.
            show_first_run: settings_initialized && !settings.first_run_completed,
            settings_initialized,
            draft: SettingsDraft::from_settings(&settings),
            save_status: String::new(),
            save_error: None,
            is_saving: false,
            pending_first_run_completion: false,
            async_result_rx: None,
            repaint_ctx: None,
        }
    }

    /// Show the settings view.
    pub fn show(&mut self, ui: &mut Ui) {
        self.repaint_ctx = Some(ui.ctx().clone());
        // Check for async results
        self.poll_async_result();

        if !self.settings_initialized {
            let loaded_settings = {
                let state = self.app_service.read_state();
                state.settings_loaded.then(|| state.settings.clone())
            };
            if let Some(settings) = loaded_settings {
                self.show_first_run = !settings.first_run_completed;
                self.draft = SettingsDraft::from_settings(&settings);
                self.settings_initialized = true;
            } else {
                ui.heading("Loading saved preferences");
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Cerberus is loading your settings…");
                });
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(50));
                return;
            }
        }

        ui.heading("Settings");
        ui.label("These preferences are saved locally for future Cerberus features. No drive is mounted in this phase.");
        ui.add_space(14.0);

        // First-run setup panel
        if self.show_first_run {
            self.show_first_run_panel(ui);
            return;
        }

        egui::Grid::new("settings_grid")
            .num_columns(2)
            .spacing([18.0, 14.0])
            .show(ui, |ui| {
                // Cache folder
                ui.label("Cache folder");
                if ui
                    .text_edit_singleline(&mut self.draft.cache_root)
                    .changed()
                {
                    // Draft updated, will be saved on Save click
                }
                ui.end_row();

                // Cache budget
                ui.label("Cache budget (GiB)");
                let mut budget_gib = self.draft.cache_budget.as_gib();
                if ui
                    .add(
                        egui::DragValue::new(&mut budget_gib)
                            .speed(0.5)
                            .clamp_range(1.0..=4096.0),
                    )
                    .changed()
                {
                    self.draft.cache_budget = Bytes::new((budget_gib * 1024_f64.powi(3)) as u64);
                }
                ui.end_row();

                // Minimum free space
                ui.label("Minimum free space (GiB)");
                let mut reserve_gib = self.draft.min_free_space.as_gib();
                if ui
                    .add(
                        egui::DragValue::new(&mut reserve_gib)
                            .speed(0.5)
                            .clamp_range(1.0..=4096.0),
                    )
                    .changed()
                {
                    self.draft.min_free_space = Bytes::new((reserve_gib * 1024_f64.powi(3)) as u64);
                }
                ui.end_row();

                // Network speed
                ui.label("Estimated connection (Mbps)");
                let mut mbps = self.draft.network_speed.value();
                if ui
                    .add(
                        egui::DragValue::new(&mut mbps)
                            .speed(1.0)
                            .clamp_range(1.0..=100000.0),
                    )
                    .changed()
                {
                    self.draft.network_speed = MegabitsPerSecond::new(mbps);
                }
                ui.end_row();

                // Restore drives (future phase)
                ui.label("Restore drives on startup (later phase)");
                if ui
                    .checkbox(&mut self.draft.restore_drives_on_startup, "Enabled")
                    .changed()
                {
                    // Will be saved on Save click
                }
                ui.end_row();

                // Theme
                ui.label("Appearance");
                ComboBox::from_id_source("theme_combo")
                    .selected_text(self.draft.theme_preference.to_string())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.draft.theme_preference,
                            ThemePreference::Dark,
                            "Dark",
                        );
                        ui.selectable_value(
                            &mut self.draft.theme_preference,
                            ThemePreference::Light,
                            "Light",
                        );
                        ui.selectable_value(
                            &mut self.draft.theme_preference,
                            ThemePreference::System,
                            "System",
                        );
                    });
                ui.end_row();
            });

        ui.add_space(16.0);

        // Save button
        ui.horizontal(|ui| {
            let save_button = ui.add_enabled(!self.is_saving, egui::Button::new("Save settings"));
            if save_button.clicked() {
                self.save_settings_async();
            }

            if self.is_saving {
                ui.spinner();
                ui.label("Saving...");
            }
        });

        // Status message
        if !self.save_status.is_empty() {
            let color = if self.save_error.is_some() {
                Color32::LIGHT_RED
            } else {
                Color32::LIGHT_GREEN
            };
            ui.colored_label(color, &self.save_status);
        }
    }

    /// Poll for async save result.
    fn poll_async_result(&mut self) {
        if let Some(rx) = self.async_result_rx.take() {
            match rx.try_recv() {
                Ok(result) => {
                    self.is_saving = false;
                    match result {
                        Ok(_) => {
                            self.save_status = "Settings saved successfully".into();
                            self.save_error = None;
                            if self.pending_first_run_completion {
                                self.show_first_run = false;
                            }
                            self.pending_first_run_completion = false;
                            // Refresh draft from live state
                            let settings = self.app_service.read_state().settings.clone();
                            self.draft = SettingsDraft::from_settings(&settings);
                        }
                        Err(e) => {
                            self.save_status = "Settings save failed".into();
                            self.save_error = Some(format!("Could not save settings: {:?}", e));
                            self.pending_first_run_completion = false;
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // Still pending, put receiver back
                    self.async_result_rx = Some(rx);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.is_saving = false;
                    self.save_status = "Settings save failed".into();
                    self.save_error = Some("Settings worker thread disconnected".into());
                    self.pending_first_run_completion = false;
                }
            }
        }
    }

    /// Show the first-run setup panel.
    fn show_first_run_panel(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.heading("Welcome to Cerberus");
            ui.add_space(10.0);
            ui.label("Set initial preferences. These values are saved for future phases; no cloud drive will be mounted now.");
            ui.add_space(10.0);
        });

        // Cache folder
        ui.label("Cache folder");
        ui.text_edit_singleline(&mut self.draft.cache_root);
        ui.add_space(5.0);

        // Cache budget
        ui.horizontal(|ui| {
            ui.label("Cache budget (GiB)");
            let mut budget_gib = self.draft.cache_budget.as_gib();
            if ui
                .add(egui::DragValue::new(&mut budget_gib).clamp_range(1.0..=4096.0))
                .changed()
            {
                self.draft.cache_budget = Bytes::new((budget_gib * 1024_f64.powi(3)) as u64);
            }
        });
        ui.add_space(5.0);

        // Free space reserve
        ui.horizontal(|ui| {
            ui.label("Free-space reserve (GiB)");
            let mut reserve_gib = self.draft.min_free_space.as_gib();
            if ui
                .add(egui::DragValue::new(&mut reserve_gib).clamp_range(1.0..=4096.0))
                .changed()
            {
                self.draft.min_free_space = Bytes::new((reserve_gib * 1024_f64.powi(3)) as u64);
            }
        });
        ui.add_space(5.0);

        // Network estimate
        ui.horizontal(|ui| {
            ui.label("Connection estimate (Mbps)");
            let mut mbps = self.draft.network_speed.value();
            ui.add(egui::DragValue::new(&mut mbps).clamp_range(1.0..=100000.0));
            self.draft.network_speed = MegabitsPerSecond::new(mbps);
        });
        ui.add_space(10.0);

        // Error display
        if let Some(error) = &self.save_error {
            ui.colored_label(Color32::LIGHT_RED, error);
            ui.add_space(10.0);
        }

        // Buttons
        ui.horizontal(|ui| {
            // Save and continue
            let save_enabled = !self.is_saving;
            if ui
                .add_enabled(save_enabled, egui::Button::new("Save and continue"))
                .clicked()
            {
                self.handle_save_and_continue();
            }

            // Use defaults
            let defaults_enabled = !self.is_saving;
            if ui
                .add_enabled(defaults_enabled, egui::Button::new("Use defaults"))
                .clicked()
            {
                self.handle_use_defaults();
            }

            // Cancel - does NOT mark first-run complete
            let cancel_enabled = !self.is_saving;
            if ui
                .add_enabled(cancel_enabled, egui::Button::new("Cancel"))
                .clicked()
            {
                self.show_first_run = false;
                // Do NOT save or mark first_run_completed
                // On next launch, incomplete setup will be offered again
            }
        });
    }

    /// Handle Save and continue button.
    fn handle_save_and_continue(&mut self) {
        // Validate draft values
        if self.draft.cache_budget.0 == 0 {
            self.save_error = Some("Cache budget must be greater than zero".into());
            return;
        }
        if self.draft.min_free_space.0 == 0 {
            self.save_error = Some("Minimum free space must be greater than zero".into());
            return;
        }
        if self.draft.min_free_space > self.draft.cache_budget {
            self.save_error = Some("Minimum free space cannot exceed cache budget".into());
            return;
        }
        if !self.draft.network_speed.is_finite() || !self.draft.network_speed.is_positive() {
            self.save_error = Some("Network estimate must be a positive number".into());
            return;
        }

        // Save settings asynchronously
        self.is_saving = true;
        self.save_error = None;
        self.save_settings_async_with_first_run(true);
    }

    /// Handle Use defaults button.
    fn handle_use_defaults(&mut self) {
        let defaults = AppSettings::default();
        self.draft = SettingsDraft::from_settings(&defaults);
        self.draft.theme_preference = defaults.theme_preference;

        // Save defaults asynchronously
        self.is_saving = true;
        self.save_error = None;
        self.save_settings_async_with_first_run(true);
    }

    /// Save regular settings (non-first-run) asynchronously.
    fn save_settings_async(&mut self) {
        self.is_saving = true;
        self.save_error = None;
        self.save_settings_async_with_first_run(false);
    }

    /// Save settings asynchronously with optional first-run completion.
    fn save_settings_async_with_first_run(&mut self, mark_first_run_complete: bool) {
        let draft = self.draft.clone();
        let mark_complete = mark_first_run_complete;
        self.pending_first_run_completion = mark_complete;
        let repaint_ctx = self.repaint_ctx.clone();

        let (tx, rx) = mpsc::channel();
        self.async_result_rx = Some(rx);

        self.app_service.update_settings_async(
            move |settings| -> Result<(), CerberusError> {
                *settings = draft.to_settings();
                if mark_complete {
                    settings.first_run_completed = true;
                }
                Ok(())
            },
            move |result| {
                let _ = tx.send(result);
                if let Some(ctx) = repaint_ctx {
                    ctx.request_repaint();
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::service::AppService;
    use std::time::{Duration, Instant};
    use tempfile::tempdir;

    fn initialize_view(view: &mut SettingsView, context: &egui::Context) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !view.settings_initialized && Instant::now() < deadline {
            let _ = context.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| view.show(ui));
            });
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            view.settings_initialized,
            "settings view did not initialize"
        );
    }

    fn wait_for_save(view: &mut SettingsView) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while view.is_saving && Instant::now() < deadline {
            view.poll_async_result();
            std::thread::sleep(Duration::from_millis(5));
        }
        view.poll_async_result();
        assert!(!view.is_saving, "settings save did not complete");
        assert!(
            view.save_error.is_none(),
            "settings save failed: {:?}",
            view.save_error
        );
    }

    #[test]
    fn saved_configuration_is_not_mistaken_for_first_run_while_loading() {
        let dir = tempdir().unwrap();
        let paths = crate::persistence::AppPaths::with_data_root(dir.path());
        let store = paths.settings_store();
        let defaults = AppSettings::default();
        let saved = AppSettings {
            first_run_completed: true,
            cache: crate::domain::models::CacheConfig {
                budget_bytes: Bytes::new(48 * Bytes::ONE_GIB),
                ..defaults.cache.clone()
            },
            ..defaults
        };
        store.save(&saved).unwrap();

        let service = AppService::new(Some(dir.path().to_path_buf()));
        let mut view = SettingsView::new(&service);
        assert!(!view.show_first_run);

        let context = egui::Context::default();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !view.settings_initialized && Instant::now() < deadline {
            let _ = context.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| view.show(ui));
            });
            std::thread::sleep(Duration::from_millis(5));
        }

        assert!(
            view.settings_initialized,
            "settings view did not observe load completion"
        );
        assert!(!view.show_first_run);
        assert_eq!(view.draft.cache_budget, saved.cache.budget_bytes);
    }

    #[test]
    fn first_run_save_edit_save_and_restart_workflow() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let context = egui::Context::default();
        let service = AppService::new(Some(root.clone()));
        let mut view = SettingsView::new(&service);
        initialize_view(&mut view, &context);
        assert!(view.show_first_run);

        // Invalid first-run values must be rejected without starting persistence.
        view.draft.cache_budget = Bytes::new(20 * Bytes::ONE_GIB);
        view.draft.min_free_space = Bytes::new(25 * Bytes::ONE_GIB);
        view.handle_save_and_continue();
        assert!(!view.is_saving);
        assert!(view
            .save_error
            .as_deref()
            .unwrap()
            .contains("cannot exceed"));

        // Save valid first-run preferences and verify setup completes.
        view.draft.cache_budget = Bytes::new(32 * Bytes::ONE_GIB);
        view.draft.min_free_space = Bytes::new(8 * Bytes::ONE_GIB);
        view.draft.network_speed = MegabitsPerSecond::new(250.0);
        view.handle_save_and_continue();
        wait_for_save(&mut view);
        assert!(!view.show_first_run);

        let saved_first_run = service.settings_store().load();
        assert!(saved_first_run.first_run_completed);
        assert_eq!(
            saved_first_run.cache.budget_bytes,
            Bytes::new(32 * Bytes::ONE_GIB)
        );
        assert_eq!(
            saved_first_run.cache.minimum_free_space_bytes,
            Bytes::new(8 * Bytes::ONE_GIB)
        );
        assert_eq!(
            saved_first_run.network.estimated_mbps,
            MegabitsPerSecond::new(250.0)
        );

        // Recreate the service and view as a fresh process would after restart.
        let restarted_service = AppService::new(Some(root));
        let mut restarted_view = SettingsView::new(&restarted_service);
        initialize_view(&mut restarted_view, &context);
        assert!(!restarted_view.show_first_run);
        assert_eq!(
            restarted_view.draft.cache_budget,
            Bytes::new(32 * Bytes::ONE_GIB)
        );
        assert_eq!(
            restarted_view.draft.network_speed,
            MegabitsPerSecond::new(250.0)
        );

        // Edit and save again over the existing file; verify it survives restart.
        restarted_view.draft.cache_budget = Bytes::new(40 * Bytes::ONE_GIB);
        restarted_view.draft.network_speed = MegabitsPerSecond::new(500.0);
        restarted_view.save_settings_async();
        wait_for_save(&mut restarted_view);

        let final_settings = restarted_service.settings_store().load();
        assert_eq!(
            final_settings.cache.budget_bytes,
            Bytes::new(40 * Bytes::ONE_GIB)
        );
        assert_eq!(
            final_settings.cache.minimum_free_space_bytes,
            Bytes::new(8 * Bytes::ONE_GIB)
        );
        assert_eq!(
            final_settings.network.estimated_mbps,
            MegabitsPerSecond::new(500.0)
        );
        assert!(final_settings.first_run_completed);
    }
}
