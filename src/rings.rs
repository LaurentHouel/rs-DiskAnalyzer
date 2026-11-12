use crate::chart::{ChartAction, ChartCtx, text_color_on, tooltip};
use crate::scan::Node;
use crate::util::format_size;
use egui::ecolor::Hsva;
use egui::epaint::Vertex;
use egui::{Align2, Color32, FontId, Mesh, Pos2, Sense, Shape, Stroke, Ui, vec2};
use std::f32::consts::{FRAC_PI_2, TAU};

const LEVELS: usize = 6;
const MIN_ANGLE: f32 = 0.012;

struct Seg<'a> {
    node: &'a Node,
    path: Vec<usize>,
    level: usize,
    a0: f32,
    a1: f32,
    color: Color32,
}

fn collect<'a>(
    node: &'a Node,
    path: &mut Vec<usize>,
    level: usize,
    (a0, a1): (f32, f32),
    hue: f32,
    segs: &mut Vec<Seg<'a>>,
) {
    if level >= LEVELS {
        return;
    }
    let total: u64 = node.children.iter().map(|c| c.size).sum();
    if total == 0 {
        return;
    }
    let mut start = a0;
    for (i, child) in node.children.iter().enumerate() {
        let span = (a1 - a0) * child.size as f32 / total as f32;
        let end = start + span;
        if child.size > 0 && span >= MIN_ANGLE {
            let h = if level == 0 { (start + span / 2.0) / TAU } else { hue };
            let value = (0.95 - level as f32 * 0.08).max(0.5);
            path.push(i);
            segs.push(Seg {
                node: child,
                path: path.clone(),
                level,
                a0: start,
                a1: end,
                color: Hsva::new(h, 0.55, value, 1.0).into(),
            });
            if child.is_dir {
                collect(child, path, level + 1, (start, end), h, segs);
            }
            path.pop();
        }
        start = end;
    }
}

/// Angles start at 12 o'clock and grow clockwise.
fn polar(center: Pos2, r: f32, a: f32) -> Pos2 {
    center + vec2((a - FRAC_PI_2).cos(), (a - FRAC_PI_2).sin()) * r
}

fn sector(center: Pos2, r0: f32, r1: f32, a0: f32, a1: f32, color: Color32) -> (Mesh, Vec<Pos2>) {
    let steps = (((a1 - a0) / 0.04).ceil() as usize).max(1);
    let mut mesh = Mesh::default();
    let mut inner = Vec::with_capacity(steps + 1);
    let mut outer = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let a = a0 + (a1 - a0) * i as f32 / steps as f32;
        let (pi, po) = (polar(center, r0, a), polar(center, r1, a));
        for pos in [pi, po] {
            mesh.vertices.push(Vertex {
                pos,
                uv: Pos2::ZERO,
                color,
            });
        }
        inner.push(pi);
        outer.push(po);
    }
    for i in 0..steps as u32 {
        let k = 2 * i;
        mesh.add_triangle(k, k + 1, k + 2);
        mesh.add_triangle(k + 1, k + 3, k + 2);
    }
    outer.reverse();
    inner.extend(outer);
    (mesh, inner)
}

pub fn show(ui: &mut Ui, cx: &ChartCtx) -> Option<ChartAction> {
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click());
    let view = cx.root.get(cx.base)?;
    let center = resp.rect.center();
    let radius = (resp.rect.width().min(resp.rect.height()) / 2.0 - 8.0).max(10.0);
    let ring = radius / (LEVELS as f32 + 1.0);

    let mut segs = Vec::new();
    collect(view, &mut cx.base.to_vec(), 0, (0.0, TAU), 0.0, &mut segs);

    let hovered = resp.hover_pos().and_then(|p| {
        let d = p - center;
        let r = d.length();
        let a = (d.y.atan2(d.x) + FRAC_PI_2).rem_euclid(TAU);
        segs.iter().position(|s| {
            let (r0, r1) = ((s.level + 1) as f32 * ring, (s.level + 2) as f32 * ring);
            r >= r0 && r < r1 && a >= s.a0 && a < s.a1
        })
    });
    let over_center = resp
        .hover_pos()
        .is_some_and(|p| (p - center).length() < ring);

    let outline = ui.visuals().extreme_bg_color;
    for (i, s) in segs.iter().enumerate() {
        let (r0, r1) = ((s.level + 1) as f32 * ring, (s.level + 2) as f32 * ring);
        let color = if hovered == Some(i) {
            let up = |v: u8| v.saturating_add(35);
            Color32::from_rgb(up(s.color.r()), up(s.color.g()), up(s.color.b()))
        } else {
            s.color
        };
        let (mesh, outline_pts) = sector(center, r0, r1, s.a0, s.a1, color);
        painter.add(Shape::mesh(mesh));
        painter.add(Shape::closed_line(outline_pts, Stroke::new(1.0, outline)));

        let mid = (r0 + r1) / 2.0;
        let arc_len = (s.a1 - s.a0) * mid;
        if arc_len > 70.0 {
            painter.text(
                polar(center, mid, (s.a0 + s.a1) / 2.0),
                Align2::CENTER_CENTER,
                &s.node.name,
                FontId::proportional(11.0),
                text_color_on(color),
            );
        }
    }

    if let Some(sel) = cx.selected
        && let Some(s) = segs.iter().find(|s| s.path == sel)
    {
        let (r0, r1) = ((s.level + 1) as f32 * ring, (s.level + 2) as f32 * ring);
        let (_, pts) = sector(center, r0, r1, s.a0, s.a1, Color32::WHITE);
        painter.add(Shape::closed_line(pts, Stroke::new(2.5, Color32::WHITE)));
    }

    let center_fill = ui.visuals().widgets.inactive.bg_fill;
    painter.circle_filled(
        center,
        ring - 1.0,
        if over_center && !cx.base.is_empty() {
            ui.visuals().widgets.hovered.bg_fill
        } else {
            center_fill
        },
    );
    let text_color = ui.visuals().strong_text_color();
    painter.with_clip_rect(egui::Rect::from_center_size(center, vec2(ring * 2.0, ring * 2.0)))
        .text(
            center - vec2(0.0, 7.0),
            Align2::CENTER_CENTER,
            &view.name,
            FontId::proportional(12.0),
            text_color,
        );
    painter.text(
        center + vec2(0.0, 8.0),
        Align2::CENTER_CENTER,
        format_size(view.size),
        FontId::proportional(11.0),
        ui.visuals().text_color(),
    );

    let hovered_path = hovered.map(|i| segs[i].path.clone());
    if let Some(path) = &hovered_path {
        resp.clone().on_hover_ui_at_pointer(|ui| tooltip(ui, cx, path));
    }

    if resp.secondary_clicked() || (resp.clicked() && over_center) {
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
