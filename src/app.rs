use crate::chart::{ChartAction, ChartCtx};
use crate::drives::{self, Drive};
use crate::icons::Icon;
use crate::scan::{self, Node, ScanActivityKind, ScanHandle};
use crate::i18n::{Key, Lang, format_age, format_duration, format_items, format_size, tr, trf};
use crate::util::{delete_permanently, reveal_in_file_manager};
use crate::widgets::{Btn, Crumb, FooterAction, breadcrumb, divider, footer, status_pill, tool_button};
use crate::{rings, theme, treemap, update};
use egui::{
    Align2, CentralPanel, Color32, CornerRadius, FontId, Panel, Rect, ScrollArea, Sense, Shape,
    Stroke, StrokeKind, Ui, pos2, vec2,
};
use std::collections::HashSet;
use std::path::{Component, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const MAX_RECENT: usize = 5;
const MAX_LOG_ENTRIES: usize = 500;
const SCAN_STALL_AFTER: std::time::Duration = std::time::Duration::from_secs(10);
const REPO_URL: &str = "https://github.com/LaurentHouel/rs-DiskAnalyzer";
include!(concat!(env!("OUT_DIR"), "/dependencies.rs"));
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

struct LogEntry {
    elapsed: std::time::Duration,
    text: String,
}

struct ScanMonitor {
    snapshot: (u64, u64, u64),
    last_progress: std::time::Instant,
    stall_logged: bool,
}

enum DeleteOutcome {
    Cancelled,
    Failed(String),
}

struct DeleteHandle {
    path: Vec<usize>,
    name: String,
    current_path: Arc<Mutex<PathBuf>>,
    cancel: Arc<AtomicBool>,
    rx: std::sync::mpsc::Receiver<Result<(), DeleteOutcome>>,
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
    deleting: Option<DeleteHandle>,
    info: Option<Vec<usize>>,
    /// Last status message; the flag is true for errors.
    message: Option<(bool, String)>,
    logs: Arc<Mutex<Vec<LogEntry>>>,
    show_logs: Arc<AtomicBool>,
    scan_monitor: Option<ScanMonitor>,
    root_ctx: Option<egui::Context>,
    update_check_rx: Option<std::sync::mpsc::Receiver<update::CheckResult>>,
    update_available: Option<update::Release>,
    started_at: std::time::Instant,
    recent: Vec<PathBuf>,
    drives: Vec<Drive>,
    /// Widths of the Size, Contents and Modified columns.
    col_widths: [f32; 3],
    dark: bool,
    logo: Option<egui::TextureHandle>,
    show_about: bool,
}

impl App {
    /// Builds application state from persisted preferences and optional launch-time scan input.
    ///
    /// # Parameters
    /// - `cc`: eframe creation context providing storage and the egui context.
    /// - `start`: Optional path to scan immediately.
    ///
    /// # Returns
    /// An initialized application state, with a release check started in the background.
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
        app.record_activity(tr(lang, Key::AppStarted).to_owned());
        app.update_check_rx = Some(update::start_check(cc.egui_ctx.clone()));
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

    /// Creates an application state with no active scan or loaded result.
    ///
    /// # Parameters
    /// - `lang`: Initial interface language.
    /// - `recent`: Previously scanned paths to show on the welcome screen.
    ///
    /// # Returns
    /// A ready-to-use application state with default view, sorting, theme, and column settings.
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
            deleting: None,
            info: None,
            message: None,
            logs: Arc::new(Mutex::new(Vec::new())),
            show_logs: Arc::new(AtomicBool::new(false)),
            scan_monitor: None,
            root_ctx: None,
            update_check_rx: None,
            update_available: None,
            started_at: std::time::Instant::now(),
            recent,
            drives: drives::list(),
            col_widths: DEFAULT_COLS,
            dark: true,
            logo: None,
            show_about: false,
        }
    }

    /// Cancels any current scan, records the chosen path, and starts a replacement scan.
    ///
    /// # Parameters
    /// - `self`: Application state to update.
    /// - `ctx`: egui context used by the scan worker for repaint requests.
    /// - `path`: File or directory to scan.
    ///
    /// # Returns
    /// Nothing; updates scan state, history, and activity logs.
    fn start_scan(&mut self, ctx: &egui::Context, path: PathBuf) {
        if let Some(old) = self.scan.take() {
            self.record_activity(trf(
                self.lang,
                Key::LogScanCancelled,
                &[&old.path.display().to_string()],
            ));
            old.cancel.store(true, Ordering::Relaxed);
        }
        self.record_activity(trf(
            self.lang,
            Key::LogScanStarted,
            &[&path.display().to_string()],
        ));
        self.message = None;
        self.pending_trash = None;
        self.pending_delete = None;
        self.info = None;
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path.clone());
        self.recent.truncate(MAX_RECENT);
        self.scan = Some(scan::start(path, ctx.clone()));
        self.scan_monitor = None;
    }

    /// Back to the selection screen: drops the current result and cancels any running scan.
    ///
    /// # Parameters
    /// - `self`: Application state to reset.
    ///
    /// # Returns
    /// Nothing; clears scan results and refreshes the available drives.
    fn go_to_start_screen(&mut self) {
        self.scan_monitor = None;
        if let Some(scan) = self.scan.take() {
            self.record_activity(trf(
                self.lang,
                Key::LogScanCancelled,
                &[&scan.path.display().to_string()],
            ));
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

    /// Checks for a completed scan and installs its result when available.
    ///
    /// # Parameters
    /// - `self`: Application state containing the optional scan handle.
    ///
    /// # Returns
    /// Nothing; updates the tree, timing, navigation state, and completion log on success.
    fn poll_scan(&mut self) {
        let Some(scan) = &self.scan else { return };
        if let Ok(root) = scan.rx.try_recv() {
            let log = trf(
                self.lang,
                Key::LogScanCompleted,
                &[
                    &scan.path.display().to_string(),
                    &scan.progress.dirs.load(Ordering::Relaxed).to_string(),
                    &scan.progress.files.load(Ordering::Relaxed).to_string(),
                    &format_size(self.lang, scan.progress.bytes.load(Ordering::Relaxed)),
                    &scan.progress.errors.load(Ordering::Relaxed).to_string(),
                    &format_duration(self.lang, scan.started.elapsed()),
                ],
            );
            self.root_path = scan.path.clone();
            self.root = Some(root);
            self.view.clear();
            self.selected = None;
            self.expanded.clear();
            self.scan_time = Some(scan.started.elapsed());
            self.scan = None;
            self.scan_monitor = None;
            self.record_activity(log);
        }
    }

    /// Adds an activity message to the bounded log and refreshes the log window when visible.
    ///
    /// # Parameters
    /// - `self`: Application state containing the log and window state.
    /// - `text`: Message to append.
    ///
    /// # Returns
    /// Nothing; stores the message and may request a repaint.
    fn record_activity(&mut self, text: String) {
        {
            let mut logs = self.logs.lock().unwrap();
            if logs.len() == MAX_LOG_ENTRIES {
                logs.remove(0);
            }
            logs.push(LogEntry {
                elapsed: self.started_at.elapsed(),
                text,
            });
        }
        if self.show_logs.load(Ordering::Relaxed)
            && let Some(ctx) = &self.root_ctx
        {
            ctx.request_repaint_of(egui::ViewportId::from_hash_of("activity_logs"));
        }
    }

    /// Receives a completed release check and records any update or error.
    ///
    /// # Parameters
    /// - `self`: Application state containing the optional release-check receiver.
    ///
    /// # Returns
    /// Nothing; consumes a ready result and updates release availability or activity logs.
    fn poll_update_check(&mut self) {
        let result = match &self.update_check_rx {
            Some(rx) => match rx.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("release check stopped before returning a result".to_owned()))
                }
            },
            None => return,
        };
        let Some(result) = result else { return };
        self.update_check_rx = None;
        match result {
            Ok(Some(release)) => {
                self.record_activity(trf(self.lang, Key::UpdateAvailable, &[&release.version]));
                self.update_available = Some(release);
            }
            Ok(None) => {}
            Err(error) => {
                self.record_activity(trf(self.lang, Key::UpdateCheckFailed, &[&error]));
            }
        }
    }

    /// Detects a stalled scan and logs its latest activity at most once per stall.
    ///
    /// # Parameters
    /// - `self`: Application state containing scan progress and monitor data.
    ///
    /// # Returns
    /// Nothing; updates monitor snapshots and records a localized no-progress message when needed.
    fn monitor_scan(&mut self) {
        let Some(scan) = self.scan.as_ref() else {
            self.scan_monitor = None;
            return;
        };
        let snapshot = (
            scan.progress.dirs.load(Ordering::Relaxed),
            scan.progress.files.load(Ordering::Relaxed),
            scan.progress.errors.load(Ordering::Relaxed),
        );
        let now = std::time::Instant::now();
        let message = {
            let monitor = self.scan_monitor.get_or_insert(ScanMonitor {
                snapshot,
                last_progress: now,
                stall_logged: false,
            });
            if monitor.snapshot != snapshot {
                monitor.snapshot = snapshot;
                monitor.last_progress = now;
                monitor.stall_logged = false;
                None
            } else if monitor.stall_logged || now.duration_since(monitor.last_progress) < SCAN_STALL_AFTER {
                None
            } else {
                monitor.stall_logged = true;
                let activity = scan.progress.activity();
                let path = activity.path.display().to_string();
                let activity = match activity.kind {
                    ScanActivityKind::CheckingRoot => {
                        trf(self.lang, Key::ScanActivityCheckingRoot, &[&path])
                    }
                    ScanActivityKind::ReadingDirectory => {
                        trf(self.lang, Key::ScanActivityReadingDirectory, &[&path])
                    }
                    ScanActivityKind::CheckingEntries => {
                        trf(self.lang, Key::ScanActivityCheckingEntries, &[&path])
                    }
                };
                Some(trf(
                    self.lang,
                    Key::LogScanNoProgress,
                    &[
                        &format_duration(self.lang, now.duration_since(monitor.last_progress)),
                        &snapshot.0.to_string(),
                        &snapshot.1.to_string(),
                        &activity,
                    ],
                ))
            }
        };
        if let Some(message) = message {
            self.record_activity(message);
        }
    }

    /// Opens or updates the deferred activity-log window when it is marked visible.
    ///
    /// # Parameters
    /// - `self`: Application state providing language, logs, and visibility state.
    /// - `ctx`: Parent egui context used to create the deferred window.
    ///
    /// # Returns
    /// Nothing; renders the window and records close requests in the shared visibility flag.
    fn show_logs_window(&self, ctx: &egui::Context) {
        if !self.show_logs.load(Ordering::Relaxed) {
            return;
        }
        let logs = self.logs.clone();
        let show_logs = self.show_logs.clone();
        let parent_ctx = ctx.clone();
        let lang = self.lang;
        let viewport_id = egui::ViewportId::from_hash_of("activity_logs");
        let viewport = egui::ViewportBuilder::default()
            .with_title(tr(lang, Key::LogsTitle))
            .with_position([200.0, 120.0])
            .with_inner_size([680.0, 480.0])
            .with_min_inner_size([420.0, 240.0])
            .with_resizable(true);
        ctx.show_viewport_deferred(viewport_id, viewport, move |ctx, _class| {
            if ctx.input(|i| i.viewport().close_requested()) {
                show_logs.store(false, Ordering::Relaxed);
                parent_ctx.request_repaint();
                return;
            }

            let mut close = false;
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading(tr(lang, Key::LogsTitle));
                ui.add_space(4.0);
                let logs_height = ui.available_height() - 40.0;
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(logs_height.max(80.0))
                    .show(ui, |ui| {
                        let logs = logs.lock().unwrap();
                        if logs.is_empty() {
                            ui.label(tr(lang, Key::LogsEmpty));
                        } else {
                            for entry in logs.iter().rev() {
                                let seconds = entry.elapsed.as_secs();
                                let timestamp =
                                    format!("{:02}:{:02}:{:02}", seconds / 3600, (seconds / 60) % 60, seconds % 60);
                                ui.horizontal_top(|ui| {
                                    ui.label(egui::RichText::new(timestamp).monospace().weak());
                                    ui.add(egui::Label::new(egui::RichText::new(&entry.text).monospace()).wrap());
                                });
                            }
                        }
                    });
                ui.add_space(8.0);
                if ui.button(tr(lang, Key::Close)).clicked() {
                    close = true;
                }
            });
            if close {
                show_logs.store(false, Ordering::Relaxed);
                parent_ctx.request_repaint();
            }
        });
    }

    /// Navigates to the parent folder and selects the folder that was just left.
    ///
    /// # Parameters
    /// - `self`: Application state whose current view is updated.
    ///
    /// # Returns
    /// Nothing; has no effect when already at the scan root.
    fn go_up(&mut self) {
        if let Some(old) = self.view.pop() {
            let mut sel = self.view.clone();
            sel.push(old);
            self.selected = Some(sel);
            self.scroll_to_selected = true;
        }
    }

    /// Selects a tree node, expands its ancestors, and optionally requests list scrolling.
    ///
    /// # Parameters
    /// - `self`: Application state to update.
    /// - `path`: Index path of the selected node.
    /// - `scroll`: Whether the list should scroll the selected node into view.
    ///
    /// # Returns
    /// Nothing; updates expansion, selection, and scroll state.
    fn select(&mut self, path: Vec<usize>, scroll: bool) {
        for n in self.view.len() + 1..path.len() {
            self.expanded.insert(path[..n].to_vec());
        }
        self.selected = Some(path);
        self.scroll_to_selected = scroll;
    }

    /// Navigates into the selected node when it is a directory.
    ///
    /// # Parameters
    /// - `self`: Application state whose current view is updated.
    /// - `path`: Index path of the node to open.
    ///
    /// # Returns
    /// Nothing; does nothing when the path is invalid or identifies a file.
    fn open(&mut self, path: Vec<usize>) {
        if self.root.as_ref().and_then(|r| r.get(&path)).is_some_and(|n| n.is_dir) {
            self.view = path;
            self.selected = None;
        }
    }

    /// Checks whether any dialog or deletion operation currently blocks global shortcuts.
    ///
    /// # Parameters
    /// - `self`: Application state whose modal flags are inspected.
    ///
    /// # Returns
    /// `true` when a confirmation, information, about, or deletion dialog is active.
    fn modal_open(&self) -> bool {
        self.pending_trash.is_some()
            || self.pending_delete.is_some()
            || self.deleting.is_some()
            || self.info.is_some()
            || self.show_about
    }

    /// Renders the application information dialog and its dependency links.
    ///
    /// # Parameters
    /// - `self`: Application state containing the dialog visibility and language.
    /// - `ctx`: egui context used to create the modal.
    ///
    /// # Returns
    /// Nothing; clears the visibility flag after the modal closes.
    fn about_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let lang = self.lang;
        let modal = egui::Modal::new(egui::Id::unique("about_modal")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading(tr(lang, Key::About));
            ui.label(format!("Disk Analyzer v{}", env!("CARGO_PKG_VERSION")));
            ui.add_space(8.0);
            ui.label(tr(lang, Key::AboutLibraries));
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height(280.0)
                .show(ui, |ui| {
                    for (dependency, version) in DEPENDENCIES {
                        ui.hyperlink_to(
                            format!("{dependency} {version}"),
                            format!("https://crates.io/crates/{dependency}"),
                        );
                    }
                });
            ui.add_space(8.0);
            if ui.button(tr(lang, Key::Close)).clicked() {
                ui.close();
            }
        });
        if modal.should_close() {
            self.show_about = false;
        }
    }

    /// Confirmation for moving to the trash, or for deleting permanently when `permanent`.
    ///
    /// # Parameters
    /// - `self`: Application state containing the pending target and dialog state.
    /// - `ctx`: egui context used to render the modal.
    /// - `permanent`: Whether the dialog confirms permanent deletion instead of trashing.
    ///
    /// # Returns
    /// Nothing; starts the confirmed operation or clears the pending target on cancellation.
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
            if permanent {
                self.start_permanent_delete(&path, ctx);
            } else {
                self.remove(&path);
            }
        } else if modal.should_close() {
            self.pending_trash = None;
            self.pending_delete = None;
        }
    }

    /// Starts permanent deletion on a worker thread and tracks its progress and cancellation.
    ///
    /// # Parameters
    /// - `self`: Application state that owns the pending deletion handle.
    /// - `path`: Index path of the node to remove from the scanned tree on success.
    /// - `ctx`: egui context used to repaint as filesystem progress changes.
    ///
    /// # Returns
    /// Nothing; stores a deletion handle or reports a worker-start error to the user.
    fn start_permanent_delete(&mut self, path: &[usize], ctx: &egui::Context) {
        let Some(root) = &self.root else { return };
        let target = root.path_of(&self.root_path, path);
        let Some(name) = root.get(path).map(|node| node.name.clone()) else {
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let current_path = Arc::new(Mutex::new(target.clone()));
        let worker_current_path = current_path.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let ctx = ctx.clone();
        let worker = std::thread::Builder::new()
            .name("permanent-delete".to_owned())
            .spawn(move || {
                let result = match delete_permanently(&target, &worker_cancel, |path| {
                    *worker_current_path.lock().unwrap() = path.to_path_buf();
                    ctx.request_repaint();
                }) {
                    Ok(()) => Ok(()),
                    Err(error)
                        if error.kind() == std::io::ErrorKind::Interrupted
                            && worker_cancel.load(Ordering::Relaxed) =>
                    {
                        Err(DeleteOutcome::Cancelled)
                    }
                    Err(error) => Err(DeleteOutcome::Failed(error.to_string())),
                };
                let _ = tx.send(result);
                ctx.request_repaint();
            });
        match worker {
            Ok(_) => {
                self.deleting = Some(DeleteHandle {
                    path: path.to_vec(),
                    name,
                    current_path,
                    cancel,
                    rx,
                })
            }
            Err(error) => {
                let message = trf(self.lang, Key::DeleteFailed, &[&error.to_string()]);
                self.record_activity(message.clone());
                self.message = Some((true, message));
            }
        }
    }

    /// Receives a completed deletion result and updates the tree or reports its outcome.
    ///
    /// # Parameters
    /// - `self`: Application state containing the optional deletion handle.
    ///
    /// # Returns
    /// Nothing; keeps waiting while the worker is active and handles its result once ready.
    fn poll_delete(&mut self) {
        let result = match self
            .deleting
            .as_ref()
            .map(|deleting| deleting.rx.try_recv())
        {
            Some(Ok(result)) => Some(result),
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => Some(Err(
                DeleteOutcome::Failed("delete worker stopped before returning a result".to_owned()),
            )),
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) | None => None,
        };
        let Some(result) = result else {
            return;
        };
        let Some(deleting) = self.deleting.take() else {
            return;
        };
        match result {
            Ok(()) => self.finish_remove(&deleting.path, deleting.name, true),
            Err(DeleteOutcome::Cancelled) => {
                let message = tr(self.lang, Key::DeleteCancelled).to_owned();
                self.record_activity(message.clone());
                self.message = Some((true, message));
            }
            Err(DeleteOutcome::Failed(error)) => {
                let message = trf(self.lang, Key::DeleteFailed, &[&error]);
                self.record_activity(message.clone());
                self.message = Some((true, message));
            }
        }
    }

    /// Displays deletion progress and lets the user request cancellation.
    ///
    /// # Parameters
    /// - `self`: Application state containing current deletion progress and cancellation state.
    /// - `ctx`: egui context used to render the modal.
    ///
    /// # Returns
    /// Nothing; updates the cancellation flag when the user requests to stop.
    fn deleting_dialog(&self, ctx: &egui::Context) {
        let Some(deleting) = &self.deleting else {
            return;
        };
        let message = tr(self.lang, Key::DeleteInProgress);
        let current_item = trf(
            self.lang,
            Key::DeleteCurrentItem,
            &[&deleting.current_path.lock().unwrap().display().to_string()],
        );
        let cancel_requested = deleting.cancel.load(Ordering::Relaxed);
        egui::Modal::new(egui::Id::unique("deleting_modal")).show(ctx, |ui| {
            ui.set_width(640.0);
            ui.with_layout(
                egui::Layout::top_down(egui::Align::Center)
                    .with_main_align(egui::Align::Center),
                |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(if cancel_requested {
                        tr(self.lang, Key::DeleteStopping)
                    } else {
                        message
                    });
                });
                ui.add_space(12.0);
                egui::ScrollArea::horizontal()
                    .max_height(36.0)
                    .show(ui, |ui| ui.add(egui::Label::new(current_item).extend()));
                ui.add_space(12.0);
                if ui
                    .add_enabled(
                        !cancel_requested,
                        egui::Button::new(tr(self.lang, Key::Cancel)),
                    )
                    .clicked()
                {
                    deleting.cancel.store(true, Ordering::Relaxed);
                    ctx.request_repaint();
                }
                },
            );
        });
    }

    /// Shows metadata and size-share details for the selected tree node.
    ///
    /// # Parameters
    /// - `self`: Application state containing the selected information path and scan tree.
    /// - `ctx`: egui context used to render the modal.
    ///
    /// # Returns
    /// Nothing; opens the selected path in the file manager or clears the dialog on close.
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

    /// Moves a scanned node to the system trash and reports any failure.
    ///
    /// # Parameters
    /// - `self`: Application state containing the scan tree and root path.
    /// - `path`: Index path of the node to move to the trash.
    ///
    /// # Returns
    /// Nothing; updates the tree after success or displays and logs the error.
    fn remove(&mut self, path: &[usize]) {
        let Some(root) = &mut self.root else { return };
        let target = root.path_of(&self.root_path, path);
        let name = root.get(path).map(|n| n.name.clone()).unwrap_or_default();
        if let Err(error) = trash::delete(&target) {
            let message = trf(self.lang, Key::TrashFailed, &[&error.to_string()]);
            self.record_activity(message.clone());
            self.message = Some((true, message));
            return;
        }

        self.finish_remove(path, name, false);
    }

    /// Removes a completed deletion from the in-memory tree and restores navigation paths.
    ///
    /// # Parameters
    /// - `self`: Application state containing the scan tree and navigation state.
    /// - `path`: Index path of the removed node in the pre-removal tree.
    /// - `name`: Removed node name used in the completion message.
    /// - `permanent`: Whether the operation bypassed the system trash.
    ///
    /// # Returns
    /// Nothing; refreshes tree indices, selection, expansion, and status message.
    fn finish_remove(&mut self, path: &[usize], name: String, permanent: bool) {
        let Some(root) = &mut self.root else { return };
        // Removing a node re-sorts its ancestors, so index paths are rebuilt from filesystem paths.
        let root_path = &self.root_path;
        let view_path = root.path_of(root_path, &self.view);
        let selected = self.selected.as_ref().map(|s| root.path_of(root_path, s));
        let expanded: Vec<PathBuf> = self
            .expanded
            .iter()
            .map(|e| root.path_of(root_path, e))
            .collect();

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
        let key = if permanent {
            Key::Deleted
        } else {
            Key::Trashed
        };
        let message = trf(self.lang, key, &[&name]);
        self.record_activity(message.clone());
        self.message = Some((false, message));
    }

    /// Progress page shown for every scan, including a rescan over an existing result.
    /// Returns true when the user asked to cancel.
    ///
    /// # Parameters
    /// - `self`: Application state containing the optional scan and language.
    /// - `ui`: UI in which to render the progress view and cancel button.
    ///
    /// # Returns
    /// `true` when the user clicked Cancel; otherwise `false`.
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
    ///
    /// # Parameters
    /// - `self`: Application state providing language, drives, and recent paths.
    /// - `ui`: UI in which to render and receive start-page input.
    ///
    /// # Returns
    /// Nothing; starts a scan when the user chooses a drive, recent path, home folder, or directory.
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
    ///
    /// # Parameters
    /// - `self`: Application state providing the scan root and current view.
    ///
    /// # Returns
    /// Breadcrumb elements for the filesystem path, with navigable depths inside the scanned tree.
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
    ///
    /// # Parameters
    /// - `self`: Application state whose view and selection are updated.
    /// - `depth`: Number of child levels to retain from the current scan root.
    ///
    /// # Returns
    /// Nothing; truncates the current view only when navigating to an ancestor.
    fn navigate_to(&mut self, depth: usize) {
        if depth < self.view.len() {
            self.view.truncate(depth);
            self.selected = None;
        }
    }

    /// Renders navigation, scan, deletion, language, theme, logs, and about controls.
    ///
    /// # Parameters
    /// - `self`: Application state updated by toolbar interactions.
    /// - `ui`: UI in which to render and receive toolbar input.
    ///
    /// # Returns
    /// Nothing; applies actions selected by the user.
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

                let about =
                    Btn::icon(Icon::Info, tr(lang, Key::AboutTip)).with_text(tr(lang, Key::About));
                if tool_button(ui, &pal, true, about)
                    .on_hover_text(tr(lang, Key::AboutTip))
                    .clicked()
                {
                    self.show_about = true;
                }

                let logs = Btn::icon(Icon::Logs, tr(lang, Key::LogsTip)).with_text(tr(lang, Key::Logs));
                if tool_button(ui, &pal, true, logs)
                    .on_hover_text(tr(lang, Key::LogsTip))
                    .clicked()
                {
                    self.show_logs.store(true, Ordering::Relaxed);
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

    /// Lazily loads and caches the embedded application logo as an egui texture.
    ///
    /// # Parameters
    /// - `self`: Application state containing the optional cached texture.
    /// - `ctx`: egui context used to create the texture if needed.
    ///
    /// # Returns
    /// The texture identifier for the cached logo.
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
    ///
    /// # Parameters
    /// - `self`: Application state providing the logo, scan status, and cancellation handle.
    /// - `ui`: UI in which to draw the title row and receive window-control input.
    ///
    /// # Returns
    /// Nothing; renders the title and may cancel a running scan.
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

    /// Draws sortable list-column headings and handles column resizing.
    ///
    /// # Parameters
    /// - `self`: Application state containing sort preferences and column widths.
    /// - `ui`: UI in which to render and receive header input.
    ///
    /// # Returns
    /// Nothing; updates sorting and column widths in response to user interaction.
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

    /// Renders the current directory's sortable, expandable file list and handles row actions.
    ///
    /// # Parameters
    /// - `self`: Application state containing the tree, view, selection, and list preferences.
    /// - `ui`: UI in which to render and receive list input.
    ///
    /// # Returns
    /// Nothing; applies the action selected from the list or a row context menu.
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
        let name_left = rect.left() + 6.0;
        let name_width = (cols.name_right - name_left).max(0.0);
        let name_font = FontId::proportional(14.0);
        let name = truncate_text(p, &view.name, name_width, &name_font, accent);
        let name_clip = Rect::from_min_max(
            pos2(name_left, rect.top()),
            pos2(name_left + name_width, rect.bottom()),
        );
        p.with_clip_rect(name_clip).text(
            pos2(name_left, y),
            Align2::LEFT_CENTER,
            name,
            name_font,
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

    /// Renders the selected chart style and applies chart selection or context-menu actions.
    ///
    /// # Parameters
    /// - `self`: Application state containing the tree, chart selection, and current view.
    /// - `ui`: UI in which to render the chart and receive pointer input.
    ///
    /// # Returns
    /// Nothing; updates navigation, selection, or pending actions from the chart response.
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
    /// Draws one application frame by delegating to the main UI update method.
    ///
    /// # Parameters
    /// - `self`: Application state to update and render.
    /// - `ui`: Root UI supplied by eframe.
    /// - `_frame`: eframe frame metadata, unused by this application.
    ///
    /// # Returns
    /// Nothing; renders the current frame and processes its input.
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }

    /// Persists language, theme, recent paths, and column widths to eframe storage.
    ///
    /// # Parameters
    /// - `self`: Application preferences to save.
    /// - `storage`: Persistent storage provided by eframe.
    ///
    /// # Returns
    /// Nothing; writes the supported preference keys to `storage`.
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
    /// Processes asynchronous results and renders the appropriate application screen and dialogs.
    ///
    /// # Parameters
    /// - `self`: Application state to update and render.
    /// - `ui`: Root UI in which to draw the current frame.
    ///
    /// # Returns
    /// Nothing; handles input, state transitions, and drawing for one frame.
    fn show(&mut self, ui: &mut Ui) {
        self.root_ctx = Some(ui.ctx().clone());
        self.poll_update_check();
        self.poll_scan();
        self.poll_delete();
        self.monitor_scan();
        if self.scan.is_some() || self.deleting.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
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
        let update_tip = self
            .update_available
            .as_ref()
            .map(|release| trf(self.lang, Key::UpdateAvailableTip, &[&release.version]));
        let mut footer_action = None;
        Panel::bottom("footer").frame(footer_frame).show(ui, |ui| {
            footer_action = footer(
                ui,
                &pal,
                &format!("Disk Analyzer v{}", env!("CARGO_PKG_VERSION")),
                tr(self.lang, Key::MadeWith),
                tr(self.lang, Key::MadeBy),
                "GitHub",
                tr(self.lang, Key::GithubTip),
                update_tip.as_deref(),
            );
        });
        match footer_action {
            Some(FooterAction::Project) => ui.ctx().open_url(egui::OpenUrl::new_tab(REPO_URL)),
            Some(FooterAction::Release) => {
                if let Some(release) = &self.update_available {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(release.url.clone()));
                }
            }
            None => {}
        }

        self.show_logs_window(ui.ctx());
        self.about_dialog(ui.ctx());

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
        self.deleting_dialog(ui.ctx());
        self.info_dialog(ui.ctx());
    }
}

/// Parses and validates the three persisted list-column widths.
///
/// # Parameters
/// - `s`: Comma-separated width values loaded from storage.
///
/// # Returns
/// Three widths within the supported range, or `None` for malformed or out-of-range input.
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
    /// Calculates the right-edge positions of each list column within a row rectangle.
    ///
    /// # Parameters
    /// - `rect`: Full available row rectangle.
    /// - `widths`: Widths of the size, contents, and modified columns.
    ///
    /// # Returns
    /// Column boundary coordinates relative to the UI's screen space.
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

/// Produces child indices ordered by the requested property and direction.
///
/// # Parameters
/// - `children`: Nodes whose display order is requested.
/// - `(key, desc)`: Sort property and whether to reverse the ascending order.
///
/// # Returns
/// Child indices in display order, leaving the input nodes unchanged.
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

/// Flattens visible descendants into list rows, recursing only into expanded directories.
///
/// # Parameters
/// - `node`: Directory whose children should be added.
/// - `path`: Mutable index path extended and restored during traversal.
/// - `depth`: Current row indentation depth.
/// - `expanded`: Set of directory paths whose descendants should be included.
/// - `sort`: Sort key and direction for each directory's children.
/// - `rows`: Output vector receiving the generated rows.
///
/// # Returns
/// Nothing; appends visible rows to `rows`.
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

/// Draws one file-list row and returns the action triggered by its controls or context menu.
///
/// # Parameters
/// - `ui`: UI in which to draw the row and receive input.
/// - `lang`: Language used for row labels and context-menu text.
/// - `row`: Node, path, depth, and parent size represented by the row.
/// - `selected`: Currently selected node path, if any.
/// - `expanded`: Paths of directories currently expanded in the list.
/// - `widths`: Widths of the size, contents, and modified columns.
///
/// # Returns
/// The selected list action, or `None` when the row was not activated.
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
    let name_width = (cols.name_right - name_left).max(0.0);
    let name_font = FontId::proportional(13.0);
    let name = truncate_text(p, &node.name, name_width, &name_font, visuals.text_color());
    let name_clip = Rect::from_min_max(
        pos2(name_left, rect.top()),
        pos2(name_left + name_width, rect.bottom()),
    );
    p.with_clip_rect(name_clip).text(
        pos2(name_left, y),
        Align2::LEFT_CENTER,
        name,
        name_font,
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

fn truncate_text(
    painter: &egui::Painter,
    text: &str,
    max_width: f32,
    font: &FontId,
    color: Color32,
) -> String {
    if max_width <= 0.0 {
        return String::new();
    }
    if painter.layout_no_wrap(text.to_owned(), font.clone(), color).size().x <= max_width {
        return text.to_owned();
    }

    let ellipsis = "…";
    if painter.layout_no_wrap(ellipsis.to_owned(), font.clone(), color).size().x > max_width {
        return String::new();
    }

    let mut end = text.len();
    while end > 0 {
        end = text[..end].char_indices().last().map_or(0, |(index, _)| index);
        let candidate = format!("{}{}", &text[..end], ellipsis);
        if painter.layout_no_wrap(candidate.clone(), font.clone(), color).size().x <= max_width {
            return candidate;
        }
    }
    ellipsis.to_owned()
}

/// Draws a drive card with its type icon, used-space bar, and free-space label.
///
/// # Parameters
/// - `ui`: UI in which to draw the card and receive input.
/// - `lang`: Language used for capacity labels.
/// - `drive`: Drive whose properties are displayed.
/// - `width`: Horizontal width allocated to the card.
///
/// # Returns
/// `true` when the card is clicked.
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

    /// Builds a small deterministic directory tree for UI tests.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// A root node with one large and one small file child.
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

    /// Verifies that the home control returns to folder selection without starting a scan.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if the result or scan remains active after navigation.
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

    /// Verifies that the toolbar opens the activity log window with its latest entry.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if the log window or recorded activity is missing.
    #[test]
    fn toolbar_logs_button_opens_logs_window() {
        let mut app = App::empty(Lang::En, Vec::new());
        app.root = Some(sample_root());
        app.root_path = PathBuf::from("/data");
        app.record_activity("Test scan completed".to_owned());
        let mut harness = Harness::builder()
            .allow_missing_glyphs()
            .with_size(egui::vec2(1100.0, 700.0))
            .build_ui_state(|ui, app: &mut App| app.show(ui), app);
        harness.run();

        harness.get_by_label("Show activity logs").click();
        harness.run();

        assert!(harness.state().show_logs.load(Ordering::Relaxed));
        assert!(harness.query_all_by_label("Activity logs").count() > 0);
        harness.get_by_label("Test scan completed");
    }

    /// Verifies that the about dialog lists the application's dependency libraries.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if the dialog or representative library links are absent.
    #[test]
    fn about_button_lists_application_libraries_on_the_welcome_screen() {
        let app = App::empty(Lang::En, Vec::new());
        let mut harness = Harness::builder()
            .allow_missing_glyphs()
            .with_size(egui::vec2(1100.0, 700.0))
            .build_ui_state(|ui, app: &mut App| app.show(ui), app);
        harness.run();

        harness.get_by_label("Show application information and libraries").click();
        harness.run();

        assert!(harness.state().show_about);
        harness.get_by_label("Libraries used in this application");
        harness.get_by_label_contains("cap-std ");
        harness.get_by_label_contains("eframe ");
        harness.get_by_label_contains("ureq ");
    }

    /// Verifies that the footer displays a link when a newer release is available.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if the expected release link is not accessible.
    #[test]
    fn footer_shows_link_when_a_newer_release_is_available() {
        let mut app = App::empty(Lang::En, Vec::new());
        app.update_available = Some(update::Release {
            version: "0.2.0".to_owned(),
            url: "https://github.com/LaurentHouel/rs-DiskAnalyzer/releases/tag/v0.2.0".to_owned(),
        });
        let mut harness = Harness::builder()
            .allow_missing_glyphs()
            .with_size(egui::vec2(1100.0, 700.0))
            .build_ui_state(|ui, app: &mut App| app.show(ui), app);
        harness.run();

        harness.get_by_label("Open release 0.2.0 on GitHub");
    }

    /// Verifies that a stalled scan logs its latest activity only once.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if the stall message is missing or duplicated.
    #[test]
    fn scan_monitor_logs_the_last_activity_once_when_progress_stalls() {
        let mut app = App::empty(Lang::En, Vec::new());
        let path = PathBuf::from("/slow-folder");
        let progress = Arc::new(scan::Progress::default());
        progress.set_activity(ScanActivityKind::ReadingDirectory, &path);
        let (_tx, rx) = std::sync::mpsc::channel();
        app.scan = Some(ScanHandle {
            path,
            progress,
            cancel: Arc::new(AtomicBool::new(false)),
            rx,
            started: std::time::Instant::now(),
        });
        app.scan_monitor = Some(ScanMonitor {
            snapshot: (0, 0, 0),
            last_progress: std::time::Instant::now() - SCAN_STALL_AFTER,
            stall_logged: false,
        });

        app.monitor_scan();
        app.monitor_scan();

        let logs = app.logs.lock().unwrap();
        assert_eq!(logs.len(), 1);
        assert!(logs[0].text.contains("No progress"));
        assert!(logs[0].text.contains("Reading folder: /slow-folder"));
    }

    /// Verifies that background permanent deletion removes the file and updates the tree.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if the worker does not finish or the deleted node remains.
    #[test]
    fn permanent_delete_runs_in_background_and_updates_the_tree_when_done() {
        let dir = std::env::temp_dir().join(format!(
            "rs-da-delete-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let target = dir.join("victim.bin");
        std::fs::write(&target, b"test").unwrap();

        let mut app = App::empty(Lang::En, Vec::new());
        app.root_path = dir.clone();
        app.root = Some(Node {
            name: dir.file_name().unwrap().to_string_lossy().into_owned(),
            size: 4,
            items: 1,
            modified: None,
            is_dir: true,
            children: vec![Node {
                name: "victim.bin".to_owned(),
                size: 4,
                items: 1,
                modified: None,
                is_dir: false,
                children: Vec::new(),
            }],
        });

        app.start_permanent_delete(&[0], &egui::Context::default());
        assert!(app.deleting.is_some());

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.deleting.is_some() && std::time::Instant::now() < deadline {
            app.poll_delete();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        assert!(app.deleting.is_none());
        assert!(!target.exists());
        assert!(app.root.as_ref().unwrap().children.is_empty());
        std::fs::remove_dir(dir).unwrap();
    }

    /// Verifies that the deletion modal reports progress and accepts cancellation.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if progress, current path, or cancellation behavior is incorrect.
    #[test]
    fn deleting_modal_shows_that_permanent_deletion_is_in_progress() {
        let mut app = App::empty(Lang::En, Vec::new());
        app.root = Some(sample_root());
        let (_tx, rx) = std::sync::mpsc::channel();
        app.deleting = Some(DeleteHandle {
            path: vec![0],
            name: "big.bin".to_owned(),
            current_path: Arc::new(Mutex::new(PathBuf::from("/data/big.bin"))),
            cancel: Arc::new(AtomicBool::new(false)),
            rx,
        });
        let mut harness = Harness::builder()
            .allow_missing_glyphs()
            .with_size(egui::vec2(1100.0, 700.0))
            .build_ui_state(|ui, app: &mut App| app.show(ui), app);

        harness.step();

        harness.get_by_label("Deleting permanently… This may take a while.");
        harness.get_by_label("Current item: /data/big.bin");
        let cancel_button_top = harness.get_by_label("Cancel").rect().top();
        *harness
            .state()
            .deleting
            .as_ref()
            .unwrap()
            .current_path
            .lock()
            .unwrap() = PathBuf::from(format!("/data/{}", "directory with a long name/".repeat(20)));
        harness.step();
        assert_eq!(
            harness.get_by_label("Cancel").rect().top(),
            cancel_button_top
        );
        harness.get_by_label("Cancel").click();
        harness.step();
        harness.step();
        assert!(
            harness.state().deleting.as_ref().unwrap().cancel.load(Ordering::Relaxed)
        );
        harness.get_by_label("Stopping after the current filesystem operation…");
    }

    /// Verifies that cancelling deletion preserves the tree and displays a rescan warning.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if the tree changes or the cancellation warning is missing.
    #[test]
    fn cancelled_delete_keeps_the_tree_and_warns_that_it_may_be_stale() {
        let mut app = App::empty(Lang::En, Vec::new());
        app.root = Some(sample_root());
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Err(DeleteOutcome::Cancelled)).unwrap();
        app.deleting = Some(DeleteHandle {
            path: vec![0],
            name: "big.bin".to_owned(),
            current_path: Arc::new(Mutex::new(PathBuf::from("/data/big.bin"))),
            cancel: Arc::new(AtomicBool::new(true)),
            rx,
        });

        app.poll_delete();

        assert!(app.deleting.is_none());
        assert_eq!(app.root.as_ref().unwrap().children.len(), 2);
        assert!(app.message.as_ref().unwrap().1.contains("rescan"));
    }

    /// Verifies that selecting a chart item expands its ancestors and scrolls it into view.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if the selected chart item is not exposed in the list.
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

    /// Verifies that dragging a list-header boundary changes the corresponding column width.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if no resize handle changes a column width.
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

    /// Verifies that right-clicking a chart item opens its context menu and selects the item.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if menu actions or selection are unavailable.
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

    /// Verifies that clicking breadcrumb elements navigates to the matching ancestor.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if the view path does not match the clicked breadcrumb.
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

    /// Verifies that cancelling a scan returns the application to the selection screen.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if a scan or result remains visible after cancellation.
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

    /// Verifies that rescanning hides the previous result and offers progress cancellation.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if stale results remain visible or cancellation fails.
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

    /// Verifies that persisted column widths are accepted only when all three are in range.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if valid widths are rejected or malformed widths are accepted.
    #[test]
    fn stored_column_widths_are_validated() {
        assert_eq!(parse_widths("90,140,110"), Some([90.0, 140.0, 110.0]));
        assert_eq!(parse_widths("5,140,110"), None);
        assert_eq!(parse_widths("90,140"), None);
    }
}
