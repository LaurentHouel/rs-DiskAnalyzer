#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod app;
mod chart;
mod drives;
mod i18n;
mod icons;
mod rings;
mod scan;
mod theme;
mod treemap;
mod update;
mod util;
mod widgets;

/// Some Windows Vulkan drivers (e.g. old Intel ones) crash while the device is created, so
/// Direct3D 12 is used unless the user picked a backend through `WGPU_BACKEND`.
#[cfg(target_os = "windows")]
fn prefer_dx12_on_windows(options: &mut eframe::NativeOptions) {
    if std::env::var_os("WGPU_BACKEND").is_some() {
        return;
    }
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        setup.instance_descriptor.backends = eframe::wgpu::Backends::DX12;
    }
}

#[cfg(not(target_os = "windows"))]
fn prefer_dx12_on_windows(_options: &mut eframe::NativeOptions) {}

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
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1200.0, 720.0])
        .with_min_inner_size([800.0, 400.0])
        .with_title("Disk Analyzer");
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png")) {
        viewport = viewport.with_icon(icon);
    }
    // macOS: the native window buttons float over our own title row.
    #[cfg(target_os = "macos")]
    {
        viewport = viewport
            .with_fullsize_content_view(true)
            .with_title_shown(false)
            .with_titlebar_shown(false);
    }
    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    prefer_dx12_on_windows(&mut options);
    eframe::run_native(
        "Disk Analyzer",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, start)))),
    )
}
