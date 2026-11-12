#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod app;
mod chart;
mod drives;
mod i18n;
mod rings;
mod scan;
mod treemap;
mod util;

fn main() -> eframe::Result {
    // The app never scans on its own. Only the explicit `--scan <folder>` option (debugging aid)
    // starts a scan at launch; any other argument, e.g. added by the OS, is ignored.
    let mut args = std::env::args_os().skip(1);
    let mut start = None;
    while let Some(arg) = args.next() {
        if arg == "--scan" {
            start = args.next().map(std::path::PathBuf::from);
        }
    }
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
