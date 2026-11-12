use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["bytes", "kB", "MB", "GB", "TB", "PB"];
    if bytes < 1000 {
        return format!("{bytes} bytes");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

pub fn format_items(n: u64) -> String {
    if n == 1 {
        "1 item".to_owned()
    } else {
        format!("{n} items")
    }
}

pub fn format_age(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return String::new();
    };
    let days = SystemTime::now()
        .duration_since(time)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0);
    let plural = |n: u64, unit: &str| {
        if n == 1 {
            format!("1 {unit}")
        } else {
            format!("{n} {unit}s")
        }
    };
    match days {
        0 => "Today".to_owned(),
        1 => "Yesterday".to_owned(),
        2..=29 => plural(days, "day"),
        30..=364 => plural(days / 30, "month"),
        _ => plural(days / 365, "year"),
    }
}

/// Opens the system file manager with `path` selected.
pub fn reveal_in_file_manager(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = Command::new("xdg-open")
        .arg(path.parent().unwrap_or(path))
        .spawn();
}
