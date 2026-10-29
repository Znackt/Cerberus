use cerberus::{AppService, MainWindow};
use std::fs;

fn main() -> eframe::Result<()> {
    // Initialize logging to file
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Cerberus")
        .join("logs");

    fs::create_dir_all(&log_dir).ok();

    let log_file = log_dir.join("cerberus.log");
    if let Ok(file) = fs::File::create(&log_file) {
        let _ = tracing_subscriber::fmt()
            .with_writer(file)
            .with_max_level(tracing::Level::INFO)
            .try_init();
        tracing::info!("Cerberus starting");
        tracing::info!("Log file: {}", log_file.display());
    } else {
        eprintln!("Failed to create log file at {}", log_file.display());
        // Fallback to stderr logging
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .try_init();
        tracing::warn!("Logging to stderr, log file creation failed");
    }

    // Create application service
    let app_service = AppService::new(None);

    // Configure and launch eframe
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([760.0, 560.0])
            .with_title("Cerberus"),
        ..Default::default()
    };

    eframe::run_native(
        "Cerberus",
        options,
        Box::new(|cc| Box::new(MainWindow::new(cc, app_service))),
    )
}
