use crate::app::service::AppService;
use crate::domain::ids::DriveId;
use crate::domain::models::DriveDefinition;
use egui::{Color32, RichText, ScrollArea, Ui};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

type DependencyCheck = (
    crate::domain::status::DiagnosticStatus,
    crate::domain::status::DiagnosticStatus,
    Option<String>,
);
type RemoteListResult = Result<Vec<crate::cloud::rclone::RemoteInfo>, String>;

/// Cloud drives view.
pub struct CloudDrivesView {
    app_service: AppService,
    show_add_dialog: bool,
    show_edit_dialog: Option<DriveId>,
    draft: DriveDraft,
    save_error: Option<String>,
    is_saving: bool,
    show_warning: Option<String>,
    dependency_check_done: Arc<AtomicBool>,
    dependency_check_running: Arc<AtomicBool>,
    dependency_status: Arc<Mutex<Option<DependencyCheck>>>,
    remotes: Arc<Mutex<Option<RemoteListResult>>>,
    remote_loading: Arc<AtomicBool>,
    remote_test_result: Arc<Mutex<Option<String>>>,
    operation_error: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, Clone, Default)]
struct DriveDraft {
    display_name: String,
    remote_name: String,
    remote_path: String,
    drive_letter: String,
    auto_mount: bool,
}

impl CloudDrivesView {
    pub fn new(app_service: &AppService) -> Self {
        let _app_service_clone = app_service.clone();
        Self {
            app_service: app_service.clone(),
            show_add_dialog: false,
            show_edit_dialog: None,
            draft: DriveDraft::default(),
            save_error: None,
            is_saving: false,
            show_warning: None,
            dependency_check_done: Arc::new(AtomicBool::new(false)),
            dependency_check_running: Arc::new(AtomicBool::new(false)),
            dependency_status: Arc::new(Mutex::new(None)),
            remotes: Arc::new(Mutex::new(None)),
            remote_loading: Arc::new(AtomicBool::new(false)),
            remote_test_result: Arc::new(Mutex::new(None)),
            operation_error: Arc::new(Mutex::new(None)),
        }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if !self.dependency_check_done.load(Ordering::Relaxed) {
            self.check_dependencies(ctx.clone());
            self.refresh_remotes(ctx.clone());
        }
        ui.heading("Cloud Drives");
        ui.add_space(5.0);

        self.show_dependency_status(ui);

        ui.label("Configure cloud drives for virtual mounting.");
        ui.horizontal(|ui| {
            if ui.button("Open rclone config").clicked() {
                if let Err(error) = self.app_service.launch_rclone_config() {
                    self.save_error = Some(error.to_string());
                } else {
                    self.show_warning = Some("Complete rclone configuration in the opened console, then refresh remotes.".into());
                }
            }
            if ui.add_enabled(!self.remote_loading.load(Ordering::Relaxed), egui::Button::new("Refresh remotes")).clicked() {
                self.refresh_remotes(ctx.clone());
            }
        });
        ui.add_space(14.0);

        // Global settings
        ui.group(|ui| {
            ui.heading("Drive Startup Settings");
            ui.add_space(10.0);

            let current_restore = self.app_service.read_state().settings.restore_drives_on_startup;
            let mut restore_on_startup = current_restore;
            if ui.checkbox(&mut restore_on_startup, "Restore drives on startup").changed() {
                let _ = self.app_service.set_restore_drives_on_startup(restore_on_startup);
            }
            ui.label(RichText::new("When enabled, drives with auto-mount enabled will be automatically mounted when Cerberus starts.").small().color(Color32::LIGHT_GRAY));
        });

        ui.add_space(15.0);

        // Drive list
        ui.group(|ui| {
            ui.heading("Saved Drives");
            ui.add_space(10.0);

            let drives = self.app_service.list_saved_drives();

            if drives.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.label("No cloud drives configured");
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("Add your first drive to get started")
                            .small()
                            .color(Color32::LIGHT_GRAY),
                    );
                });
            } else {
                ScrollArea::vertical().show(ui, |ui| {
                    for drive in drives {
                        self.show_drive_row(ui, drive);
                    }
                });
            }

            ui.add_space(10.0);
            if ui
                .button(RichText::new("Add Cloud Drive").strong())
                .clicked()
            {
                self.show_add_dialog = true;
                self.draft = DriveDraft::default();
                self.save_error = None;
                self.show_warning = None;
            }
        });

        // Add/Edit dialog
        if self.show_add_dialog || self.show_edit_dialog.is_some() {
            self.show_drive_dialog(ui);
        }

        // Warnings and errors
        if let Some(warning) = &self.show_warning {
            ui.add_space(10.0);
            ui.colored_label(Color32::LIGHT_YELLOW, warning);
        }

        if let Some(error) = &self.save_error {
            ui.add_space(10.0);
            ui.colored_label(Color32::LIGHT_RED, error);
        }
    }

    fn show_dependency_status(&mut self, ui: &mut Ui) {
        let status = self.dependency_status.lock().ok().and_then(|s| s.clone());
        let is_running = self.dependency_check_running.load(Ordering::Relaxed);
        ui.group(|ui| {
            ui.heading("Dependencies");
            ui.add_space(5.0);

            // Show cached status
            ui.horizontal(|ui| {
                ui.label("rclone:");
                show_dependency_label(ui, "rclone", status.as_ref().map(|s| s.0), is_running);
            });

            ui.horizontal(|ui| {
                ui.label("WinFsp:");
                show_dependency_label(ui, "WinFsp", status.as_ref().map(|s| s.1), is_running);
            });
            if let Some(error) = status.as_ref().and_then(|entry| entry.2.as_ref()) {
                ui.colored_label(Color32::YELLOW, format!("Dependency check issue: {error}"));
            }

            if ui
                .add_enabled(!is_running, egui::Button::new("Check Dependencies"))
                .clicked()
            {
                self.check_dependencies(ui.ctx().clone());
            }

            // Display operation error if any
            if let Ok(error) = self.operation_error.lock() {
                if let Some(err) = error.as_ref() {
                    ui.add_space(5.0);
                    ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
                }
            }
        });
        ui.add_space(10.0);
    }

    fn check_dependencies(&self, ctx: egui::Context) {
        if self.dependency_check_running.swap(true, Ordering::AcqRel) {
            return;
        }
        let service = self.app_service.clone();
        let result = Arc::clone(&self.dependency_status);
        let done = Arc::clone(&self.dependency_check_done);
        let running = Arc::clone(&self.dependency_check_running);
        std::thread::spawn(move || {
            let error = service.refresh_dependencies().err().map(|e| e.to_string());
            let d = service.get_diagnostics();
            if let Ok(mut out) = result.lock() {
                *out = Some((d.rclone_available, d.winfsp_available, error));
            }
            done.store(true, Ordering::Release);
            running.store(false, Ordering::Release);
            ctx.request_repaint();
        });
    }

    fn refresh_remotes(&self, ctx: egui::Context) {
        if self.remote_loading.swap(true, Ordering::AcqRel) {
            return;
        }
        let service = self.app_service.clone();
        let out = Arc::clone(&self.remotes);
        let loading = Arc::clone(&self.remote_loading);
        std::thread::spawn(move || {
            let result = service.list_rclone_remotes().map_err(|e| e.to_string());
            if let Ok(mut cache) = out.lock() {
                *cache = Some(result);
            }
            loading.store(false, Ordering::Release);
            ctx.request_repaint();
        });
    }

    fn show_drive_row(&mut self, ui: &mut Ui, drive: DriveDefinition) {
        let mount_status = self.app_service.get_mount_status(&drive.id);
        let repaint_ctx = ui.ctx().clone();
        if matches!(
            mount_status,
            Some(
                crate::domain::status::MountStatus::Mounting
                    | crate::domain::status::MountStatus::Unmounting
            )
        ) {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
        }

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(drive.display_name.value()).strong());
                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        ui.label("Remote:");
                        ui.label(format!("{}/{}", drive.remote_name, drive.remote_path));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Drive letter:");
                        ui.label(
                            drive
                                .drive_letter
                                .clone()
                                .unwrap_or_else(|| "Not assigned".to_string()),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Auto-mount:");
                        ui.label(if drive.auto_mount {
                            "Enabled"
                        } else {
                            "Disabled"
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Status:");
                        let status_text = mount_status
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "Unmounted".to_string());
                        let status_color = match mount_status {
                            Some(crate::domain::status::MountStatus::Mounted) => {
                                Color32::LIGHT_GREEN
                            }
                            Some(crate::domain::status::MountStatus::Mounting) => Color32::YELLOW,
                            Some(crate::domain::status::MountStatus::Failed) => Color32::LIGHT_RED,
                            _ => Color32::LIGHT_GRAY,
                        };
                        ui.colored_label(status_color, status_text);
                    });
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Mount/Unmount buttons
                    match mount_status {
                        Some(crate::domain::status::MountStatus::Mounted) => {
                            if ui.button("Unmount").clicked() {
                                let drive_id = drive.id;
                                let app_service = self.app_service.clone();
                                let operation_error = Arc::clone(&self.operation_error);
                                let ctx = repaint_ctx.clone();
                                std::thread::spawn(move || {
                                    if let Err(e) = app_service.unmount_drive(&drive_id) {
                                        if let Ok(mut err) = operation_error.lock() {
                                            *err = Some(format!("Unmount failed: {}", e));
                                        }
                                    } else {
                                        if let Ok(mut err) = operation_error.lock() {
                                            *err = None;
                                        }
                                    }
                                    ctx.request_repaint();
                                });
                            }
                        }
                        Some(crate::domain::status::MountStatus::Unmounting)
                        | Some(crate::domain::status::MountStatus::Mounting) => {
                            ui.add_enabled(false, egui::Button::new("Busy..."));
                        }
                        _ => {
                            if ui.button("Mount").clicked() {
                                let drive_clone = drive.clone();
                                let app_service = self.app_service.clone();
                                let operation_error = Arc::clone(&self.operation_error);
                                let ctx = repaint_ctx.clone();
                                std::thread::spawn(move || {
                                    if let Err(e) = app_service.mount_drive(drive_clone) {
                                        if let Ok(mut err) = operation_error.lock() {
                                            *err = Some(format!("Mount failed: {}", e));
                                        }
                                    } else {
                                        if let Ok(mut err) = operation_error.lock() {
                                            *err = None;
                                        }
                                    }
                                    ctx.request_repaint();
                                });
                            }
                        }
                    }

                    let can_edit_remove = matches!(
                        mount_status,
                        None | Some(
                            crate::domain::status::MountStatus::Unmounted
                                | crate::domain::status::MountStatus::Failed
                        )
                    );
                    if ui
                        .add_enabled(can_edit_remove, egui::Button::new("Edit"))
                        .clicked()
                    {
                        self.show_edit_dialog = Some(drive.id);
                        self.draft = DriveDraft {
                            display_name: drive.display_name.value().to_string(),
                            remote_name: drive.remote_name,
                            remote_path: drive.remote_path,
                            drive_letter: drive.drive_letter.unwrap_or_default(),
                            auto_mount: drive.auto_mount,
                        };
                        self.save_error = None;
                    }

                    if ui
                        .add_enabled(can_edit_remove, egui::Button::new("Remove"))
                        .clicked()
                        && can_edit_remove
                    {
                        let drive_id = drive.id;
                        let app_service = self.app_service.clone();
                        let operation_error = Arc::clone(&self.operation_error);
                        std::thread::spawn(move || {
                            if let Err(e) = app_service.remove_drive_definition(drive_id) {
                                if let Ok(mut err) = operation_error.lock() {
                                    *err = Some(format!("Remove failed: {}", e));
                                }
                            }
                        });
                    }
                });
            });
        });
        ui.add_space(8.0);
    }

    fn show_drive_dialog(&mut self, ui: &mut Ui) {
        let title = if self.show_edit_dialog.is_some() {
            "Edit Cloud Drive"
        } else {
            "Add Cloud Drive"
        };

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.label("Display Name");
                ui.text_edit_singleline(&mut self.draft.display_name);
                ui.add_space(10.0);

                ui.label("Remote");
                let remotes = self.remotes.lock().ok().and_then(|cache| cache.clone());
                match remotes {
                    Some(Ok(items)) if !items.is_empty() => {
                        egui::ComboBox::from_id_source("remote_selector")
                            .selected_text(if self.draft.remote_name.is_empty() {
                                "Select remote"
                            } else {
                                &self.draft.remote_name
                            })
                            .show_ui(ui, |ui| {
                                for remote in items {
                                    ui.selectable_value(
                                        &mut self.draft.remote_name,
                                        remote.name.clone(),
                                        remote.display_name,
                                    );
                                }
                            });
                    }
                    Some(Ok(_)) => {
                        ui.label("No rclone remotes configured. Run rclone config, then refresh.");
                    }
                    Some(Err(error)) => {
                        ui.colored_label(Color32::LIGHT_RED, error);
                    }
                    None => {
                        ui.label(if self.remote_loading.load(Ordering::Relaxed) {
                            "Loading remotes…"
                        } else {
                            "Remote list unavailable"
                        });
                    }
                }
                if ui
                    .add_enabled(
                        !self.remote_loading.load(Ordering::Relaxed),
                        egui::Button::new("Refresh remotes"),
                    )
                    .clicked()
                {
                    self.refresh_remotes(ui.ctx().clone());
                }
                ui.add_space(10.0);

                ui.label("Remote Path");
                ui.text_edit_singleline(&mut self.draft.remote_path);
                ui.label(
                    RichText::new("Path within the remote")
                        .small()
                        .color(Color32::LIGHT_GRAY),
                );
                ui.add_space(10.0);

                let test_running = self.remote_loading.load(Ordering::Relaxed);
                if ui
                    .add_enabled(
                        !test_running && !self.draft.remote_name.is_empty(),
                        egui::Button::new(if test_running {
                            "Testing…"
                        } else {
                            "Test connection"
                        }),
                    )
                    .clicked()
                {
                    let service = self.app_service.clone();
                    let name = self.draft.remote_name.clone();
                    let path = self.draft.remote_path.clone();
                    let result = Arc::clone(&self.remote_test_result);
                    let loading = Arc::clone(&self.remote_loading);
                    let ctx = ui.ctx().clone();
                    loading.store(true, Ordering::Release);
                    std::thread::spawn(move || {
                        let message = match service.test_rclone_remote(&name, &path) {
                            Ok(()) => "Connection test succeeded".to_owned(),
                            Err(error) => format!("Connection test failed: {error}"),
                        };
                        if let Ok(mut output) = result.lock() {
                            *output = Some(message);
                        }
                        loading.store(false, Ordering::Release);
                        ctx.request_repaint();
                    });
                }
                if let Ok(result) = self.remote_test_result.lock() {
                    if let Some(message) = result.as_ref() {
                        ui.label(message);
                    }
                }

                ui.label("Drive Letter");
                ui.text_edit_singleline(&mut self.draft.drive_letter);
                ui.label(
                    RichText::new("Optional drive letter (e.g., E:)")
                        .small()
                        .color(Color32::LIGHT_GRAY),
                );
                ui.add_space(10.0);

                ui.checkbox(&mut self.draft.auto_mount, "Auto-mount on startup");
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        self.handle_save();
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_add_dialog = false;
                        self.show_edit_dialog = None;
                    }
                });
            });
    }

    fn handle_save(&mut self) {
        self.is_saving = true;
        self.save_error = None;

        // Validate
        if self.draft.display_name.trim().is_empty() {
            self.save_error = Some("Display name is required".into());
            self.is_saving = false;
            return;
        }
        if self.draft.remote_name.trim().is_empty() {
            self.save_error = Some("Remote name is required".into());
            self.is_saving = false;
            return;
        }
        let display_name =
            match crate::domain::units::DisplayName::new(self.draft.display_name.clone()) {
                Ok(name) => name,
                Err(error) => {
                    self.save_error = Some(error);
                    self.is_saving = false;
                    return;
                }
            };
        let letter = self.draft.drive_letter.trim().trim_end_matches(':');
        if !letter.is_empty() && (letter.len() != 1 || !letter.as_bytes()[0].is_ascii_alphabetic())
        {
            self.save_error = Some("Drive letter must be a single letter, such as E or E:".into());
            self.is_saving = false;
            return;
        }

        let drive = DriveDefinition {
            id: DriveId::new(),
            display_name,
            remote_name: self.draft.remote_name.clone(),
            remote_path: self.draft.remote_path.clone(),
            drive_letter: if self.draft.drive_letter.trim().is_empty() {
                None
            } else {
                Some(format!("{}:", letter.to_ascii_uppercase()))
            },
            auto_mount: self.draft.auto_mount,
            permanent: false,
        };

        // Handle save asynchronously
        let app_service = self.app_service.clone();
        let operation_error = Arc::clone(&self.operation_error);
        let drive_clone = drive.clone();
        let id_opt = self.show_edit_dialog;

        std::thread::spawn(move || {
            let result = if let Some(id) = id_opt {
                let mut drive = drive_clone;
                drive.id = id;
                app_service.update_drive_definition(id, drive)
            } else {
                app_service.add_drive_definition(drive_clone)
            };

            if let Err(e) = result {
                if let Ok(mut err) = operation_error.lock() {
                    *err = Some(format!("Save failed: {}", e));
                }
            } else {
                if let Ok(mut err) = operation_error.lock() {
                    *err = None;
                }
            }
        });

        self.is_saving = false;
        self.show_add_dialog = false;
        self.show_edit_dialog = None;
    }
}

fn show_dependency_label(
    ui: &mut Ui,
    name: &str,
    status: Option<crate::domain::status::DiagnosticStatus>,
    running: bool,
) {
    ui.label(format!("{name}:"));
    let (color, text) = match status {
        Some(crate::domain::status::DiagnosticStatus::Available) => {
            (Color32::LIGHT_GREEN, "Available")
        }
        Some(crate::domain::status::DiagnosticStatus::NotAvailable) => {
            (Color32::LIGHT_RED, "Not available")
        }
        Some(crate::domain::status::DiagnosticStatus::Error) => (Color32::YELLOW, "Check failed"),
        _ if running => (Color32::LIGHT_YELLOW, "Checking…"),
        _ => (Color32::LIGHT_GRAY, "Not checked"),
    };
    ui.colored_label(color, text);
}
