use egui::epaint::PathShape;
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, vec2};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Back,
    ChevronRight,
    Home,
    Folder,
    Refresh,
    Trash,
    Globe,
    ChevronDown,
    Sun,
    Moon,
    Close,
}

/// Maps the 24x24 design grid onto the target square.
struct Grid {
    origin: Pos2,
    scale: f32,
}

impl Grid {
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(x, y) * self.scale
    }

    fn pts(&self, points: &[(f32, f32)]) -> Vec<Pos2> {
        points.iter().map(|&(x, y)| self.pt(x, y)).collect()
    }
}

/// Polyline whose corners are rounded with quadratic curves of the given radius (in grid units).
fn rounded(points: &[(f32, f32)], radius: f32, closed: bool) -> Vec<(f32, f32)> {
    let n = points.len();
    let mut out = Vec::new();
    for i in 0..n {
        let p = points[i];
        if !closed && (i == 0 || i == n - 1) {
            out.push(p);
            continue;
        }
        let prev = points[(i + n - 1) % n];
        let next = points[(i + 1) % n];
        let toward = |q: (f32, f32)| {
            let d = ((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)).sqrt().max(1e-3);
            let r = radius.min(d / 2.0);
            (p.0 + (q.0 - p.0) / d * r, p.1 + (q.1 - p.1) / d * r)
        };
        let (a, b) = (toward(prev), toward(next));
        for s in 0..=6 {
            let t = s as f32 / 6.0;
            let u = 1.0 - t;
            out.push((
                u * u * a.0 + 2.0 * u * t * p.0 + t * t * b.0,
                u * u * a.1 + 2.0 * u * t * p.1 + t * t * b.1,
            ));
        }
    }
    out
}

fn arc(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Vec<(f32, f32)> {
    let steps = ((to - from).abs() / 0.12).ceil().max(2.0) as usize;
    (0..=steps)
        .map(|i| {
            let a = from + (to - from) * i as f32 / steps as f32;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

pub fn paint(p: &Painter, icon: Icon, rect: Rect, color: Color32) {
    let g = Grid { origin: rect.min, scale: rect.width() / 24.0 };
    let stroke = Stroke::new((2.0 * g.scale).max(1.2), color);
    let line = |points: Vec<(f32, f32)>, closed: bool| {
        let pts = g.pts(&points);
        let shape = if closed {
            PathShape::closed_line(pts, stroke)
        } else {
            PathShape::line(pts, stroke)
        };
        p.add(Shape::Path(shape));
        // Round caps for open lines.
        if !closed {
            for end in [points.first(), points.last()].into_iter().flatten() {
                p.circle_filled(g.pt(end.0, end.1), stroke.width / 2.0, color);
            }
        }
    };

    match icon {
        Icon::Back => line(vec![(15.0, 5.5), (8.5, 12.0), (15.0, 18.5)], false),
        Icon::Close => {
            line(vec![(6.0, 6.0), (18.0, 18.0)], false);
            line(vec![(18.0, 6.0), (6.0, 18.0)], false);
        }
        Icon::ChevronRight => line(vec![(9.0, 5.5), (15.5, 12.0), (9.0, 18.5)], false),
        Icon::ChevronDown => line(vec![(6.0, 9.0), (12.0, 15.0), (18.0, 9.0)], false),
        Icon::Home => {
            line(rounded(&[(3.0, 11.0), (12.0, 3.0), (21.0, 11.0)], 1.0, false), false);
            line(
                rounded(&[(5.0, 9.5), (5.0, 21.0), (19.0, 21.0), (19.0, 9.5)], 1.6, false),
                false,
            );
            line(vec![(9.5, 21.0), (9.5, 14.0), (14.5, 14.0), (14.5, 21.0)], false);
        }
        Icon::Folder => line(
            rounded(
                &[
                    (2.5, 5.5),
                    (2.5, 19.0),
                    (4.0, 20.0),
                    (20.0, 20.0),
                    (21.5, 18.5),
                    (21.5, 8.5),
                    (20.0, 7.0),
                    (12.0, 7.0),
                    (10.0, 4.0),
                    (4.0, 4.0),
                ],
                1.8,
                true,
            ),
            true,
        ),
        Icon::Refresh => {
            let mut pts = arc(12.0, 12.0, 9.0, 0.0, 1.5 * PI);
            pts.extend([(15.0, 3.4), (17.2, 4.4), (18.7, 5.7), (21.0, 8.0)]);
            line(pts, false);
            line(vec![(21.0, 3.0), (21.0, 8.0), (16.0, 8.0)], false);
        }
        Icon::Trash => {
            line(vec![(3.0, 6.0), (21.0, 6.0)], false);
            line(
                rounded(&[(5.0, 6.0), (5.0, 20.0), (7.0, 22.0), (17.0, 22.0), (19.0, 20.0), (19.0, 6.0)], 1.6, false),
                false,
            );
            line(
                rounded(&[(8.0, 6.0), (8.0, 4.0), (10.0, 2.0), (14.0, 2.0), (16.0, 4.0), (16.0, 6.0)], 1.6, false),
                false,
            );
            line(vec![(10.0, 11.0), (10.0, 17.0)], false);
            line(vec![(14.0, 11.0), (14.0, 17.0)], false);
        }
        Icon::Globe => {
            line(arc(12.0, 12.0, 10.0, 0.0, TAU), true);
            line(vec![(2.0, 12.0), (22.0, 12.0)], false);
            let ellipse: Vec<(f32, f32)> = (0..48)
                .map(|i| {
                    let a = TAU * i as f32 / 48.0;
                    (12.0 + 4.6 * a.cos(), 12.0 + 10.0 * a.sin())
                })
                .collect();
            line(ellipse, true);
        }
        Icon::Sun => {
            line(arc(12.0, 12.0, 4.2, 0.0, TAU), true);
            for i in 0..8 {
                let a = TAU * i as f32 / 8.0;
                let (c, s) = (a.cos(), a.sin());
                line(
                    vec![(12.0 + 7.6 * c, 12.0 + 7.6 * s), (12.0 + 10.0 * c, 12.0 + 10.0 * s)],
                    false,
                );
            }
        }
        Icon::Moon => {
            // Two separate strokes: a joined acute cusp would produce a long miter spike.
            line(arc(12.0, 12.0, 9.0, 0.0, 1.5 * PI), false);
            line(arc(16.5, 7.5, 6.364, -0.75 * PI, -1.75 * PI), false);
        }
    }
}
