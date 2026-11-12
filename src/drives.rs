use egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke, vec2};
use std::path::PathBuf;
use sysinfo::{DiskKind, Disks};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DriveKind {
    Ssd,
    Hdd,
    Removable,
    Network,
    Optical,
}

pub struct Drive {
    pub name: String,
    pub mount: PathBuf,
    pub total: u64,
    pub available: u64,
    pub kind: DriveKind,
}

impl Drive {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }
}

const NETWORK_FS: [&str; 9] = [
    "nfs", "nfs4", "smbfs", "cifs", "afpfs", "webdav", "sshfs", "fuse.sshfs", "9p",
];
const OPTICAL_FS: [&str; 3] = ["cd9660", "udf", "iso9660"];
const PSEUDO_FS: [&str; 16] = [
    "tmpfs", "devtmpfs", "devfs", "squashfs", "overlay", "proc", "sysfs", "cgroup", "cgroup2",
    "efivarfs", "autofs", "fusectl", "ramfs", "debugfs", "tracefs", "bpf",
];
const HIDDEN_PREFIXES: [&str; 8] = [
    "/System/Volumes/VM",
    "/System/Volumes/Preboot",
    "/System/Volumes/Update",
    "/System/Volumes/xarts",
    "/System/Volumes/iSCPreboot",
    "/System/Volumes/Hardware",
    "/private/var/vm",
    "/boot/efi",
];

/// Mounted volumes worth scanning, without system-internal or pseudo filesystems.
pub fn list() -> Vec<Drive> {
    let Ok(disks) = Disks::new_with_refreshed_list() else {
        return Vec::new();
    };
    let mut drives: Vec<Drive> = disks
        .iter()
        .filter_map(|d| {
            let fs = d.file_system().to_string_lossy().to_ascii_lowercase();
            let mount = d.mount_point().to_path_buf();
            let mount_str = mount.to_string_lossy();
            let hidden = HIDDEN_PREFIXES.iter().any(|p| mount_str.starts_with(p))
                || ["/snap", "/run", "/sys", "/proc", "/dev"]
                    .iter()
                    .any(|p| mount_str == *p || mount_str.starts_with(&format!("{p}/")));
            if d.total_space() == 0 || hidden || PSEUDO_FS.contains(&fs.as_str()) {
                return None;
            }

            let kind = if OPTICAL_FS.contains(&fs.as_str()) {
                DriveKind::Optical
            } else if NETWORK_FS.contains(&fs.as_str()) {
                DriveKind::Network
            } else if d.is_removable() || (mount_str.starts_with("/Volumes/") && !is_boot_volume(&mount_str)) {
                DriveKind::Removable
            } else if d.kind() == DiskKind::HDD {
                DriveKind::Hdd
            } else {
                DriveKind::Ssd
            };

            let mut name = d.name().to_string_lossy().into_owned();
            if mount_str == "/System/Volumes/Data" {
                name = name.trim_end_matches(" - Data").to_owned();
            }
            if name.is_empty() {
                name = mount_str.into_owned();
            }
            Some(Drive {
                name,
                mount,
                total: d.total_space(),
                available: d.available_space(),
                kind,
            })
        })
        .collect();

    // On macOS the user data lives on the Data volume; "/" is only the sealed system snapshot.
    if drives.iter().any(|d| d.mount.as_path() == std::path::Path::new("/System/Volumes/Data")) {
        drives.retain(|d| d.mount.as_path() != std::path::Path::new("/"));
    }
    drives.sort_by(|a, b| a.mount.cmp(&b.mount));
    drives.dedup_by(|a, b| a.mount == b.mount);
    drives
}

fn is_boot_volume(mount: &str) -> bool {
    mount == "/" || mount.starts_with("/System/Volumes/")
}

/// Draws a vector icon matching the drive type inside `rect`.
pub fn paint_icon(painter: &Painter, rect: Rect, kind: DriveKind, accent: Color32) {
    let c = rect.center();
    let w = rect.width();
    let body = Color32::from_rgb(120, 126, 138);
    let dark = Color32::from_rgb(58, 62, 70);
    let outline = Stroke::new(1.2, Color32::from_black_alpha(140));
    let r = CornerRadius::same((w * 0.08) as u8);

    match kind {
        DriveKind::Ssd | DriveKind::Hdd | DriveKind::Removable => {
            let h = w * 0.55;
            let b = Rect::from_center_size(c, vec2(w, h));
            let fill = if kind == DriveKind::Removable {
                Color32::from_rgb(210, 150, 60)
            } else {
                body
            };
            painter.rect(b, r, fill, outline, egui::StrokeKind::Inside);
            let strip = Rect::from_min_max(Pos2::new(b.left(), b.bottom() - h * 0.32), b.max);
            painter.rect_filled(strip, CornerRadius { nw: 0, ne: 0, sw: r.sw, se: r.se }, dark);
            painter.circle_filled(Pos2::new(b.right() - w * 0.14, strip.center().y), w * 0.035, accent);
            match kind {
                DriveKind::Hdd => {
                    // Platter
                    painter.circle_stroke(
                        Pos2::new(b.center().x, b.top() + h * 0.34),
                        h * 0.2,
                        Stroke::new(1.5, dark),
                    );
                    painter.circle_filled(Pos2::new(b.center().x, b.top() + h * 0.34), h * 0.05, dark);
                }
                DriveKind::Ssd => {
                    painter.text(
                        Pos2::new(b.center().x, b.top() + h * 0.34),
                        Align2::CENTER_CENTER,
                        "SSD",
                        FontId::proportional(w * 0.22),
                        Color32::WHITE,
                    );
                }
                _ => {
                    // USB plug on the side.
                    let plug = Rect::from_center_size(
                        Pos2::new(b.right() + w * 0.06, b.top() + h * 0.34),
                        vec2(w * 0.12, h * 0.3),
                    );
                    painter.rect(plug, 1.0, Color32::from_gray(200), outline, egui::StrokeKind::Inside);
                    painter.text(
                        Pos2::new(b.center().x - w * 0.04, b.top() + h * 0.34),
                        Align2::CENTER_CENTER,
                        "USB",
                        FontId::proportional(w * 0.2),
                        Color32::WHITE,
                    );
                }
            }
        }
        DriveKind::Optical => {
            let radius = w * 0.46;
            painter.circle(c, radius, Color32::from_rgb(190, 196, 210), outline);
            painter.circle_stroke(c, radius * 0.7, Stroke::new(1.0, Color32::from_rgb(140, 150, 175)));
            painter.circle_filled(c, radius * 0.22, Color32::from_gray(40));
            painter.circle_filled(c, radius * 0.1, Color32::from_gray(235));
            painter.add(Shape::line(
                vec![
                    c + vec2(-radius * 0.6, -radius * 0.45),
                    c + vec2(-radius * 0.3, -radius * 0.75),
                ],
                Stroke::new(2.0, Color32::from_white_alpha(170)),
            ));
        }
        DriveKind::Network => {
            // Cloud over a server base.
            let cloud = Color32::from_rgb(100, 150, 225);
            let base = c + vec2(0.0, w * 0.06);
            for (dx, dy, rad) in [(-0.2, 0.04, 0.17), (0.0, -0.08, 0.22), (0.2, 0.02, 0.18)] {
                painter.circle_filled(base + vec2(w * dx, w * dy), w * rad, cloud);
            }
            painter.rect_filled(
                Rect::from_min_max(
                    base + vec2(-w * 0.3, w * 0.0),
                    base + vec2(w * 0.36, w * 0.2),
                ),
                CornerRadius::same((w * 0.08) as u8),
                cloud,
            );
            painter.circle_filled(base + vec2(w * 0.24, w * 0.1), w * 0.03, accent);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_at_least_one_drive_with_sane_numbers() {
        let drives = list();
        assert!(!drives.is_empty());
        for d in &drives {
            assert!(d.total > 0 && d.available <= d.total, "{}", d.name);
            println!("{:?} {} {:?} {}/{}", d.kind, d.name, d.mount, d.used(), d.total);
        }
    }
}
