use crate::chart::{ChartAction, ChartCtx};
use crate::drives::{self, Drive};
use crate::scan::{self, Node, ScanHandle};
use crate::i18n::{Key, Lang, format_age, format_items, format_size, tr, trf};
use crate::util::reveal_in_file_manager;
use crate::{rings, treemap};
use egui::{
    Align2, CentralPanel, Color32, CornerRadius, FontId, Panel, Rect, ScrollArea, Sense, Shape,
    Stroke, StrokeKind, Ui, pos2, vec2,
};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

const MAX_RECENT: usize = 5;
const CARD_W: f32 = 250.0;
const CARD_H: f32 = 84.0;
const CARD_GAP: f32 = 10.0;
const ROW_H: f32 = 24.0;
const INDENT: f32 = 16.0;
const BAR_W: f32 = 56.0;
const COL_SIZE: f32 = 100.0;
const COL_ITEMS: f32 = 150.0;
const COL_DATE: f32 = 100.0;

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
    chart: ChartKind,
    scroll_to_selected: bool,
    lang: Lang,
    sort_key: SortKey,
    sort_desc: bool,
    pending_trash: Option<Vec<usize>>,
    /// Last status message; the flag is true for errors.
    message: Option<(bool, String)>,
    recent: Vec<PathBuf>,
    drives: Vec<Drive>,
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
        let mut app = Self {
            root: None,
            root_path: PathBuf::new(),
            view: Vec::new(),
            selected: None,
            expanded: HashSet::new(),
            scan: None,
            chart: ChartKind::Treemap,
            scroll_to_selected: false,
            lang,
            sort_key: SortKey::Size,
            sort_desc: true,
            pending_trash: None,
            message: None,
            recent,
            drives: drives::list(),
        };
        if let Some(start) = start {
            app.start_scan(&cc.egui_ctx, start);
        }
        app
    }

    fn start_scan(&mut self, ctx: &egui::Context, path: PathBuf) {
        if let Some(old) = self.scan.take() {
            old.cancel.store(true, Ordering::Relaxed);
        }
        self.message = None;
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
        self.root_path = PathBuf::new();
        self.view.clear();
        self.selected = None;
        self.expanded.clear();
        self.pending_trash = None;
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

    fn trash_dialog(&mut self, ctx: &egui::Context) {
        let Some(path) = self.pending_trash.clone() else { return };
        let Some(node) = self.root.as_ref().and_then(|r| r.get(&path)) else {
            self.pending_trash = None;
            return;
        };
        let lang = self.lang;
        let text = trf(lang, Key::ConfirmTrash, &[&node.name, &format_size(lang, node.size)]);
        let mut confirmed = false;
        let modal = egui::Modal::new(egui::Id::unique("trash_modal")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading(tr(lang, Key::MoveToTrash));
            ui.add_space(6.0);
            ui.label(text);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(tr(lang, Key::MoveToTrash)).clicked() {
                    confirmed = true;
                }
                if ui.button(tr(lang, Key::Cancel)).clicked() {
                    ui.close();
                }
            });
        });
        if confirmed {
            self.pending_trash = None;
            self.trash(&path);
        } else if modal.should_close() {
            self.pending_trash = None;
        }
    }

    fn trash(&mut self, path: &[usize]) {
        let Some(root) = &mut self.root else { return };
        let target = root.path_of(&self.root_path, path);
        let name = root.get(path).map(|n| n.name.clone()).unwrap_or_default();
        if let Err(e) = trash::delete(&target) {
            self.message = Some((true, trf(self.lang, Key::TrashFailed, &[&e.to_string()])));
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
        self.message = Some((false, trf(self.lang, Key::Trashed, &[&name])));
    }

    fn scanning_screen(&self, ui: &mut Ui) {
        let Some(scan) = &self.scan else { return };
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 3.0);
            ui.spinner();
            ui.label(trf(self.lang, Key::ScanningPath, &[&scan.path.display().to_string()]));
            let p = &scan.progress;
            ui.label(trf(
                self.lang,
                Key::ScanDetails,
                &[
                    &p.dirs.load(Ordering::Relaxed).to_string(),
                    &p.files.load(Ordering::Relaxed).to_string(),
                    &format_size(self.lang, p.bytes.load(Ordering::Relaxed)),
                    &format!("{:.0}", scan.started.elapsed().as_secs_f32()),
                ],
            ));
        });
    }

    /// Start page: nothing is scanned until the user picks a folder.
    fn welcome_screen(&mut self, ui: &mut Ui) {
        let lang = self.lang;
        let mut chosen = None;
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() / 8.0).min(60.0));
            ui.heading(tr(lang, Key::WelcomeTitle));
            ui.add_space(6.0);
            ui.label(tr(lang, Key::WelcomeHint));
            ui.add_space(18.0);

            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 360.0).max(0.0) / 2.0);
                if ui.button(format!("🏠 {}", tr(lang, Key::HomeTip))).clicked() {
                    chosen = dirs::home_dir();
                }
                if ui.button(format!("📂 {}", tr(lang, Key::Open))).clicked() {
                    chosen = rfd::FileDialog::new().pick_folder();
                }
            });

            if !self.drives.is_empty() {
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 120.0).max(0.0) / 2.0);
                    ui.label(egui::RichText::new(tr(lang, Key::Drives)).strong());
                    if ui.small_button("⟳").on_hover_text(tr(lang, Key::RescanTip)).clicked() {
                        self.drives = drives::list();
                    }
                });
                ui.add_space(6.0);
                let per_row = (((ui.available_width() + CARD_GAP) / (CARD_W + CARD_GAP)) as usize).clamp(1, 4);
                for row in self.drives.chunks(per_row) {
                    let used = row.len() as f32 * (CARD_W + CARD_GAP) - CARD_GAP;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = CARD_GAP;
                        ui.add_space(((ui.available_width() - used) / 2.0).max(0.0));
                        for drive in row {
                            if drive_card(ui, lang, drive) {
                                chosen = Some(drive.mount.clone());
                            }
                        }
                    });
                }
            }

            if !self.recent.is_empty() {
                ui.add_space(20.0);
                ui.label(egui::RichText::new(tr(lang, Key::Recent)).strong());
                for path in &self.recent {
                    if ui.link(path.display().to_string()).clicked() {
                        chosen = Some(path.clone());
                    }
                }
            }
        });
        if let Some(path) = chosen {
            self.start_scan(ui.ctx(), path);
        }
    }

    fn display_path(&self) -> String {
        self.root
            .as_ref()
            .map(|r| r.path_of(&self.root_path, &self.view).display().to_string())
            .unwrap_or_default()
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let lang = self.lang;
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            if ui
                .add_enabled(!self.view.is_empty(), egui::Button::new("⬅"))
                .on_hover_text(tr(lang, Key::ParentTip))
                .clicked()
            {
                self.go_up();
            }
            if ui.button("🏠").on_hover_text(tr(lang, Key::StartScreenTip)).clicked() {
                self.go_to_start_screen();
            }
            if ui
                .button(format!("📂 {}", tr(lang, Key::Open)))
                .on_hover_text(tr(lang, Key::OpenTip))
                .clicked()
                && let Some(dir) = rfd::FileDialog::new().pick_folder()
            {
                self.start_scan(&ctx, dir);
            }
            let rescan_path = self.scan.as_ref().map_or(self.root_path.clone(), |s| s.path.clone());
            if ui
                .add_enabled(!rescan_path.as_os_str().is_empty(), egui::Button::new("⟳"))
                .on_hover_text(tr(lang, Key::RescanTip))
                .clicked()
            {
                self.start_scan(&ctx, rescan_path);
            }
            let can_trash = self.root.is_some() && self.selected.is_some();
            if ui
                .add_enabled(can_trash, egui::Button::new(format!("🗑 {}", tr(lang, Key::MoveToTrash))))
                .on_hover_text(tr(lang, Key::TrashTip))
                .clicked()
            {
                self.pending_trash = self.selected.clone();
            }
            ui.separator();
            ui.label(self.display_path());

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_salt("lang")
                    .selected_text(self.lang.native_name())
                    .show_ui(ui, |ui| {
                        for l in Lang::ALL {
                            ui.selectable_value(&mut self.lang, l, l.native_name());
                        }
                    })
                    .response
                    .on_hover_text(tr(lang, Key::Language));
                if let Some(scan) = &self.scan {
                    if ui.button(tr(lang, Key::Cancel)).clicked() {
                        scan.cancel.store(true, Ordering::Relaxed);
                    }
                    ui.spinner();
                    let p = &scan.progress;
                    ui.label(trf(
                        lang,
                        Key::ScanProgress,
                        &[
                            &p.files.load(Ordering::Relaxed).to_string(),
                            &format_size(lang, p.bytes.load(Ordering::Relaxed)),
                        ],
                    ));
                }
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

    fn header(&mut self, ui: &mut Ui) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
        let cols = Columns::new(rect);
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
        let cols = Columns::new(rect);
        let p = ui.painter();
        let y = rect.center().y;
        let strong = ui.visuals().strong_text_color();
        p.text(
            pos2(rect.left() + 6.0, y),
            Align2::LEFT_CENTER,
            &view.name,
            FontId::proportional(14.0),
            Color32::from_rgb(190, 140, 40),
        );
        let normal = ui.visuals().text_color();
        let f = FontId::proportional(13.0);
        p.text(pos2(cols.size_right, y), Align2::RIGHT_CENTER, format_size(lang, view.size), f.clone(), strong);
        p.text(pos2(cols.items_right, y), Align2::RIGHT_CENTER, format_items(lang, view.items), f.clone(), normal);
        p.text(pos2(cols.date_right, y), Align2::RIGHT_CENTER, format_age(lang, view.modified), f, normal);

        let mut action = None;
        let mut area = ScrollArea::vertical().auto_shrink([false, false]);
        if self.scroll_to_selected {
            if let Some(idx) = self
                .selected
                .as_ref()
                .and_then(|sel| rows.iter().position(|r| &r.path == sel))
            {
                let offset = (idx as f32 * ROW_H - ui.available_height() / 2.0).max(0.0);
                area = area.vertical_scroll_offset(offset);
            }
            self.scroll_to_selected = false;
        }

        area.show_rows(ui, ROW_H, rows.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for row in &rows[range] {
                if let Some(a) = draw_row(ui, lang, row, self.selected.as_ref(), &self.expanded) {
                    action = Some(a);
                }
            }
        });

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
            Some(ChartAction::Select(p)) => self.select(p, true),
            Some(ChartAction::Open(p)) => self.open(p),
            Some(ChartAction::Up) => self.go_up(),
            None => {}
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
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
        if go_up && self.pending_trash.is_none() {
            self.go_up();
        }
        if delete && self.pending_trash.is_none() && self.root.is_some() {
            self.pending_trash = self.selected.clone();
        }

        Panel::top("toolbar").show(ui, |ui| {
            ui.add_space(4.0);
            self.toolbar(ui);
            ui.add_space(4.0);
        });

        if self.root.is_none() {
            CentralPanel::default().show(ui, |ui| {
                if self.scan.is_some() {
                    self.scanning_screen(ui);
                } else {
                    self.welcome_screen(ui);
                }
            });
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
        self.trash_dialog(ui.ctx());
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("lang", self.lang.code().to_owned());
        let recent: Vec<String> = self.recent.iter().map(|p| p.display().to_string()).collect();
        storage.set_string("recent", recent.join("\n"));
    }
}

struct Columns {
    size_right: f32,
    items_right: f32,
    date_right: f32,
    name_right: f32,
}

impl Columns {
    fn new(rect: Rect) -> Self {
        let date_right = rect.right() - 8.0;
        let items_right = date_right - COL_DATE;
        let size_right = items_right - COL_ITEMS;
        Self {
            size_right,
            items_right,
            date_right,
            name_right: size_right - COL_SIZE,
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
) -> Option<ListAction> {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let cols = Columns::new(rect);
    let visuals = ui.visuals().clone();
    let node = row.node;
    let is_selected = selected == Some(&row.path);

    let p = ui.painter();
    if is_selected {
        p.rect_filled(rect, CornerRadius::ZERO, visuals.selection.bg_fill);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::ZERO, visuals.widgets.hovered.weak_bg_fill);
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
    p.rect_filled(bar, 3.0, visuals.faint_bg_color.gamma_multiply(2.0));
    let fill = Rect::from_min_size(bar.min, vec2(bar.width() * (pct / 100.0) as f32, bar.height()));
    p.rect_filled(fill, 3.0, Color32::from_rgb(52, 120, 220));
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
        ui.separator();
        if ui.button(tr(lang, Key::MoveToTrash)).clicked() {
            action = Some(ListAction::Trash(row.path.clone()));
            ui.close();
        }
    });
    action
}

fn drive_card(ui: &mut Ui, lang: Lang, drive: &Drive) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(CARD_W, CARD_H), Sense::click());
    let visuals = ui.visuals();
    let fill = if resp.hovered() {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        visuals.widgets.inactive.weak_bg_fill
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
        r if r >= 0.9 => Color32::from_rgb(214, 60, 50),
        r if r >= 0.75 => Color32::from_rgb(230, 150, 40),
        _ => Color32::from_rgb(52, 120, 220),
    };

    let p = ui.painter();
    p.rect(rect, 8.0, fill, Stroke::new(1.0, border), StrokeKind::Inside);
    drives::paint_icon(
        p,
        Rect::from_min_size(rect.min + vec2(12.0, 16.0), vec2(56.0, 52.0)),
        drive.kind,
        Color32::from_rgb(90, 220, 120),
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
    p.rect_filled(bar, 5.0, Color32::from_black_alpha(70));
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
