use crate::app::service::AppService;
use crate::ui::views::{CloudDrivesView, DashboardView, DiagnosticsView, SettingsView};
use egui::{
    Align, CentralPanel, Color32, Context, Layout, RichText, ScrollArea, SidePanel, Ui, Vec2,
    Visuals,
};

/// Navigation view state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationView {
    Dashboard,
    CloudDrives,
    GameLibrary,
    Prepare,
    SmartCache,
    Performance,
    Settings,
    Diagnostics,
}

/// Navigation items for sidebar.
const NAVIGATION_ITEMS: [(&str, NavigationView); 8] = [
    ("Dashboard", NavigationView::Dashboard),
    ("Cloud Drives", NavigationView::CloudDrives),
    ("Game Library", NavigationView::GameLibrary),
    ("Prepare", NavigationView::Prepare),
    ("Smart Cache", NavigationView::SmartCache),
    ("Performance", NavigationView::Performance),
    ("Settings", NavigationView::Settings),
    ("Diagnostics", NavigationView::Diagnostics),
];

/// Main application window.
pub struct MainWindow {
    app_service: AppService,
    current_view: NavigationView,
    dashboard_view: DashboardView,
    cloud_drives_view: CloudDrivesView,
    settings_view: SettingsView,
    diagnostics_view: DiagnosticsView,
}

impl MainWindow {
    /// Create a new main window.
    pub fn new(cc: &eframe::CreationContext<'_>, app_service: AppService) -> Self {
        // Apply theme from settings
        let theme = app_service.read_state().settings.theme_preference;
        Self::apply_theme(&cc.egui_ctx, theme);

        Self {
            app_service: app_service.clone(),
            current_view: NavigationView::Dashboard,
            dashboard_view: DashboardView::new(&app_service),
            cloud_drives_view: CloudDrivesView::new(&app_service),
            settings_view: SettingsView::new(&app_service),
            diagnostics_view: DiagnosticsView::new(&app_service),
        }
    }

    /// Apply theme to the UI context.
    fn apply_theme(ctx: &Context, theme: crate::domain::ThemePreference) {
        let mut style = (*ctx.style()).clone();
        style.visuals = match theme {
            crate::domain::ThemePreference::Dark => Visuals::dark(),
            crate::domain::ThemePreference::Light => Visuals::light(),
            crate::domain::ThemePreference::System => {
                // System theme - use dark as default
                Visuals::dark()
            }
        };
        style.spacing.item_spacing = Vec2::new(12.0, 10.0);
        ctx.set_style(style);
    }

    /// Get the current navigation view.
    pub fn current_view(&self) -> NavigationView {
        self.current_view
    }
}

impl eframe::App for MainWindow {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Err(error) = self.app_service.shutdown() {
            tracing::error!("Failed to stop Cerberus mounts during shutdown: {error}");
        }
    }

    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Check for theme changes
        let current_theme = self.app_service.read_state().settings.theme_preference;
        let style = (*ctx.style()).clone();
        let is_dark = style.visuals.dark_mode;
        let theme_is_dark = matches!(current_theme, crate::domain::ThemePreference::Dark);

        // Apply theme immediately if changed
        if is_dark != theme_is_dark {
            Self::apply_theme(ctx, current_theme);
        }

        // Draw sidebar
        SidePanel::left("navigation")
            .default_width(205.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(18.0);
                ui.label(RichText::new("CERBERUS").size(22.0).strong());
                ui.label(RichText::new("Cloud-to-SSD engine").weak());
                ui.add_space(24.0);

                for (title, view) in NAVIGATION_ITEMS {
                    let is_active = self.current_view == view;
                    let response = ui.selectable_label(is_active, title);
                    if response.clicked() {
                        self.current_view = view;
                    }
                }

                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).weak());
                });
            });

        // Draw central panel
        let mut navigate_to_settings = false;
        CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(self.current_view_title());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("No drives mounted").weak());
                });
            });
            ui.separator();
            ScrollArea::vertical().show(ui, |ui| match self.current_view {
                NavigationView::Dashboard => {
                    navigate_to_settings = self.dashboard_view.show(ui);
                }
                NavigationView::CloudDrives => self.cloud_drives_view.show(ui),
                NavigationView::Settings => self.settings_view.show(ui),
                NavigationView::Diagnostics => self.diagnostics_view.show(ui),
                NavigationView::GameLibrary => {
                    Self::show_placeholder_view(ui, "Game Library", "Game scanning and library")
                }
                NavigationView::Prepare => {
                    Self::show_placeholder_view(ui, "Prepare", "Game preparation")
                }
                NavigationView::SmartCache => {
                    Self::show_placeholder_view(ui, "Smart Cache", "Cache management")
                }
                NavigationView::Performance => {
                    Self::show_placeholder_view(ui, "Performance", "Performance metrics")
                }
            });
        });
        if navigate_to_settings {
            self.current_view = NavigationView::Settings;
        }
    }
}

impl MainWindow {
    fn current_view_title(&self) -> &'static str {
        match self.current_view {
            NavigationView::Dashboard => "Dashboard",
            NavigationView::CloudDrives => "Cloud Drives",
            NavigationView::GameLibrary => "Game Library",
            NavigationView::Prepare => "Prepare",
            NavigationView::SmartCache => "Smart Cache",
            NavigationView::Performance => "Performance",
            NavigationView::Settings => "Settings",
            NavigationView::Diagnostics => "Diagnostics",
        }
    }

    /// Show a placeholder view for unimplemented features.
    fn show_placeholder_view(ui: &mut Ui, title: &str, feature: &str) {
        ui.vertical_centered(|ui| {
            ui.heading(title);
            ui.add_space(12.0);
            ui.label(format!("{} is not implemented yet.", feature));
            ui.add_space(10.0);
            ui.label(
                RichText::new("Planned for a later phase")
                    .small()
                    .color(Color32::LIGHT_GRAY),
            );
        });
    }
}
