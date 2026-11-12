use crate::chart::{ChartAction, ChartCtx};
use crate::scan::{self, Node, ScanHandle};
use crate::util::{format_age, format_items, format_size, reveal_in_file_manager};
use crate::{rings, treemap};
use egui::{
    Align2, CentralPanel, Color32, CornerRadius, FontId, Panel, Rect, ScrollArea, Sense, Shape,
    Stroke, StrokeKind, Ui, pos2, vec2,
};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

const ROW_H: f32 = 24.0;
const INDENT: f32 = 16.0;
const BAR_W: f32 = 56.0;
const COL_SIZE: f32 = 84.0;
const COL_ITEMS: f32 = 92.0;
const COL_DATE: f32 = 96.0;

#[derive(PartialEq, Clone, Copy)]
enum ChartKind {
    Rings,
    Treemap,
}

enum ListAction {
    Select(Vec<usize>),
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
}

impl App {
    pub fn new(ctx: &egui::Context, start: PathBuf) -> Self {
        let mut app = Self {
            root: None,
            root_path: start.clone(),
            view: Vec::new(),
            selected: None,
            expanded: HashSet::new(),
            scan: None,
            chart: ChartKind::Treemap,
            scroll_to_selected: false,
        };
        app.start_scan(ctx, start);
        app
    }

    fn start_scan(&mut self, ctx: &egui::Context, path: PathBuf) {
        if let Some(old) = self.scan.take() {
            old.cancel.store(true, Ordering::Relaxed);
        }
        self.scan = Some(scan::start(path, ctx.clone()));
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

    fn display_path(&self) -> String {
        self.root
            .as_ref()
            .map(|r| r.path_of(&self.root_path, &self.view))
            .unwrap_or_else(|| self.root_path.clone())
            .display()
            .to_string()
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            if ui
                .add_enabled(!self.view.is_empty(), egui::Button::new("⬅"))
                .on_hover_text("Parent folder (Backspace)")
                .clicked()
            {
                self.go_up();
            }
            if ui.button("🏠").on_hover_text("Scan home folder").clicked()
                && let Some(home) = dirs::home_dir()
            {
                self.start_scan(&ctx, home);
            }
            if ui.button("📂 Open…").on_hover_text("Choose a folder to scan").clicked()
                && let Some(dir) = rfd::FileDialog::new().pick_folder()
            {
                self.start_scan(&ctx, dir);
            }
            if ui.button("⟳").on_hover_text("Rescan").clicked() {
                let path = self.scan.as_ref().map_or(self.root_path.clone(), |s| s.path.clone());
                self.start_scan(&ctx, path);
            }
            ui.separator();
            ui.label(self.display_path());

            if let Some(scan) = &self.scan {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        scan.cancel.store(true, Ordering::Relaxed);
                    }
                    ui.spinner();
                    let p = &scan.progress;
                    ui.label(format!(
                        "{} files · {}",
                        p.files.load(Ordering::Relaxed),
                        format_size(p.bytes.load(Ordering::Relaxed))
                    ));
                });
            }
        });
    }

    fn list(&mut self, ui: &mut Ui) {
        let Some(root) = &self.root else { return };
        let Some(view) = root.get(&self.view) else { return };

        let mut rows = Vec::new();
        flatten(view, &mut self.view.clone(), 0, &self.expanded, &mut rows);

        // Header: column titles, then the current folder.
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
        let cols = Columns::new(rect);
        let weak = ui.visuals().weak_text_color();
        let font = FontId::proportional(12.0);
        let p = ui.painter();
        p.text(pos2(rect.left() + 6.0, rect.center().y), Align2::LEFT_CENTER, "Folder", font.clone(), weak);
        p.text(pos2(cols.size_right, rect.center().y), Align2::RIGHT_CENTER, "Size ▼", font.clone(), weak);
        p.text(pos2(cols.items_right, rect.center().y), Align2::RIGHT_CENTER, "Contents", font.clone(), weak);
        p.text(pos2(cols.date_right, rect.center().y), Align2::RIGHT_CENTER, "Modified", font, weak);

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
        p.text(pos2(cols.size_right, y), Align2::RIGHT_CENTER, format_size(view.size), f.clone(), strong);
        p.text(pos2(cols.items_right, y), Align2::RIGHT_CENTER, format_items(view.items), f.clone(), normal);
        p.text(pos2(cols.date_right, y), Align2::RIGHT_CENTER, format_age(view.modified), f, normal);

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
                if let Some(a) = draw_row(ui, row, self.selected.as_ref(), &self.expanded) {
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
        if ui.input(|i| i.key_pressed(egui::Key::Backspace)) {
            self.go_up();
        }

        Panel::top("toolbar").show(ui, |ui| {
            ui.add_space(4.0);
            self.toolbar(ui);
            ui.add_space(4.0);
        });

        if self.root.is_none() {
            CentralPanel::default().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() / 3.0);
                        ui.spinner();
                        if let Some(scan) = &self.scan {
                            ui.label(format!("Scanning {}…", scan.path.display()));
                            let p = &scan.progress;
                            ui.label(format!(
                                "{} folders · {} files · {} · {:.0}s",
                                p.dirs.load(Ordering::Relaxed),
                                p.files.load(Ordering::Relaxed),
                                format_size(p.bytes.load(Ordering::Relaxed)),
                                scan.started.elapsed().as_secs_f32()
                            ));
                        } else {
                            ui.label("Scan cancelled. Use “Open…” to choose a folder.");
                        }
                    });
                });
            });
            return;
        }

        Panel::left("list")
            .resizable(true)
            .default_size(560.0)
            .size_range(320.0..=900.0)
            .show(ui, |ui| self.list(ui));

        Panel::bottom("chart_kind").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.horizontal(|ui| {
                        let w = ui.available_width();
                        ui.add_space((w - 270.0).max(0.0) / 2.0);
                        ui.selectable_value(&mut self.chart, ChartKind::Rings, "◎ Rings Chart");
                        ui.selectable_value(&mut self.chart, ChartKind::Treemap, "▦ Treemap Chart");
                    });
                });
            });
            ui.add_space(4.0);
        });

        CentralPanel::default().show(ui, |ui| self.chart_panel(ui));
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

fn flatten<'a>(
    node: &'a Node,
    path: &mut Vec<usize>,
    depth: usize,
    expanded: &HashSet<Vec<usize>>,
    rows: &mut Vec<Row<'a>>,
) {
    for (i, child) in node.children.iter().enumerate() {
        path.push(i);
        rows.push(Row {
            node: child,
            path: path.clone(),
            depth,
            parent_size: node.size,
        });
        if child.is_dir && expanded.contains(path.as_slice()) {
            flatten(child, path, depth + 1, expanded, rows);
        }
        path.pop();
    }
}

fn draw_row(
    ui: &mut Ui,
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
    p.text(pos2(cols.size_right, y), Align2::RIGHT_CENTER, format_size(node.size), f.clone(), visuals.text_color());
    p.text(pos2(cols.items_right, y), Align2::RIGHT_CENTER, format_items(node.items), f.clone(), visuals.text_color());
    p.text(pos2(cols.date_right, y), Align2::RIGHT_CENTER, format_age(node.modified), f, visuals.text_color());

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
        if ui.button("Show in file manager").clicked() {
            action = Some(ListAction::Reveal(row.path.clone()));
            ui.close();
        }
        if ui.button("Copy path").clicked() {
            action = Some(ListAction::CopyPath(row.path.clone()));
            ui.close();
        }
        if node.is_dir && ui.button("Open in chart").clicked() {
            action = Some(ListAction::Open(row.path.clone()));
            ui.close();
        }
    });
    action
}
