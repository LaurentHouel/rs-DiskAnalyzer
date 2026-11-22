use crate::scan::Node;
use crate::i18n::{Key, Lang, format_age, format_items, format_size, tr};
use egui::{Color32, Response, Ui};
use std::path::Path;

pub enum ChartAction {
    Select(Vec<usize>),
    Open(Vec<usize>),
    Up,
    Reveal(Vec<usize>),
    Trash(Vec<usize>),
    Delete(Vec<usize>),
    Info(Vec<usize>),
}

pub struct ChartCtx<'a> {
    pub lang: Lang,
    pub root: &'a Node,
    pub root_path: &'a Path,
    /// Path (from the scan root) of the directory shown by the chart.
    pub base: &'a [usize],
    pub selected: Option<&'a [usize]>,
}

/// Displays the name, filesystem path, size, item count, and modification age of a chart node.
///
/// # Parameters
/// - `ui`: UI in which to render the tooltip.
/// - `cx`: Chart data and localization context.
/// - `path`: Index path of the node relative to the scanned root.
///
/// # Returns
/// Nothing; does nothing when `path` does not identify a node.
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

/// Chooses a readable dark or light text color based on the background luminance.
///
/// # Parameters
/// - `bg`: Background color on which the text will be drawn.
///
/// # Returns
/// Near-black text for a light background, or white text for a dark background.
pub fn text_color_on(bg: Color32) -> Color32 {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum > 150.0 {
        Color32::from_rgb(20, 20, 20)
    } else {
        Color32::WHITE
    }
}

/// Right-click menu for the chart item under the pointer when the click happened.
/// The target is remembered so the menu keeps working while the pointer moves over it.
///
/// # Parameters
/// - `ui`: UI context used to retain the target and render the menu.
/// - `resp`: Chart interaction response that owns the context menu.
/// - `cx`: Chart tree, path, and localization context.
/// - `hovered`: Index path of the item currently under the pointer, if any.
///
/// # Returns
/// The selected chart action, or `None` when no menu action was chosen.
pub fn item_menu(
    ui: &Ui,
    resp: &Response,
    cx: &ChartCtx,
    hovered: Option<&[usize]>,
) -> Option<ChartAction> {
    let id = resp.id.with("menu_target");
    if resp.secondary_clicked() {
        ui.ctx().data_mut(|d| d.insert_temp(id, hovered.map(<[usize]>::to_vec)));
    }
    let target = ui.ctx().data(|d| d.get_temp::<Option<Vec<usize>>>(id)).flatten()?;
    let node = cx.root.get(&target)?;

    let mut action = None;
    resp.context_menu(|ui| {
        ui.label(egui::RichText::new(&node.name).strong());
        ui.separator();
        if ui.button(tr(cx.lang, Key::ShowInFileManager)).clicked() {
            action = Some(ChartAction::Reveal(target.clone()));
            ui.close();
        }
        if ui.button(tr(cx.lang, Key::MoreInfo)).clicked() {
            action = Some(ChartAction::Info(target.clone()));
            ui.close();
        }
        ui.separator();
        if ui.button(tr(cx.lang, Key::MoveToTrash)).clicked() {
            action = Some(ChartAction::Trash(target.clone()));
            ui.close();
        }
        let delete = egui::RichText::new(tr(cx.lang, Key::DeletePermanently))
            .color(Color32::from_rgb(232, 84, 74));
        if ui.button(delete).clicked() {
            action = Some(ChartAction::Delete(target.clone()));
            ui.close();
        }
    });
    action
}
