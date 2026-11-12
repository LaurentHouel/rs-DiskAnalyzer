use crate::chart::{ChartAction, ChartCtx, text_color_on, tooltip};
use crate::scan::Node;
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2};

const MIN_CELL: f32 = 3.0;
const PAD: f32 = 3.0;
const HEADER: f32 = 15.0;

struct Cell<'a> {
    rect: Rect,
    path: Vec<usize>,
    depth: usize,
    node: &'a Node,
    /// Space reserved for the title of a directory whose content is drawn inside.
    has_header: bool,
    has_inner: bool,
}

/// Squarified treemap layout (Bruls et al.). `sizes` must be sorted in decreasing order.
fn squarify(sizes: &[f64], rect: Rect) -> Vec<Rect> {
    let total: f64 = sizes.iter().sum();
    let (mut x, mut y) = (rect.min.x as f64, rect.min.y as f64);
    let (mut w, mut h) = (rect.width() as f64, rect.height() as f64);
    if total <= 0.0 || w <= 0.0 || h <= 0.0 {
        return Vec::new();
    }
    let scale = w * h / total;
    let areas: Vec<f64> = sizes.iter().map(|s| s * scale).collect();
    let mut out = Vec::with_capacity(sizes.len());

    let mut i = 0;
    while i < areas.len() {
        let short = w.min(h);
        let mut j = i;
        let mut sum = 0.0;
        let mut best = f64::INFINITY;
        while j < areas.len() {
            let s = sum + areas[j];
            let worst = (short * short * areas[i] / (s * s)).max(s * s / (short * short * areas[j]));
            if worst > best {
                break;
            }
            best = worst;
            sum = s;
            j += 1;
        }

        if w >= h {
            let col_w = sum / h;
            let mut cy = y;
            for a in &areas[i..j] {
                let ch = a / col_w;
                out.push(Rect::from_min_size(
                    Pos2::new(x as f32, cy as f32),
                    vec2(col_w as f32, ch as f32),
                ));
                cy += ch;
            }
            x += col_w;
            w -= col_w;
        } else {
            let row_h = sum / w;
            let mut cx = x;
            for a in &areas[i..j] {
                let cw = a / row_h;
                out.push(Rect::from_min_size(
                    Pos2::new(cx as f32, y as f32),
                    vec2(cw as f32, row_h as f32),
                ));
                cx += cw;
            }
            y += row_h;
            h -= row_h;
        }
        i = j;
    }
    out
}

fn layout<'a>(
    node: &'a Node,
    rect: Rect,
    path: &mut Vec<usize>,
    depth: usize,
    cells: &mut Vec<Cell<'a>>,
) {
    let kids: Vec<(usize, &Node)> = node
        .children
        .iter()
        .enumerate()
        .filter(|(_, c)| c.size > 0)
        .collect();
    let sizes: Vec<f64> = kids.iter().map(|(_, c)| c.size as f64).collect();

    for ((idx, child), r) in kids.into_iter().zip(squarify(&sizes, rect)) {
        if r.width() < MIN_CELL || r.height() < MIN_CELL {
            continue;
        }
        path.push(idx);
        let nested = child.is_dir && r.width() > 16.0 && r.height() > 16.0;
        let has_header = nested && r.width() > 50.0 && r.height() > 36.0;
        cells.push(Cell {
            rect: r,
            path: path.clone(),
            depth,
            node: child,
            has_header,
            has_inner: nested,
        });
        if nested {
            let top = if has_header { HEADER } else { 0.0 };
            let inner = Rect::from_min_max(
                r.min + vec2(PAD, PAD + top),
                r.max - vec2(PAD, PAD),
            );
            layout(child, inner, path, depth + 1, cells);
        }
        path.pop();
    }
}

fn extension_color(name: &str) -> Color32 {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "zip" | "7z" | "rar" | "tar" | "gz" | "xz" | "bz2" | "zst" | "iso" | "img" | "dmg"
        | "vmdk" | "vdi" | "qcow2" | "nup" | "pkg" | "deb" | "rpm" | "msi" => {
            Color32::from_rgb(214, 40, 34)
        }
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "wmv" | "flv" | "m4v" => {
            Color32::from_rgb(230, 126, 34)
        }
        "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac" | "opus" => Color32::from_rgb(150, 60, 150),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "bmp" | "tif" | "tiff" | "svg"
        | "raw" | "psd" => Color32::from_rgb(52, 160, 90),
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "odt" | "csv" => {
            Color32::from_rgb(64, 104, 190)
        }
        "rs" | "c" | "h" | "cpp" | "py" | "js" | "ts" | "java" | "go" | "json" | "toml" | "html"
        | "css" | "o" | "a" | "so" | "dll" | "dylib" | "exe" | "class" | "jar" => {
            Color32::from_rgb(36, 150, 160)
        }
        _ => Color32::from_rgb(150, 150, 160),
    }
}

fn lighten(c: Color32) -> Color32 {
    let up = |v: u8| v.saturating_add(40);
    Color32::from_rgb(up(c.r()), up(c.g()), up(c.b()))
}

fn dir_color(depth: usize) -> Color32 {
    if depth % 2 == 0 {
        Color32::from_rgb(52, 84, 160)
    } else {
        Color32::from_rgb(40, 56, 96)
    }
}

pub fn show(ui: &mut Ui, cx: &ChartCtx) -> Option<ChartAction> {
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click());
    let bg = ui.visuals().extreme_bg_color;
    painter.rect_filled(resp.rect, CornerRadius::ZERO, bg);

    let view = cx.root.get(cx.base)?;
    let mut cells = Vec::new();
    layout(
        view,
        resp.rect.shrink(4.0),
        &mut cx.base.to_vec(),
        0,
        &mut cells,
    );

    let hovered = resp
        .hover_pos()
        .and_then(|p| cells.iter().rposition(|c| c.rect.contains(p)));

    for (i, cell) in cells.iter().enumerate() {
        let fill = if cell.node.is_dir {
            dir_color(cell.depth)
        } else {
            extension_color(&cell.node.name)
        };
        let fill = if hovered == Some(i) && !cell.has_inner {
            lighten(fill)
        } else {
            fill
        };
        painter.rect_filled(cell.rect, CornerRadius::ZERO, fill);
        painter.rect_stroke(
            cell.rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, Color32::from_black_alpha(90)),
            StrokeKind::Inside,
        );

        let label_rect = if cell.has_header {
            Some(Rect::from_min_size(cell.rect.min, vec2(cell.rect.width(), HEADER + PAD)))
        } else if !cell.has_inner && cell.rect.width() > 50.0 && cell.rect.height() > 18.0 {
            Some(cell.rect)
        } else {
            None
        };
        if let Some(lr) = label_rect {
            let clipped = painter.with_clip_rect(lr.shrink(2.0));
            let (pos, align) = if cell.has_header {
                (lr.left_center() + vec2(5.0, 0.0), Align2::LEFT_CENTER)
            } else {
                (lr.center(), Align2::CENTER_CENTER)
            };
            clipped.text(
                pos,
                align,
                &cell.node.name,
                FontId::proportional(12.0),
                text_color_on(fill),
            );
        }
    }

    if let Some(sel) = cx.selected
        && let Some(cell) = cells.iter().find(|c| c.path == sel)
    {
        painter.rect_stroke(
            cell.rect,
            CornerRadius::ZERO,
            Stroke::new(2.5, Color32::WHITE),
            StrokeKind::Inside,
        );
    }

    let hovered_path = hovered.map(|i| cells[i].path.clone());
    if let Some(path) = &hovered_path {
        resp.clone().on_hover_ui_at_pointer(|ui| tooltip(ui, cx, path));
    }

    if resp.secondary_clicked() {
        return Some(ChartAction::Up);
    }
    let path = hovered_path?;
    if resp.double_clicked() {
        let is_dir = cx.root.get(&path).is_some_and(|n| n.is_dir);
        return Some(if is_dir {
            ChartAction::Open(path)
        } else {
            ChartAction::Select(path)
        });
    }
    resp.clicked().then_some(ChartAction::Select(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squarify_tiles_the_whole_rect_proportionally() {
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), vec2(400.0, 300.0));
        let sizes = [50.0, 25.0, 15.0, 7.0, 3.0];
        let rects = squarify(&sizes, rect);
        assert_eq!(rects.len(), sizes.len());
        let area: f32 = rects.iter().map(|r| r.area()).sum();
        assert!((area - rect.area()).abs() < 1.0);
        for (r, s) in rects.iter().zip(sizes) {
            assert!((r.area() / rect.area() - s as f32 / 100.0).abs() < 1e-3);
            assert!(rect.expand(0.01).contains_rect(*r));
        }
    }
}
