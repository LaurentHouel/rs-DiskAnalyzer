use crate::scan::Node;
use crate::i18n::{Key, Lang, format_age, format_items, format_size, tr};
use egui::{Color32, Ui};
use std::path::Path;

pub enum ChartAction {
    Select(Vec<usize>),
    Open(Vec<usize>),
    Up,
}

pub struct ChartCtx<'a> {
    pub lang: Lang,
    pub root: &'a Node,
    pub root_path: &'a Path,
    /// Path (from the scan root) of the directory shown by the chart.
    pub base: &'a [usize],
    pub selected: Option<&'a [usize]>,
}

pub fn tooltip(ui: &mut Ui, cx: &ChartCtx, path: &[usize]) {
    let Some(node) = cx.root.get(path) else {
        return;
    };
    let total = cx.root.get(cx.base).map_or(0, |n| n.size).max(1);
    ui.strong(&node.name);
    ui.label(cx.root.path_of(cx.root_path, path).display().to_string());
    ui.label(format!(
        "{} ({:.1}%)",
        format_size(cx.lang, node.size),
        node.size as f64 * 100.0 / total as f64
    ));
    if node.is_dir {
        ui.label(format_items(cx.lang, node.items));
    }
    ui.label(format!(
        "{} {}",
        tr(cx.lang, Key::ModifiedLabel),
        format_age(cx.lang, node.modified)
    ));
}

pub fn text_color_on(bg: Color32) -> Color32 {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum > 150.0 {
        Color32::from_rgb(20, 20, 20)
    } else {
        Color32::WHITE
    }
}
