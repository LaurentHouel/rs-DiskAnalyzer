#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod app;
mod chart;
mod i18n;
mod rings;
mod scan;
mod treemap;
mod util;

fn main() -> eframe::Result {
    // Optional first argument: folder to scan at startup (handy when debugging).
    // Without it, nothing is scanned until the user chooses a folder.
    let start = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 720.0])
            .with_min_inner_size([800.0, 400.0])
            .with_title("Disk Analyzer"),
        ..Default::default()
    };
    eframe::run_native(
        "Disk Analyzer",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, start)))),
    )
}
