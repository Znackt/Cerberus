use crate::app::service::AppService;
use crate::domain::models::AppDiagnostics;
use egui::{Color32, Context, RichText, ScrollArea, Ui};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone, Copy)]
struct PathStatus {
    settings_file_exists: bool,
    log_file_exists: bool,
}

/// Diagnostics view.
pub struct DiagnosticsView {
    app_service: AppService,
    log_content: Arc<Mutex<Option<String>>>,
    log_loading: Arc<Mutex<bool>>,
    log_error: Arc<Mutex<Option<String>>>,
    last_log_refresh: Arc<Mutex<std::time::Instant>>,
    path_status: Arc<Mutex<Option<PathStatus>>>,
    path_status_loading: Arc<Mutex<bool>>,
    last_path_refresh: Arc<Mutex<std::time::Instant>>,
    egui_ctx: Option<Context>,
}

impl DiagnosticsView {
    /// Create a new diagnostics view.
    pub fn new(app_service: &AppService) -> Self {
        Self {
            app_service: app_service.clone(),
            log_content: Arc::new(Mutex::new(None)),
            log_loading: Arc::new(Mutex::new(false)),
            log_error: Arc::new(Mutex::new(None)),
            last_log_refresh: Arc::new(Mutex::new(
                std::time::Instant::now() - std::time::Duration::from_secs(10),
            )),
            path_status: Arc::new(Mutex::new(None)),
            path_status_loading: Arc::new(Mutex::new(false)),
            last_path_refresh: Arc::new(Mutex::new(
                std::time::Instant::now() - std::time::Duration::from_secs(10),
            )),
            egui_ctx: None,
        }
    }

    /// Show the diagnostics view.
    pub fn show(&mut self, ui: &mut Ui) {
        // Store egui context for async updates
        self.egui_ctx = Some(ui.ctx().clone());

        ui.heading(RichText::new("Diagnostics").strong());
        ui.add_space(10.0);

        let diagnostics = self.app_service.get_diagnostics();
        if let (Some(settings_path), Some(log_dir)) =
            (&diagnostics.settings_file_path, &diagnostics.log_directory)
        {
            let should_refresh = {
                let last_refresh = *self.last_path_refresh.lock().unwrap();
                last_refresh.elapsed() > std::time::Duration::from_secs(5)
            };
            if should_refresh {
                self.load_path_status_async(settings_path.clone(), log_dir.join("cerberus.log"));
            }
        }
        let path_status = *self.path_status.lock().unwrap();
        let path_loading = *self.path_status_loading.lock().unwrap();

        // App Information
        self.show_app_info(ui, &diagnostics);

        ui.add_space(15.0);

        // Status sections
        ui.group(|ui| {
            ui.heading("System Status");
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.label("rclone:");
                ui.colored_label(
                    status_color(diagnostics.rclone_available),
                    diagnostics.rclone_available.to_string(),
                );
            });
            ui.horizontal(|ui| {
                ui.label("WinFsp:");
                ui.colored_label(
                    status_color(diagnostics.winfsp_available),
                    diagnostics.winfsp_available.to_string(),
                );
            });
        });

        ui.add_space(15.0);

        // Paths
        ui.group(|ui| {
            ui.heading("Paths");
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.label("Data directory:");
                ui.monospace(diagnostics.data_root_path.display().to_string());
            });
            ui.add_space(5.0);
            if let Some(cache) = &diagnostics.cache_root_path {
                ui.horizontal(|ui| {
                    ui.label("Cache directory:");
                    ui.monospace(cache.display().to_string());
                });
                ui.add_space(5.0);
            }
            if let Some(settings) = &diagnostics.settings_file_path {
                ui.horizontal(|ui| {
                    ui.label("Settings file:");
                    ui.monospace(settings.display().to_string());
                    match path_status {
                        Some(status) => ui.colored_label(
                            if status.settings_file_exists {
                                Color32::LIGHT_GREEN
                            } else {
                                Color32::LIGHT_RED
                            },
                            if status.settings_file_exists {
                                "✓ Exists"
                            } else {
                                "✗ Not found"
                            },
                        ),
                        None if path_loading => ui.label("Checking…"),
                        None => ui.label("Status unavailable"),
                    };
                });
                ui.add_space(5.0);
            }
            if let Some(logs) = &diagnostics.log_directory {
                ui.horizontal(|ui| {
                    ui.label("Log directory:");
                    ui.monospace(logs.display().to_string());
                });
            }
        });

        ui.add_space(15.0);

        // Log file content - load asynchronously
        if let Some(log_dir) = &diagnostics.log_directory {
            let log_file = log_dir.join("cerberus.log");
            if path_status
                .map(|status| status.log_file_exists)
                .unwrap_or(false)
            {
                ui.group(|ui| {
                    ui.heading("Recent Log Entries");
                    ui.add_space(10.0);

                    // Check if we need to load/refresh log content
                    let should_refresh = {
                        let last_refresh = *self.last_log_refresh.lock().unwrap();
                        last_refresh.elapsed() > std::time::Duration::from_secs(5)
                    };

                    if should_refresh {
                        self.load_log_async(log_file.clone());
                    }

                    // Display current log content or loading state
                    let loading = *self.log_loading.lock().unwrap();
                    let content = self.log_content.lock().unwrap().clone();
                    let error = self.log_error.lock().unwrap().clone();

                    if loading {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Loading log...");
                        });
                    } else if let Some(err) = error {
                        ui.colored_label(
                            Color32::LIGHT_RED,
                            format!("Failed to read log: {}", err),
                        );
                    } else if let Some(content) = content {
                        ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                            ui.monospace(content);
                        });
                    } else {
                        ui.label("No log entries yet.");
                    }

                    // Refresh button
                    ui.add_space(10.0);
                    if ui.button("Refresh log").clicked() {
                        self.load_log_async(log_file);
                    }
                });
            } else {
                ui.group(|ui| {
                    ui.heading("Recent Log Entries");
                    ui.add_space(10.0);
                    ui.label(if path_loading || path_status.is_none() {
                        "Checking log file…"
                    } else {
                        "Log file not found."
                    });
                });
            }
        }
    }

    /// Check diagnostic paths on a worker thread so the UI never performs
    /// filesystem metadata operations while drawing.
    fn load_path_status_async(
        &self,
        settings_path: std::path::PathBuf,
        log_file: std::path::PathBuf,
    ) {
        {
            let mut loading = self.path_status_loading.lock().unwrap();
            if *loading {
                return;
            }
            *loading = true;
        }
        *self.last_path_refresh.lock().unwrap() = std::time::Instant::now();

        let path_status = Arc::clone(&self.path_status);
        let path_loading = Arc::clone(&self.path_status_loading);
        let egui_ctx = self.egui_ctx.clone();
        thread::spawn(move || {
            let status = PathStatus {
                settings_file_exists: settings_path.is_file(),
                log_file_exists: log_file.is_file(),
            };
            *path_status.lock().unwrap() = Some(status);
            *path_loading.lock().unwrap() = false;
            if let Some(ctx) = egui_ctx {
                ctx.request_repaint();
            }
        });
    }

    /// Load log file asynchronously.
    fn load_log_async(&self, log_file: std::path::PathBuf) {
        // Check if already loading
        if *self.log_loading.lock().unwrap() {
            return;
        }

        *self.log_loading.lock().unwrap() = true;
        *self.last_log_refresh.lock().unwrap() = std::time::Instant::now();

        let log_content = Arc::clone(&self.log_content);
        let log_loading = Arc::clone(&self.log_loading);
        let log_error = Arc::clone(&self.log_error);
        let egui_ctx = self.egui_ctx.clone();

        thread::spawn(move || {
            // Read log file with size limit
            const MAX_LOG_SIZE: usize = 100_000; // 100 KB limit
            let result = std::fs::read(&log_file);

            let (content, error) = match result {
                Ok(bytes) => {
                    if bytes.len() > MAX_LOG_SIZE {
                        // Read only the last MAX_LOG_SIZE bytes
                        let start = bytes.len() - MAX_LOG_SIZE;
                        let partial = &bytes[start..];
                        // Find first newline to avoid partial line
                        let newline_pos = partial.iter().position(|&b| b == b'\n');
                        let display_bytes = if let Some(pos) = newline_pos {
                            &partial[pos + 1..]
                        } else {
                            partial
                        };
                        match String::from_utf8(display_bytes.to_vec()) {
                            Ok(s) => (Some(s), None),
                            Err(_) => (None, Some("Log file contains invalid UTF-8".to_string())),
                        }
                    } else {
                        match String::from_utf8(bytes) {
                            Ok(s) => (Some(s), None),
                            Err(_) => (None, Some("Log file contains invalid UTF-8".to_string())),
                        }
                    }
                }
                Err(e) => (None, Some(format!("Failed to read log: {}", e))),
            };

            *log_content.lock().unwrap() = content;
            *log_error.lock().unwrap() = error;
            *log_loading.lock().unwrap() = false;

            // Request UI repaint
            if let Some(ctx) = egui_ctx {
                ctx.request_repaint();
            }
        });
    }

    fn show_app_info(&self, ui: &mut Ui, diagnostics: &AppDiagnostics) {
        ui.group(|ui| {
            ui.heading("Application");
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.label("Version:");
                ui.monospace(&diagnostics.version);
            });
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label("Settings load:");
                let status = &diagnostics.settings_load_status;
                let color = match status {
                    crate::domain::SettingsLoadStatus::Loaded
                    | crate::domain::SettingsLoadStatus::FirstRunDefaults
                    | crate::domain::SettingsLoadStatus::Migrated { .. } => Color32::LIGHT_GREEN,
                    crate::domain::SettingsLoadStatus::Loading => Color32::LIGHT_GRAY,
                    _ => Color32::LIGHT_RED,
                };
                ui.colored_label(color, status.to_string());
            });
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label("Build:");
                ui.monospace("Release (MSVC)");
            });
        });
    }
}

fn status_color(status: crate::domain::status::DiagnosticStatus) -> Color32 {
    match status {
        crate::domain::status::DiagnosticStatus::Available => Color32::LIGHT_GREEN,
        crate::domain::status::DiagnosticStatus::NotAvailable => Color32::LIGHT_RED,
        crate::domain::status::DiagnosticStatus::Error => Color32::YELLOW,
        crate::domain::status::DiagnosticStatus::NotChecked => Color32::LIGHT_GRAY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use tempfile::tempdir;

    #[test]
    fn diagnostic_path_checks_complete_off_the_ui_thread() {
        let dir = tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");
        let log_file = dir.path().join("cerberus.log");
        std::fs::write(&settings_path, "{}").unwrap();
        std::fs::write(&log_file, "started").unwrap();

        let service = AppService::new(Some(dir.path().to_path_buf()));
        let mut view = DiagnosticsView::new(&service);
        view.egui_ctx = Some(Context::default());
        view.load_path_status_async(settings_path, log_file);

        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(status) = *view.path_status.lock().unwrap() {
                assert!(status.settings_file_exists);
                assert!(status.log_file_exists);
                return;
            }
            assert!(
                Instant::now() < deadline,
                "diagnostic file checks did not complete"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
