use crate::app::service::AppService;
use egui::{Color32, RichText, Ui};

/// Dashboard view.
pub struct DashboardView {
    app_service: AppService,
}

impl DashboardView {
    /// Create a new dashboard view.
    pub fn new(app_service: &AppService) -> Self {
        Self {
            app_service: app_service.clone(),
        }
    }

    /// Show the dashboard view.
    /// Render the dashboard and return whether the first-run CTA requested
    /// navigation to Settings.
    pub fn show(&self, ui: &mut Ui) -> bool {
        ui.heading("Your cloud game library, on your PC");
        ui.label("Cerberus is a cloud-to-SSD storage app. Games will run on this PC; this is not cloud gaming.");
        ui.add_space(16.0);

        // Status cards
        ui.columns(3, |columns| {
            self.show_card(
                &mut columns[0],
                "Cloud Drives",
                "No drives mounted. Drive support is planned for a later phase.",
            );
            self.show_card(
                &mut columns[1],
                "Game Library",
                "No games added. Game scanning and launch support are planned.",
            );
            self.show_card(
                &mut columns[2],
                "Smart Cache",
                "Cache settings are saved; cache management is not active yet.",
            );
        });

        ui.add_space(20.0);

        // Show first-run welcome if needed
        let needs_first_run = {
            let state = self.app_service.read_state();
            state.settings_loaded && state.needs_first_run()
        };
        let mut open_settings = false;
        if needs_first_run {
            ui.separator();
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                ui.heading(RichText::new("Welcome to Cerberus").color(Color32::from_rgb(130, 170, 255)));
                ui.add_space(10.0);
                ui.label("Complete the first-run setup in Settings to configure your cache and network preferences.");
                ui.add_space(10.0);
                if ui.button("Open Settings").clicked() {
                    open_settings = true;
                }
            });
        }

        ui.add_space(14.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("Phase 1 · Foundation").color(Color32::from_rgb(130, 170, 255)));
        });
        open_settings
    }

    fn show_card(&self, ui: &mut Ui, title: &str, body: &str) {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(240.0);
            ui.label(RichText::new(title).size(16.0).strong());
            ui.add_space(5.0);
            ui.label(body);
        });
    }
}
