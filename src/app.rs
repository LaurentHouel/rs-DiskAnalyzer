use crate::chart::{ChartAction, ChartCtx};
use crate::drives::{self, Drive};
use crate::icons::Icon;
use crate::scan::{self, Node, ScanHandle};
use crate::i18n::{Key, Lang, format_age, format_duration, format_items, format_size, tr, trf};
use crate::util::{delete_permanently, reveal_in_file_manager};
use crate::widgets::{Btn, Crumb, breadcrumb, divider, footer, status_pill, tool_button};
use crate::{rings, theme, treemap};
use egui::{
    Align2, CentralPanel, Color32, CornerRadius, FontId, Panel, Rect, ScrollArea, Sense, Shape,
    Stroke, StrokeKind, Ui, pos2, vec2,
};
use std::collections::HashSet;
use std::path::{Component, PathBuf};
use std::sync::atomic::Ordering;

const MAX_RECENT: usize = 5;
const REPO_URL: &str = "https://github.com/LaurentHouel/rs-DiskAnalyzer";
const CARD_W: f32 = 250.0;
const CARD_H: f32 = 84.0;
const CARD_GAP: f32 = 10.0;
const ROW_H: f32 = 26.0;
const INDENT: f32 = 16.0;
const BAR_W: f32 = 56.0;
const DEFAULT_COLS: [f32; 3] = [100.0, 150.0, 100.0];
const COL_MIN: f32 = 60.0;
const NAME_COL_MIN: f32 = 160.0;

#[derive(PartialEq, Clone, Copy)]
enum ChartKind {
    Rings,
    Treemap,
}

#[derive(PartialEq, Clone, Copy)]
enum SortKey {
    Name,
    Size,
    Items,
    Modified,
}

enum ListAction {
    Select(Vec<usize>),
    Trash(Vec<usize>),
    Toggle(Vec<usize>),
    Open(Vec<usize>),
    Reveal(Vec<usize>),
    CopyPath(Vec<usize>),
    Delete(Vec<usize>),
    Info(Vec<usize>),
}

struct Row<'a> {
    node: &'a Node,
    path: Vec<usize>,
    depth: usize,
    parent_size: u64,
}

pub struct App {
    root: Option<Node>,
    root_path: PathBuf,
    /// Index path (from the scan root) of the directory currently shown.
    view: Vec<usize>,
    selected: Option<Vec<usize>>,
    expanded: HashSet<Vec<usize>>,
    scan: Option<ScanHandle>,
    /// How long the last completed scan took.
    scan_time: Option<std::time::Duration>,
    chart: ChartKind,
    scroll_to_selected: bool,
    lang: Lang,
    sort_key: SortKey,
    sort_desc: bool,
    pending_trash: Option<Vec<usize>>,
    pending_delete: Option<Vec<usize>>,
    info: Option<Vec<usize>>,
    /// Last status message; the flag is true for errors.
    message: Option<(bool, String)>,
    recent: Vec<PathBuf>,
    drives: Vec<Drive>,
    /// Widths of the Size, Contents and Modified columns.
    col_widths: [f32; 3],
    dark: bool,
    logo: Option<egui::TextureHandle>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, start: Option<PathBuf>) -> Self {
        let lang = cc
            .storage
            .and_then(|s| s.get_string("lang"))
            .and_then(|code| Lang::from_code(&code))
            .unwrap_or(Lang::En);
        let recent = cc
            .storage
            .and_then(|s| s.get_string("recent"))
            .map(|r| r.lines().map(PathBuf::from).collect())
            .unwrap_or_default();
        let mut app = Self::empty(lang, recent);
        app.dark = cc.storage.and_then(|s| s.get_string("dark")).is_none_or(|d| d == "1");
        theme::apply(&cc.egui_ctx, app.dark);
        if let Some(w) = cc.storage.and_then(|s| s.get_string("cols")).and_then(|c| parse_widths(&c)) {
            app.col_widths = w;
        }
        if let Some(start) = start {
            app.start_scan(&cc.egui_ctx, start);
        }
        app
    }

    fn empty(lang: Lang, recent: Vec<PathBuf>) -> Self {
        Self {
            root: None,
            root_path: PathBuf::new(),
            view: Vec::new(),
            selected: None,
            expanded: HashSet::new(),
            scan: None,
            scan_time: None,
            chart: ChartKind::Treemap,
            scroll_to_selected: false,
            lang,
            sort_key: SortKey::Size,
            sort_desc: true,
            pending_trash: None,
            pending_delete: None,
            info: None,
            message: None,
            recent,
            drives: drives::list(),
            col_widths: DEFAULT_COLS,
            dark: true,
            logo: None,
        }
    }

    fn start_scan(&mut self, ctx: &egui::Context, path: PathBuf) {
        if let Some(old) = self.scan.take() {
            old.cancel.store(true, Ordering::Relaxed);
        }
        self.message = None;
        self.pending_trash = None;
        self.pending_delete = None;
        self.info = None;
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path.clone());
        self.recent.truncate(MAX_RECENT);
        self.scan = Some(scan::start(path, ctx.clone()));
    }

    /// Back to the selection screen: drops the current result and cancels any running scan.
    fn go_to_start_screen(&mut self) {
        if let Some(scan) = self.scan.take() {
            scan.cancel.store(true, Ordering::Relaxed);
        }
        self.root = None;
        self.scan_time = None;
        self.root_path = PathBuf::new();
        self.view.clear();
        self.selected = None;
        self.expanded.clear();
        self.pending_trash = None;
        self.pending_delete = None;
        self.info = None;
        self.message = None;
        self.drives = drives::list();
    }

    fn poll_scan(&mut self) {
        let Some(scan) = &self.scan else { return };
        if let Ok(root) = scan.rx.try_recv() {
            self.root_path = scan.path.clone();
            self.root = Some(root);
            self.view.clear();
            self.selected = None;
            self.expanded.clear();
            self.scan_time = Some(scan.started.elapsed());
            self.scan = None;
        }
    }

    fn go_up(&mut self) {
        if let Some(old) = self.view.pop() {
            let mut sel = self.view.clone();
            sel.push(old);
            self.selected = Some(sel);
            self.scroll_to_selected = true;
        }
    }

    fn select(&mut self, path: Vec<usize>, scroll: bool) {
        for n in self.view.len() + 1..path.len() {
            self.expanded.insert(path[..n].to_vec());
        }
        self.selected = Some(path);
        self.scroll_to_selected = scroll;
    }

    fn open(&mut self, path: Vec<usize>) {
        if self.root.as_ref().and_then(|r| r.get(&path)).is_some_and(|n| n.is_dir) {
            self.view = path;
            self.selected = None;
        }
    }

    fn modal_open(&self) -> bool {
        self.pending_trash.is_some() || self.pending_delete.is_some() || self.info.is_some()
    }

    /// Confirmation for moving to the trash, or for deleting permanently when `permanent`.
    fn removal_dialog(&mut self, ctx: &egui::Context, permanent: bool) {
        let pending = if permanent { &self.pending_delete } else { &self.pending_trash };
        let Some(path) = pending.clone() else { return };
        let Some(node) = self.root.as_ref().and_then(|r| r.get(&path)) else {
            self.pending_trash = None;
            self.pending_delete = None;
            return;
        };
        let lang = self.lang;
        let (title, confirm_key) = if permanent {
            (tr(lang, Key::DeletePermanently), Key::ConfirmDelete)
        } else {
            (tr(lang, Key::MoveToTrash), Key::ConfirmTrash)
        };
        let text = trf(lang, confirm_key, &[&node.name, &format_size(lang, node.size)]);
        let mut confirmed = false;
        let id = egui::Id::unique(if permanent { "delete_modal" } else { "trash_modal" });
        let modal = egui::Modal::new(id).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading(title);
            ui.add_space(6.0);
            ui.label(text);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let label = if permanent {
                    egui::RichText::new(title).color(Color32::from_rgb(232, 84, 74))
                } else {
                    egui::RichText::new(title)
                };
                if ui.button(label).clicked() {
                    confirmed = true;
                }
                if ui.button(tr(lang, Key::Cancel)).clicked() {
                    ui.close();
                }
            });
        });
        if confirmed {
            self.pending_trash = None;
            self.pending_delete = None;
            self.remove(&path, permanent);
        } else if modal.should_close() {
            self.pending_trash = None;
            self.pending_delete = None;
        }
    }

    fn info_dialog(&mut self, ctx: &egui::Context) {
        let Some(path) = self.info.clone() else { return };
        let Some(root) = &self.root else {
            self.info = None;
            return;
        };
        let Some(node) = root.get(&path) else {
            self.info = None;
            return;
        };
        let lang = self.lang;
        let full_path = root.path_of(&self.root_path, &path);
        let percent = |part: u64, whole: u64| {
            let value = if whole == 0 { 0.0 } else { part as f64 * 100.0 / whole as f64 };
            let text = format!("{value:.1} %");
            if lang == Lang::En { text.replace(" %", "%") } else { text.replace('.', ",") }
        };
        let parent_size = path
            .split_last()
            .and_then(|(_, parent)| root.get(parent))
            .map_or(node.size, |p| p.size);
        let kind = if node.is_dir { tr(lang, Key::Folder) } else { tr(lang, Key::KindFile) };
        let mut rows = vec![
            (tr(lang, Key::InfoPath), full_path.display().to_string()),
            (tr(lang, Key::InfoType), kind.to_owned()),
            (tr(lang, Key::Size), {
                let human = format_size(lang, node.size);
                if node.size < 1000 {
                    human
                } else {
                    format!("{human} ({} {})", node.size, tr(lang, Key::Bytes))
                }
            }),
        ];
        if node.is_dir {
            rows.push((tr(lang, Key::Contents), format_items(lang, node.items)));
        }
        rows.push((tr(lang, Key::Modified), format_age(lang, node.modified)));
        rows.push((tr(lang, Key::ShareOfParent), percent(node.size, parent_size)));
        rows.push((tr(lang, Key::ShareOfScan), percent(node.size, root.size)));
        let title = node.name.clone();

        let mut reveal = false;
        let modal = egui::Modal::new(egui::Id::unique("info_modal")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.heading(title);
            ui.add_space(8.0);
            egui::Grid::new("info_grid").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                for (label, value) in &rows {
                    ui.label(egui::RichText::new(*label).weak());
                    ui.add(egui::Label::new(value).wrap());
                    ui.end_row();
                }
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(tr(lang, Key::ShowInFileManager)).clicked() {
                    reveal = true;
                }
                if ui.button(tr(lang, Key::Close)).clicked() {
                    ui.close();
                }
            });
        });
        if reveal {
            reveal_in_file_manager(&full_path);
        }
        if modal.should_close() {
            self.info = None;
        }
    }

    fn remove(&mut self, path: &[usize], permanent: bool) {
        let Some(root) = &mut self.root else { return };
        let target = root.path_of(&self.root_path, path);
        let name = root.get(path).map(|n| n.name.clone()).unwrap_or_default();
        let result = if permanent {
            delete_permanently(&target).map_err(|e| e.to_string())
        } else {
            trash::delete(&target).map_err(|e| e.to_string())
        };
        if let Err(e) = result {
            let key = if permanent { Key::DeleteFailed } else { Key::TrashFailed };
            self.message = Some((true, trf(self.lang, key, &[&e])));
            return;
        }

        // Removing a node re-sorts its ancestors, so index paths are rebuilt from filesystem paths.
        let root_path = &self.root_path;
        let view_path = root.path_of(root_path, &self.view);
        let selected = self.selected.as_ref().map(|s| root.path_of(root_path, s));
        let expanded: Vec<PathBuf> = self.expanded.iter().map(|e| root.path_of(root_path, e)).collect();

        root.remove(path);

        let mut view_path = view_path;
        self.view = loop {
            if let Some(v) = root.resolve(root_path, &view_path) {
                break v;
            }
            if !view_path.pop() {
                break Vec::new();
            }
        };
        self.selected = selected.and_then(|s| root.resolve(root_path, &s));
        self.expanded = expanded
            .iter()
            .filter_map(|e| root.resolve(root_path, e))
            .collect();
        let key = if permanent { Key::Deleted } else { Key::Trashed };
        self.message = Some((false, trf(self.lang, key, &[&name])));
    }

    /// Progress page shown for every scan, including a rescan over an existing result.
    /// Returns true when the user asked to cancel.
    fn scanning_screen(&self, ui: &mut Ui) -> bool {
        let Some(scan) = &self.scan else { return false };
        let lang = self.lang;
        let pal = theme::Palette::new(ui.visuals().dark_mode);
        let mut cancelled = false;
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 3.0);
            ui.spinner();
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(trf(lang, Key::ScanningPath, &[&scan.path.display().to_string()]))
                    .size(16.0)
                    .strong(),
            );
            let p = &scan.progress;
            ui.label(
                egui::RichText::new(trf(
                    lang,
                    Key::ScanDetails,
                    &[
                        &p.dirs.load(Ordering::Relaxed).to_string(),
                        &p.files.load(Ordering::Relaxed).to_string(),
                        &format_size(lang, p.bytes.load(Ordering::Relaxed)),
                        &format!("{:.0}", scan.started.elapsed().as_secs_f32()),
                    ],
                ))
                .weak(),
            );
            ui.add_space(14.0);
            let cancel = Btn::icon(Icon::Close, tr(lang, Key::Cancel)).with_text(tr(lang, Key::Cancel));
            cancelled = tool_button(ui, &pal, true, cancel).clicked();
        });
        cancelled
    }

    /// Start page: nothing is scanned until the user picks a folder.
    /// The layout follows the window size and scrolls when the window is short.
    fn welcome_screen(&mut self, ui: &mut Ui) {
        let lang = self.lang;
        let pal = theme::Palette::new(ui.visuals().dark_mode);
        let logo = self.logo_texture(ui.ctx());
        let compact = ui.available_height() < 560.0;
        let mut chosen = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let full = ui.available_width();
            let w = (full - 32.0).clamp(240.0, 880.0);
            ui.horizontal_top(|ui| {
                ui.add_space(((full - w) / 2.0).max(0.0));
                ui.vertical(|ui| {
                    ui.set_width(w);
                    ui.add_space(if compact { 6.0 } else { 24.0 });

                    let margin = if compact { 16.0 } else { 26.0 };
                    egui::Frame::new()
                        .fill(pal.hero_fill)
                        .stroke(Stroke::new(1.0, pal.hero_stroke))
                        .corner_radius(CornerRadius::same(18))
                        .inner_margin(egui::Margin::same(margin as i8))
                        .show(ui, |ui| {
                            let inner = w - 2.0 * margin - 2.0;
                            ui.set_width(inner);
                            let logo_size = if compact { 84.0 } else { 128.0 };
                            let show_logo = inner >= 460.0;
                            let text_w = if show_logo { inner - logo_size - 16.0 } else { inner };
                            if !compact {
                                ui.set_min_height(logo_size + 4.0);
                            }
                            let right = ui.max_rect().left() + inner;
                            let title_size = if compact || inner < 400.0 { 22.0 } else { 28.0 };
                            let left = ui.vertical(|ui| {
                                ui.set_max_width(text_w);
                                ui.heading(
                                    egui::RichText::new(tr(lang, Key::WelcomeTitle))
                                        .size(title_size)
                                        .strong()
                                        .color(pal.hero_title),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(tr(lang, Key::WelcomeHint))
                                        .size(if compact { 13.5 } else { 15.0 })
                                        .color(pal.hero_text),
                                );
                                ui.add_space(if compact { 10.0 } else { 18.0 });
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                                    let open = Btn::icon(Icon::Folder, tr(lang, Key::OpenTip))
                                        .with_text(tr(lang, Key::Open))
                                        .icon_color(pal.accent);
                                    if tool_button(ui, &pal, true, open).clicked() {
                                        chosen = rfd::FileDialog::new().pick_folder();
                                    }
                                    let home = Btn::icon(Icon::Home, tr(lang, Key::HomeTip))
                                        .with_text(tr(lang, Key::HomeTip));
                                    if tool_button(ui, &pal, true, home).clicked() {
                                        chosen = dirs::home_dir();
                                    }
                                });
                            });
                            if show_logo {
                                let center = pos2(right - logo_size / 2.0, left.response.rect.center().y);
                                ui.painter().image(
                                    logo,
                                    Rect::from_center_size(center, vec2(logo_size, logo_size)),
                                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            }
                        });

                    if !self.drives.is_empty() {
                        ui.add_space(if compact { 14.0 } else { 24.0 });
                        let card_w = CARD_W.min(w);
                        let per_row = (((w + CARD_GAP) / (card_w + CARD_GAP)) as usize).clamp(1, 4);
                        let block = per_row.min(self.drives.len()) as f32 * (card_w + CARD_GAP) - CARD_GAP;
                        ui.horizontal(|ui| {
                            ui.add_space(((w - block) / 2.0).max(0.0));
                            ui.label(egui::RichText::new(tr(lang, Key::Drives)).size(17.0).strong());
                            let tip = tr(lang, Key::RescanTip);
                            if tool_button(ui, &pal, true, Btn::icon(Icon::Refresh, tip))
                                .on_hover_text(tip)
                                .clicked()
                            {
                                self.drives = drives::list();
                            }
                        });
                        ui.add_space(6.0);
                        for row in self.drives.chunks(per_row) {
                            let used = row.len() as f32 * (card_w + CARD_GAP) - CARD_GAP;
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = CARD_GAP;
                                ui.add_space(((w - used) / 2.0).max(0.0));
                                for drive in row {
                                    if drive_card(ui, lang, drive, card_w) {
                                        chosen = Some(drive.mount.clone());
                                    }
                                }
                            });
                        }
                    }

                    if !self.recent.is_empty() {
                        ui.add_space(if compact { 14.0 } else { 24.0 });
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new(tr(lang, Key::Recent)).size(17.0).strong());
                        });
                        for path in &self.recent {
                            let label = path.display().to_string();
                            let recent = egui::Button::new(label.clone())
                                .fill(pal.btn_fill)
                                .stroke(Stroke::new(1.0, pal.btn_border))
                                .corner_radius(CornerRadius::same(9))
                                .truncate();
                            if ui.add_sized(vec2(w, 36.0), recent).on_hover_text(&label).clicked() {
                                chosen = Some(path.clone());
                            }
                        }
                    }
                    ui.add_space(16.0);
                });
            });
        });
        if let Some(path) = chosen {
            self.start_scan(ui.ctx(), path);
        }
    }

    /// Path of the folder shown by the charts, one element per folder. Folders above the scan root
    /// are listed for context but cannot be opened; the others navigate inside the scan.
    fn crumbs(&self) -> Vec<Crumb> {
        let Some(root) = &self.root else { return Vec::new() };
        let mut crumbs: Vec<Crumb> = Vec::new();
        for component in self.root_path.components() {
            let label = match component {
                Component::Prefix(p) => p.as_os_str().to_string_lossy().into_owned(),
                Component::RootDir if crumbs.is_empty() => std::path::MAIN_SEPARATOR.to_string(),
                Component::Normal(n) => n.to_string_lossy().into_owned(),
                _ => continue,
            };
            crumbs.push(Crumb { label, depth: None, current: false });
        }
        if let Some(scan_root) = crumbs.last_mut() {
            scan_root.depth = Some(0);
        }
        for i in 1..=self.view.len() {
            let Some(node) = root.get(&self.view[..i]) else { break };
            crumbs.push(Crumb { label: node.name.clone(), depth: Some(i), current: false });
        }
        if let Some(last) = crumbs.last_mut() {
            last.current = true;
        }
        crumbs
    }

    /// Shows the folder at `depth` levels below the scan root.
    fn navigate_to(&mut self, depth: usize) {
        if depth < self.view.len() {
            self.view.truncate(depth);
            self.selected = None;
        }
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let lang = self.lang;
        let pal = theme::Palette::new(ui.visuals().dark_mode);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let tip = tr(lang, Key::ParentTip);
            if tool_button(ui, &pal, !self.view.is_empty(), Btn::icon(Icon::Back, tip))
                .on_hover_text(tip)
                .clicked()
            {
                self.go_up();
            }
            let tip = tr(lang, Key::StartScreenTip);
            if tool_button(ui, &pal, true, Btn::icon(Icon::Home, tip)).on_hover_text(tip).clicked() {
                self.go_to_start_screen();
            }
            let open = Btn::icon(Icon::Folder, tr(lang, Key::OpenTip))
                .with_text(tr(lang, Key::Open))
                .icon_color(pal.accent);
            if tool_button(ui, &pal, true, open).on_hover_text(tr(lang, Key::OpenTip)).clicked()
                && let Some(dir) = rfd::FileDialog::new().pick_folder()
            {
                self.start_scan(&ctx, dir);
            }
            let rescan_path = self.scan.as_ref().map_or(self.root_path.clone(), |s| s.path.clone());
            let tip = tr(lang, Key::RescanTip);
            if tool_button(ui, &pal, !rescan_path.as_os_str().is_empty(), Btn::icon(Icon::Refresh, tip))
                .on_hover_text(tip)
                .clicked()
            {
                self.start_scan(&ctx, rescan_path);
            }
            divider(ui, &pal);
            let can_trash = self.root.is_some() && self.selected.is_some();
            let trash = Btn::icon(Icon::Trash, tr(lang, Key::TrashTip)).with_text(tr(lang, Key::MoveToTrash));
            if tool_button(ui, &pal, can_trash, trash)
                .on_hover_text(tr(lang, Key::TrashTip))
                .clicked()
            {
                self.pending_trash = self.selected.clone();
            }

            // Keep room for the right-hand group (language and theme buttons).
            let crumbs = self.crumbs();
            if let Some(depth) = breadcrumb(ui, &pal, &crumbs, (ui.available_width() - 230.0).max(0.0)) {
                self.navigate_to(depth);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (icon, tip) = (if self.dark { Icon::Sun } else { Icon::Moon }, tr(lang, Key::ThemeTip));
                if tool_button(ui, &pal, true, Btn::icon(icon, tip).icon_color(pal.theme_icon))
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.dark = !self.dark;
                    theme::apply(&ctx, self.dark);
                }

                let language = Btn::icon(Icon::Globe, tr(lang, Key::Language))
                    .with_text(self.lang.native_name())
                    .trailing(Icon::ChevronDown);
                let button = tool_button(ui, &pal, true, language);
                egui::Popup::menu(&button).align(egui::RectAlign::BOTTOM_END).gap(6.0).show(|ui| {
                    ui.set_min_width(150.0);
                    for l in Lang::ALL {
                        if ui.selectable_label(self.lang == l, l.native_name()).clicked() {
                            self.lang = l;
                        }
                    }
                });
            });
        });
        if let Some((is_error, text)) = &self.message {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                let color = if *is_error {
                    Color32::from_rgb(230, 80, 70)
                } else {
                    ui.visuals().weak_text_color()
                };
                ui.colored_label(color, text);
            });
        }
    }

    fn logo_texture(&mut self, ctx: &egui::Context) -> egui::TextureId {
        self.logo
            .get_or_insert_with(|| {
                let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png"))
                    .expect("embedded icon is a valid PNG");
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [icon.width as usize, icon.height as usize],
                    &icon.rgba,
                );
                ctx.load_texture("logo", image, egui::TextureOptions::LINEAR)
            })
            .id()
    }

    /// Title row: app logo and name on the left, scan status on the right. On macOS it replaces the
    /// native title bar, so it also drags and zooms the window.
    fn title_row(&mut self, ui: &mut Ui) {
        let lang = self.lang;
        let pal = theme::Palette::new(ui.visuals().dark_mode);
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click_and_drag());
        if resp.drag_started_by(egui::PointerButton::Primary) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if resp.double_clicked() {
            let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }

        let logo_id = self.logo_texture(ui.ctx());

        let cy = rect.center().y;
        let mut x = rect.left();
        if cfg!(target_os = "macos") {
            // Room for the native window buttons.
            ui.painter().vline(rect.left() + 62.0, (cy - 8.0)..=(cy + 8.0), Stroke::new(1.0, pal.btn_border));
            x += 76.0;
        }
        let logo_rect = Rect::from_center_size(pos2(x + 14.0, cy), vec2(32.0, 32.0));
        ui.painter().image(
            logo_id,
            logo_rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        ui.painter().text(
            pos2(logo_rect.right() + 4.0, cy),
            Align2::LEFT_CENTER,
            "Disk Analyzer",
            FontId::proportional(15.0),
            ui.visuals().strong_text_color(),
        );

        let (text, dot) = if let Some(scan) = &self.scan {
            let p = &scan.progress;
            (
                trf(
                    lang,
                    Key::StatusScanning,
                    &[
                        &p.files.load(Ordering::Relaxed).to_string(),
                        &format_size(lang, p.bytes.load(Ordering::Relaxed)),
                    ],
                ),
                pal.warn,
            )
        } else if let Some(root) = &self.root {
            let time = self.scan_time.map(|d| format_duration(lang, d)).unwrap_or_default();
            let text = trf(lang, Key::StatusDone, &[&format_size(lang, root.size), &time]);
            (text.trim_end_matches([' ', '·']).to_owned(), pal.ok)
        } else {
            (tr(lang, Key::StatusIdle).to_owned(), pal.ok)
        };
        // While scanning, the Cancel button sits to the right of the progress pill.
        let mut pill_right = rect.right();
        let mut cancelled = false;
        if self.scan.is_some() {
            let cancel = Btn::icon(Icon::Close, tr(lang, Key::Cancel)).with_text(tr(lang, Key::Cancel));
            let layout = egui::Layout::right_to_left(egui::Align::Center);
            let button = ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(layout), |ui| {
                tool_button(ui, &pal, true, cancel)
            });
            cancelled = button.inner.clicked();
            pill_right = button.inner.rect.left() - 8.0;
        }
        status_pill(ui, &pal, pos2(pill_right, cy), &text, dot);
        if cancelled {
            // Cancelling abandons the scan and goes back to the start screen.
            self.go_to_start_screen();
        }
    }

    fn header(&mut self, ui: &mut Ui) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
        let cols = Columns::new(rect, self.col_widths);
        let spans = [
            (SortKey::Name, Key::Folder, rect.left(), cols.name_right, cols.name_right),
            (SortKey::Size, Key::Size, cols.name_right, cols.size_right, cols.size_right),
            (SortKey::Items, Key::Contents, cols.size_right, cols.items_right, cols.items_right),
            (SortKey::Modified, Key::Modified, cols.items_right, rect.right(), cols.date_right),
        ];
        let font = FontId::proportional(12.0);
        for (i, (key, label, x0, x1, anchor)) in spans.into_iter().enumerate() {
            let hit = Rect::from_min_max(pos2(x0, rect.top()), pos2(x1, rect.bottom()));
            let resp = ui.interact(hit, ui.scope_id().with(("sort_header", i)), Sense::click());
            let active = self.sort_key == key;
            let color = if active || resp.hovered() {
                ui.visuals().strong_text_color()
            } else {
                ui.visuals().weak_text_color()
            };
            let y = rect.center().y;
            let left_aligned = key == SortKey::Name;
            let (pos, align) = if left_aligned {
                (pos2(x0 + 6.0, y), Align2::LEFT_CENTER)
            } else {
                (pos2(anchor, y), Align2::RIGHT_CENTER)
            };
            let text_rect = ui.painter().text(pos, align, tr(self.lang, label), font.clone(), color);
            if active {
                let c = if left_aligned {
                    pos2(text_rect.right() + 9.0, y)
                } else {
                    pos2(text_rect.left() - 9.0, y)
                };
                let pts = if self.sort_desc {
                    vec![c + vec2(-3.5, -2.0), c + vec2(3.5, -2.0), c + vec2(0.0, 3.0)]
                } else {
                    vec![c + vec2(-3.5, 2.0), c + vec2(3.5, 2.0), c + vec2(0.0, -3.0)]
                };
                ui.painter().add(Shape::convex_polygon(pts, color, Stroke::NONE));
            }
            if resp.clicked() {
                if active {
                    self.sort_desc = !self.sort_desc;
                } else {
                    self.sort_key = key;
                    self.sort_desc = key != SortKey::Name;
                }
            }
        }

        // Drag handles on the left edge of the Size, Contents and Modified columns.
        let max_total = rect.width() - 8.0 - NAME_COL_MIN;
        for (i, x) in [cols.name_right, cols.size_right, cols.items_right].into_iter().enumerate() {
            let hit = Rect::from_min_max(pos2(x - 4.0, rect.top()), pos2(x + 4.0, rect.bottom()));
            let resp = ui
                .interact(hit, ui.scope_id().with(("col_resize", i)), Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            if resp.dragged() {
                let others: f32 = self.col_widths.iter().sum::<f32>() - self.col_widths[i];
                let max = (max_total - others).max(COL_MIN);
                self.col_widths[i] = (self.col_widths[i] - resp.drag_delta().x).clamp(COL_MIN, max);
            }
            if resp.double_clicked() {
                self.col_widths[i] = DEFAULT_COLS[i];
            }
            let active = resp.hovered() || resp.dragged();
            let color = if active {
                ui.visuals().selection.stroke.color
            } else {
                ui.visuals().widgets.noninteractive.bg_stroke.color
            };
            ui.painter().vline(x, rect.y_range().shrink(4.0), Stroke::new(if active { 2.0 } else { 1.0 }, color));
        }
    }

    fn list(&mut self, ui: &mut Ui) {
        self.header(ui);
        let Some(root) = &self.root else { return };
        let Some(view) = root.get(&self.view) else { return };
        let lang = self.lang;

        let mut rows = Vec::new();
        flatten(
            view,
            &mut self.view.clone(),
            0,
            &self.expanded,
            (self.sort_key, self.sort_desc),
            &mut rows,
        );

        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
        let cols = Columns::new(rect, self.col_widths);
        let accent = theme::Palette::new(ui.visuals().dark_mode).accent;
        let p = ui.painter();
        let y = rect.center().y;
        let strong = ui.visuals().strong_text_color();
        p.text(
            pos2(rect.left() + 6.0, y),
            Align2::LEFT_CENTER,
            &view.name,
            FontId::proportional(14.0),
            accent,
        );
        let normal = ui.visuals().text_color();
        let f = FontId::proportional(13.0);
        p.text(pos2(cols.size_right, y), Align2::RIGHT_CENTER, format_size(lang, view.size), f.clone(), strong);
        p.text(pos2(cols.items_right, y), Align2::RIGHT_CENTER, format_items(lang, view.items), f.clone(), normal);
        p.text(pos2(cols.date_right, y), Align2::RIGHT_CENTER, format_age(lang, view.modified), f, normal);

        let mut action = None;
        let mut area = ScrollArea::vertical().auto_shrink([false, false]);
        let scroll_target = self
            .selected
            .as_ref()
            .filter(|_| self.scroll_to_selected)
            .and_then(|sel| rows.iter().position(|r| &r.path == sel));
        if let Some(idx) = scroll_target {
            let offset = (idx as f32 * ROW_H - ui.available_height() / 2.0).max(0.0);
            area = area.vertical_scroll_offset(offset);
        }

        let output = ui
            .scope(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                area.show_rows(ui, ROW_H, rows.len(), |ui, range| {
                    for row in &rows[range.clone()] {
                        if let Some(a) = draw_row(
                            ui,
                            lang,
                            row,
                            self.selected.as_ref(),
                            &self.expanded,
                            self.col_widths,
                        ) {
                            action = Some(a);
                        }
                    }
                    range
                })
            })
            .inner;
        if self.scroll_to_selected {
            self.scroll_to_selected = scroll_target.is_some_and(|idx| !output.inner.contains(&idx));
            if self.scroll_to_selected {
                ui.ctx().request_repaint();
            }
        }

        let root_path = self.root_path.clone();
        match action {
            Some(ListAction::Select(p)) => self.select(p, false),
            Some(ListAction::Toggle(p)) => {
                if !self.expanded.remove(&p) {
                    self.expanded.insert(p);
                }
            }
            Some(ListAction::Open(p)) => self.open(p),
            Some(ListAction::Trash(p)) => self.pending_trash = Some(p),
            Some(ListAction::Delete(p)) => self.pending_delete = Some(p),
            Some(ListAction::Info(p)) => self.info = Some(p),
            Some(ListAction::Reveal(p)) => {
                if let Some(root) = &self.root {
                    reveal_in_file_manager(&root.path_of(&root_path, &p));
                }
            }
            Some(ListAction::CopyPath(p)) => {
                if let Some(root) = &self.root {
                    let text = root.path_of(&root_path, &p).display().to_string();
                    ui.ctx().copy_text(text);
                }
            }
            None => {}
        }
    }

    fn chart_panel(&mut self, ui: &mut Ui) {
        let Some(root) = &self.root else { return };
        let cx = ChartCtx {
            lang: self.lang,
            root,
            root_path: &self.root_path,
            base: &self.view,
            selected: self.selected.as_deref(),
        };
        let action = match self.chart {
            ChartKind::Rings => rings::show(ui, &cx),
            ChartKind::Treemap => treemap::show(ui, &cx),
        };
        match action {
            Some(ChartAction::Select(p)) => {
                self.select(p, true);
                ui.ctx().request_repaint();
            }
            Some(ChartAction::Open(p)) => {
                self.open(p);
                ui.ctx().request_repaint();
            }
            Some(ChartAction::Up) => {
                self.go_up();
                ui.ctx().request_repaint();
            }
            Some(ChartAction::Reveal(p)) => {
                if let Some(root) = &self.root {
                    reveal_in_file_manager(&root.path_of(&self.root_path, &p));
                }
            }
            Some(ChartAction::Trash(p)) => self.pending_trash = Some(p),
            Some(ChartAction::Delete(p)) => self.pending_delete = Some(p),
            Some(ChartAction::Info(p)) => self.info = Some(p),
            None => {}
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("lang", self.lang.code().to_owned());
        storage.set_string("dark", if self.dark { "1" } else { "0" }.to_owned());
        let recent: Vec<String> = self.recent.iter().map(|p| p.display().to_string()).collect();
        storage.set_string("recent", recent.join("\n"));
        let cols: Vec<String> = self.col_widths.iter().map(|w| w.to_string()).collect();
        storage.set_string("cols", cols.join(","));
    }
}

impl App {
    fn show(&mut self, ui: &mut Ui) {
        self.poll_scan();
        if self.scan.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        let (go_up, delete) = ui.input(|i| {
            let cmd_backspace = i.modifiers.command && i.key_pressed(egui::Key::Backspace);
            (
                i.key_pressed(egui::Key::Backspace) && !i.modifiers.command,
                i.key_pressed(egui::Key::Delete) || cmd_backspace,
            )
        });
        if go_up && !self.modal_open() {
            self.go_up();
        }
        if delete && !self.modal_open() && self.root.is_some() {
            self.pending_trash = self.selected.clone();
        }

        let toolbar_frame = egui::Frame::new()
            .fill(ui.visuals().panel_fill)
            .inner_margin(egui::Margin::symmetric(14, 0));
        Panel::top("toolbar").frame(toolbar_frame).show(ui, |ui| {
            self.title_row(ui);
            self.toolbar(ui);
            ui.add_space(12.0);
        });

        // Added before the other panels so it stays the outermost bottom bar.
        let footer_frame = egui::Frame::new()
            .fill(ui.visuals().panel_fill)
            .inner_margin(egui::Margin::symmetric(14, 0));
        let pal = theme::Palette::new(ui.visuals().dark_mode);
        let mut open_repo = false;
        Panel::bottom("footer").frame(footer_frame).show(ui, |ui| {
            open_repo = footer(
                ui,
                &pal,
                &format!("Disk Analyzer v{}", env!("CARGO_PKG_VERSION")),
                tr(self.lang, Key::MadeWith),
                "GitHub",
                tr(self.lang, Key::GithubTip),
            );
        });
        if open_repo {
            ui.ctx().open_url(egui::OpenUrl::new_tab(REPO_URL));
        }

        if self.scan.is_some() || self.root.is_none() {
            let mut cancelled = false;
            CentralPanel::default().show(ui, |ui| {
                if self.scan.is_some() {
                    cancelled = self.scanning_screen(ui);
                } else {
                    self.welcome_screen(ui);
                }
            });
            if cancelled {
                self.go_to_start_screen();
            }
            return;
        }

        Panel::left("list")
            .resizable(true)
            .default_size(720.0)
            .size_range(520.0..=1100.0)
            .show(ui, |ui| self.list(ui));

        Panel::bottom("chart_kind").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.horizontal(|ui| {
                        let w = ui.available_width();
                        ui.add_space((w - 300.0).max(0.0) / 2.0);
                        let (rings, treemap) = (
                            format!("◎ {}", tr(self.lang, Key::Rings)),
                            format!("▦ {}", tr(self.lang, Key::Treemap)),
                        );
                        ui.selectable_value(&mut self.chart, ChartKind::Rings, rings);
                        ui.selectable_value(&mut self.chart, ChartKind::Treemap, treemap);
                    });
                });
            });
            ui.add_space(4.0);
        });

        CentralPanel::default().show(ui, |ui| self.chart_panel(ui));
        self.removal_dialog(ui.ctx(), false);
        self.removal_dialog(ui.ctx(), true);
        self.info_dialog(ui.ctx());
    }
}

fn parse_widths(s: &str) -> Option<[f32; 3]> {
    let v: Vec<f32> = s.split(',').filter_map(|p| p.parse().ok()).collect();
    let w: [f32; 3] = v.try_into().ok()?;
    w.iter().all(|x| (COL_MIN..=1000.0).contains(x)).then_some(w)
}

struct Columns {
    size_right: f32,
    items_right: f32,
    date_right: f32,
    name_right: f32,
}

impl Columns {
    fn new(rect: Rect, widths: [f32; 3]) -> Self {
        let date_right = rect.right() - 8.0;
        let items_right = date_right - widths[2];
        let size_right = items_right - widths[1];
        Self {
            size_right,
            items_right,
            date_right,
            name_right: size_right - widths[0],
        }
    }
}

fn sorted_indices(children: &[Node], (key, desc): (SortKey, bool)) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..children.len()).collect();
    match key {
        SortKey::Name => idx.sort_by_cached_key(|&i| children[i].name.to_lowercase()),
        SortKey::Size => idx.sort_by_key(|&i| children[i].size),
        SortKey::Items => idx.sort_by_key(|&i| children[i].items),
        SortKey::Modified => idx.sort_by_key(|&i| children[i].modified),
    }
    if desc {
        idx.reverse();
    }
    idx
}

fn flatten<'a>(
    node: &'a Node,
    path: &mut Vec<usize>,
    depth: usize,
    expanded: &HashSet<Vec<usize>>,
    sort: (SortKey, bool),
    rows: &mut Vec<Row<'a>>,
) {
    for i in sorted_indices(&node.children, sort) {
        let child = &node.children[i];
        path.push(i);
        rows.push(Row {
            node: child,
            path: path.clone(),
            depth,
            parent_size: node.size,
        });
        if child.is_dir && expanded.contains(path.as_slice()) {
            flatten(child, path, depth + 1, expanded, sort, rows);
        }
        path.pop();
    }
}

fn draw_row(
    ui: &mut Ui,
    lang: Lang,
    row: &Row,
    selected: Option<&Vec<usize>>,
    expanded: &HashSet<Vec<usize>>,
    widths: [f32; 3],
) -> Option<ListAction> {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let cols = Columns::new(rect, widths);
    let visuals = ui.visuals().clone();
    let pal = theme::Palette::new(visuals.dark_mode);
    let node = row.node;
    let is_selected = selected == Some(&row.path);

    let p = ui.painter();
    if is_selected {
        p.rect_filled(rect, CornerRadius::same(5), visuals.selection.bg_fill);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(5), visuals.widgets.hovered.weak_bg_fill);
    }

    let x0 = rect.left() + 6.0 + row.depth as f32 * INDENT;
    let y = rect.center().y;
    let expandable = node.is_dir && !node.children.is_empty();
    if expandable {
        let c = pos2(x0 + 6.0, y);
        let pts = if expanded.contains(&row.path) {
            vec![c + vec2(-4.0, -2.5), c + vec2(4.0, -2.5), c + vec2(0.0, 3.5)]
        } else {
            vec![c + vec2(-2.5, -4.0), c + vec2(-2.5, 4.0), c + vec2(3.5, 0.0)]
        };
        p.add(Shape::convex_polygon(pts, visuals.text_color(), Stroke::NONE));
    }

    // Percentage bar relative to the parent folder.
    let pct = if row.parent_size > 0 {
        node.size as f64 * 100.0 / row.parent_size as f64
    } else {
        0.0
    };
    let bar = Rect::from_min_size(pos2(x0 + 16.0, rect.top() + 4.0), vec2(BAR_W, ROW_H - 8.0));
    p.rect_filled(bar, 3.0, pal.track);
    let fill = Rect::from_min_size(bar.min, vec2(bar.width() * (pct / 100.0) as f32, bar.height()));
    p.rect_filled(fill, 3.0, pal.accent.gamma_multiply(if visuals.dark_mode { 0.6 } else { 1.0 }));
    p.rect_stroke(bar, 3.0, Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color), StrokeKind::Inside);
    p.text(
        pos2(bar.right() - 3.0, y),
        Align2::RIGHT_CENTER,
        format!("{pct:.2}%"),
        FontId::proportional(11.0),
        visuals.text_color(),
    );

    let name_left = bar.right() + 8.0;
    let name_clip = Rect::from_min_max(pos2(name_left, rect.top()), pos2(cols.name_right, rect.bottom()));
    p.with_clip_rect(name_clip).text(
        pos2(name_left, y),
        Align2::LEFT_CENTER,
        &node.name,
        FontId::proportional(13.0),
        visuals.text_color(),
    );

    let f = FontId::proportional(13.0);
    p.text(pos2(cols.size_right, y), Align2::RIGHT_CENTER, format_size(lang, node.size), f.clone(), visuals.text_color());
    p.text(pos2(cols.items_right, y), Align2::RIGHT_CENTER, format_items(lang, node.items), f.clone(), visuals.text_color());
    p.text(pos2(cols.date_right, y), Align2::RIGHT_CENTER, format_age(lang, node.modified), f, visuals.text_color());

    let mut action = None;
    if resp.double_clicked() && node.is_dir {
        action = Some(ListAction::Open(row.path.clone()));
    } else if resp.clicked() {
        let on_arrow = resp
            .interact_pointer_pos()
            .is_some_and(|pos| pos.x < x0 + 16.0 && pos.x >= x0 - 2.0);
        action = Some(if expandable && on_arrow {
            ListAction::Toggle(row.path.clone())
        } else {
            ListAction::Select(row.path.clone())
        });
    }
    resp.context_menu(|ui| {
        if ui.button(tr(lang, Key::ShowInFileManager)).clicked() {
            action = Some(ListAction::Reveal(row.path.clone()));
            ui.close();
        }
        if ui.button(tr(lang, Key::CopyPath)).clicked() {
            action = Some(ListAction::CopyPath(row.path.clone()));
            ui.close();
        }
        if node.is_dir && ui.button(tr(lang, Key::OpenInChart)).clicked() {
            action = Some(ListAction::Open(row.path.clone()));
            ui.close();
        }
        if ui.button(tr(lang, Key::MoreInfo)).clicked() {
            action = Some(ListAction::Info(row.path.clone()));
            ui.close();
        }
        ui.separator();
        if ui.button(tr(lang, Key::MoveToTrash)).clicked() {
            action = Some(ListAction::Trash(row.path.clone()));
            ui.close();
        }
        let delete = egui::RichText::new(tr(lang, Key::DeletePermanently)).color(Color32::from_rgb(232, 84, 74));
        if ui.button(delete).clicked() {
            action = Some(ListAction::Delete(row.path.clone()));
            ui.close();
        }
    });
    action
}

fn drive_card(ui: &mut Ui, lang: Lang, drive: &Drive, width: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, CARD_H), Sense::click());
    let visuals = ui.visuals();
    let pal = theme::Palette::new(visuals.dark_mode);
    let fill = if resp.hovered() {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        pal.card
    };
    let border = if resp.hovered() {
        visuals.selection.stroke.color
    } else {
        visuals.widgets.noninteractive.bg_stroke.color
    };
    let (strong, weak) = (visuals.strong_text_color(), visuals.weak_text_color());

    let ratio = if drive.total > 0 {
        (drive.used() as f64 / drive.total as f64) as f32
    } else {
        0.0
    };
    let usage_color = match ratio {
        r if r >= 0.9 => Color32::from_rgb(232, 84, 74),
        r if r >= 0.75 => Color32::from_rgb(235, 160, 55),
        _ => pal.accent,
    };

    let p = ui.painter();
    p.rect(rect, 12.0, fill, Stroke::new(1.0, border), StrokeKind::Inside);
    drives::paint_icon(
        p,
        Rect::from_min_size(rect.min + vec2(12.0, 16.0), vec2(56.0, 52.0)),
        drive.kind,
        pal.drive_icon,
    );

    let x = rect.left() + 82.0;
    let text_w = rect.right() - 12.0 - x;
    p.with_clip_rect(Rect::from_min_size(pos2(x, rect.top()), vec2(text_w, CARD_H))).text(
        pos2(x, rect.top() + 18.0),
        Align2::LEFT_CENTER,
        &drive.name,
        FontId::proportional(14.0),
        strong,
    );
    let bar = Rect::from_min_size(pos2(x, rect.top() + 34.0), vec2(text_w, 10.0));
    p.rect_filled(bar, 5.0, pal.track);
    p.rect_filled(
        Rect::from_min_size(bar.min, vec2((bar.width() * ratio).max(if ratio > 0.0 { 5.0 } else { 0.0 }), bar.height())),
        5.0,
        usage_color,
    );
    p.with_clip_rect(Rect::from_min_size(pos2(x, rect.top()), vec2(text_w, CARD_H))).text(
        pos2(x, rect.top() + 58.0),
        Align2::LEFT_CENTER,
        trf(
            lang,
            Key::DriveFree,
            &[&format_size(lang, drive.available), &format_size(lang, drive.total)],
        ),
        FontId::proportional(11.5),
        weak,
    );

    resp.on_hover_text(drive.mount.display().to_string()).clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    fn sample_root() -> Node {
        let file = |name: &str, size| Node {
            name: name.into(),
            size,
            items: 1,
            modified: None,
            is_dir: false,
            children: Vec::new(),
        };
        Node {
            name: "/data".into(),
            size: 30,
            items: 2,
            modified: None,
            is_dir: true,
            children: vec![file("big.bin", 20), file("small.bin", 10)],
        }
    }

    #[test]
    fn home_button_returns_to_selection_screen_without_scanning() {
        let mut app = App::empty(Lang::En, Vec::new());
        app.root = Some(sample_root());
        app.root_path = PathBuf::from("/data");
        // The test context has no system font fallback for the toolbar icons.
        let mut harness = Harness::builder()
            .allow_missing_glyphs()
            .with_size(egui::vec2(1100.0, 700.0))
            .build_ui_state(|ui, app: &mut App| app.show(ui), app);
        harness.run();
        assert!(harness.query_by_label("Choose what to scan. Nothing is scanned until you ask.").is_none());

        harness.get_by_label("Back to the selection screen").click();
        harness.run();

        assert!(harness.state().root.is_none());
        assert!(harness.state().scan.is_none());
        harness.get_by_label("Choose what to scan. Nothing is scanned until you ask.");
    }

    #[test]
    fn chart_selection_expands_and_scrolls_the_list_to_the_selected_item() {
        let files = (0..40)
            .map(|i| Node {
                name: format!("file-{i:02}.bin"),
                size: 40 - i,
                items: 1,
                modified: None,
                is_dir: false,
                children: Vec::new(),
            })
            .collect();
        let total_size = (1..=40).sum();
        let mut app = App::empty(Lang::En, Vec::new());
        app.root_path = PathBuf::from("/data");
        app.root = Some(Node {
            name: "/data".into(),
            size: total_size,
            items: 40,
            modified: None,
            is_dir: true,
            children: vec![Node {
                name: "folder".into(),
                size: total_size,
                items: 40,
                modified: None,
                is_dir: true,
                children: files,
            }],
        });
        let mut harness = Harness::builder()
            .allow_missing_glyphs()
            .with_size(egui::vec2(1100.0, 700.0))
            .build_ui_state(|ui, app: &mut App| app.show(ui), app);
        harness.run();

        let point = egui::pos2(1070.0, 585.0);
        harness.hover_at(point);
        harness.run();
        harness.event(egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
        harness.event(egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();

        let selected = harness.state().selected.as_ref().unwrap();
        assert!(selected[1] >= 30, "the chart click should select a low-ranked row");
        assert!(harness.state().scroll_to_selected);

        harness.step();
        assert!(
            !harness.state().scroll_to_selected,
            "the selected row should be visible after scrolling"
        );
        harness.run();

        let app = harness.state();
        let selected = app
            .selected
            .as_ref()
            .expect("chart click should select an item");
        assert_eq!(selected.len(), 2);
        assert!(app.expanded.contains(&selected[..1].to_vec()));
        assert!(
            !app.scroll_to_selected,
            "the list should process the chart's scroll-to-selection request"
        );
    }

    #[test]
    fn dragging_a_header_edge_resizes_the_column() {
        let mut app = App::empty(Lang::En, Vec::new());
        app.root = Some(sample_root());
        app.root_path = PathBuf::from("/data");
        let mut harness = Harness::builder()
            .allow_missing_glyphs()
            .with_size(egui::vec2(1100.0, 700.0))
            .build_ui_state(|ui, app: &mut App| app.show(ui), app);
        harness.run();

        // Probe the header row for a resize handle: press, move left by 30px, release.
        let before = harness.state().col_widths;
        let mut found = false;
        'probe: for y in (90..140).step_by(6) {
            let y = y as f32;
            for x in (250..520).step_by(2) {
                let x = x as f32;
                harness.hover_at(egui::pos2(x, y));
                harness.run();
                harness.drag_at(egui::pos2(x, y));
                harness.run();
                harness.hover_at(egui::pos2(x - 30.0, y));
                harness.run();
                harness.drop_at(egui::pos2(x - 30.0, y));
                harness.run();
                if harness.state().col_widths != before {
                    found = true;
                    break 'probe;
                }
            }
        }
        assert!(found, "no resize handle found");
        let after = harness.state().col_widths;
        assert!(after.iter().zip(before).any(|(a, b)| a > &b));
    }



    #[test]
    fn right_click_on_a_chart_item_opens_its_menu() {
        let mut app = App::empty(Lang::En, Vec::new());
        app.root = Some(sample_root());
        app.root_path = PathBuf::from("/data");
        let mut harness = Harness::builder()
            .allow_missing_glyphs()
            .with_size(egui::vec2(1100.0, 700.0))
            .build_ui_state(|ui, app: &mut App| app.show(ui), app);
        harness.run();

        let point = egui::pos2(900.0, 400.0);
        harness.hover_at(point);
        harness.run();
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.step();
        }
        harness.run();

        for label in ["Show in file manager", "More information…", "Move to trash", "Delete permanently"] {
            assert!(
                harness.query_all_by_label(label).count() > 0,
                "menu entry {label:?} is missing"
            );
        }
        assert!(harness.state().selected.is_some(), "right click should select the item");

        harness.get_by_label("More information…").click();
        harness.run();
        assert!(harness.state().info.is_some());
    }

    #[test]
    fn clicking_a_path_element_navigates_up_the_tree() {
        let leaf = |name: &str| Node {
            name: name.into(),
            size: 10,
            items: 1,
            modified: None,
            is_dir: false,
            children: Vec::new(),
        };
        let dir = |name: &str, children: Vec<Node>| Node {
            name: name.into(),
            size: 10,
            items: 1,
            modified: None,
            is_dir: true,
            children,
        };
        let mut app = App::empty(Lang::En, Vec::new());
        app.root_path = PathBuf::from("/data");
        app.root = Some(dir("/data", vec![dir("outer", vec![dir("inner", vec![leaf("f.bin")])])]));
        app.view = vec![0, 0];
        let mut harness = Harness::builder()
            .allow_missing_glyphs()
            .with_size(egui::vec2(1100.0, 700.0))
            .build_ui_state(|ui, app: &mut App| app.show(ui), app);
        harness.run();

        harness.get_by_label("outer").click();
        harness.run();
        assert_eq!(harness.state().view, vec![0]);

        harness.get_by_label("data").click();
        harness.run();
        assert!(harness.state().view.is_empty());
    }

    #[test]
    fn cancelling_a_scan_returns_to_the_start_screen() {
        let app = App::empty(Lang::En, Vec::new());
        let mut harness = Harness::builder()
            .allow_missing_glyphs()
            .with_size(egui::vec2(1100.0, 700.0))
            .build_ui_state(|ui, app: &mut App| app.show(ui), app);
        let ctx = harness.ctx.clone();
        harness.state_mut().start_scan(&ctx, PathBuf::from("/"));
        harness.step();
        assert!(harness.state().scan.is_some());

        // Cancel is offered both in the title row and on the progress page.
        assert_eq!(harness.query_all_by_label("Cancel").count(), 2);
        harness.query_all_by_label("Cancel").last().unwrap().click();
        harness.run();

        assert!(harness.state().scan.is_none());
        assert!(harness.state().root.is_none());
        harness.get_by_label("Choose what to scan. Nothing is scanned until you ask.");
    }


    #[test]
    fn rescanning_shows_progress_and_cancel_instead_of_the_old_result() {
        let mut app = App::empty(Lang::En, Vec::new());
        app.root = Some(sample_root());
        app.root_path = PathBuf::from("/data");
        let mut harness = Harness::builder()
            .allow_missing_glyphs()
            .with_size(egui::vec2(1100.0, 700.0))
            .build_ui_state(|ui, app: &mut App| app.show(ui), app);
        harness.run();
        assert!(harness.query_by_label_contains("Rings Chart").is_some());

        let ctx = harness.ctx.clone();
        harness.state_mut().start_scan(&ctx, PathBuf::from("/"));
        harness.step();

        assert!(harness.query_by_label_contains("Rings Chart").is_none(), "old result should be hidden");
        assert!(harness.query_by_label_contains("Scanning /").is_some());
        harness.query_all_by_label("Cancel").last().unwrap().click();
        harness.run();
        assert!(harness.state().scan.is_none());
        assert!(harness.state().root.is_none());
    }

    #[test]
    fn stored_column_widths_are_validated() {
        assert_eq!(parse_widths("90,140,110"), Some([90.0, 140.0, 110.0]));
        assert_eq!(parse_widths("5,140,110"), None);
        assert_eq!(parse_widths("90,140"), None);
    }
}
