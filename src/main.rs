mod app;
mod chart;
mod rings;
mod scan;
mod treemap;
mod util;

fn main() -> eframe::Result {
    let start = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
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
        Box::new(move |cc| Ok(Box::new(app::App::new(&cc.egui_ctx, start)))),
    )
}
