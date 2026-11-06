//! Flat custom controls: knob, algorithm diagram, envelope graph, piano keyboard, level bars.

use crate::fm::{is_carrier, modulators_of, ALGORITHMS};
use crate::theme::Theme;
use egui::{self, pos2, vec2, Align2, Color32, CornerRadius, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui};
use std::collections::BTreeSet;

/// A flat rotary control. Drag up/down, Shift for fine, double-click to reset.
pub fn knob(ui: &mut Ui, t: &Theme, label: &str, value: &mut f32, min: f32, max: f32, default: f32, fmt: &dyn Fn(f32) -> String, size: f32) -> Response {
    let desired = vec2(size + 24.0, size + 14.0 + crate::theme::fs(28.0));
    let (rect, mut resp) = ui.allocate_exact_size(desired, Sense::click_and_drag());
    let center = pos2(rect.center().x, rect.top() + 4.0 + size / 2.0);
    if resp.dragged() {
        let fine = ui.input(|i| i.modifiers.shift);
        let speed = if fine { 0.0008 } else { 0.005 };
        let d = -resp.drag_delta().y + resp.drag_delta().x * 0.3;
        let nv = (*value + d * speed * (max - min)).clamp(min, max);
        if nv != *value {
            *value = nv;
            resp.mark_changed();
        }
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let fine = ui.input(|i| i.modifiers.shift);
            let nv = (*value + scroll * if fine { 0.0002 } else { 0.0015 } * (max - min)).clamp(min, max);
            if nv != *value {
                *value = nv;
                resp.mark_changed();
            }
        }
    }
    if resp.double_clicked() {
        *value = default;
        resp.mark_changed();
    }
    let p = ui.painter();
    let r = size / 2.0;
    let frac = ((*value - min) / (max - min)).clamp(0.0, 1.0);
    let a0 = std::f32::consts::PI * 0.75;
    let sweep = std::f32::consts::PI * 1.5;
    let arc = |from: f32, to: f32, radius: f32| -> Vec<Pos2> {
        let steps = 40;
        (0..=steps)
            .map(|i| {
                let a = a0 + sweep * (from + (to - from) * i as f32 / steps as f32);
                center + vec2(a.cos(), a.sin()) * radius
            })
            .collect()
    };
    p.circle_filled(center, r - 6.0, t.raised);
    p.circle_stroke(center, r - 6.0, Stroke::new(1.0, t.border));
    p.add(Shape::line(arc(0.0, 1.0, r - 1.5), Stroke::new(3.0, t.border)));
    // bipolar controls fill from the middle
    let bipolar = min < 0.0 && max > 0.0;
    let (fa, fb) = if bipolar { let z = -min / (max - min); (z.min(frac), z.max(frac)) } else { (0.0, frac) };
    if fb - fa > 0.002 {
        p.add(Shape::line(arc(fa, fb, r - 1.5), Stroke::new(3.0, t.accent)));
    }
    let a = a0 + sweep * frac;
    let dir = vec2(a.cos(), a.sin());
    p.line_segment([center + dir * (r * 0.18), center + dir * (r - 9.0)], Stroke::new(2.0, t.text));
    let active = resp.hovered() || resp.dragged();
    p.text(pos2(rect.center().x, rect.top() + size + 8.0), Align2::CENTER_TOP, label, crate::theme::fp(12.0), if active { t.text } else { t.text_dim });
    p.text(pos2(rect.center().x, rect.top() + size + 9.0 + crate::theme::fs(15.0)), Align2::CENTER_TOP, fmt(*value), crate::theme::fm(11.0), if active { t.accent } else { t.text_faint });
    resp.on_hover_text("Drag to adjust · Shift for fine steps · Double-click to reset")
}

/// Six-operator routing diagram. `levels` gives a 0..1 activity value per operator (1 = operator 1).
pub fn algorithm_diagram(ui: &mut Ui, t: &Theme, alg: usize, enabled: &[bool; 6], levels: &[f32; 6], size: Vec2Size) -> Option<usize> {
    let (rect, resp) = ui.allocate_exact_size(vec2(size.0, size.1), Sense::click());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, CornerRadius::same(4), t.panel);
    // layout
    let mut pos: [Option<(i32, i32)>; 6] = [None; 6];
    let mut next_col = 0i32;
    fn place(op: usize, depth: i32, col: i32, alg: usize, pos: &mut [Option<(i32, i32)>; 6], next_col: &mut i32) {
        pos[op] = Some((col, depth));
        *next_col = (*next_col).max(col + 1);
        let mods = modulators_of(alg, op);
        let mut first = true;
        for m in mods {
            if m == op || pos[m].is_some() {
                continue;
            }
            let c = if first { col } else { *next_col };
            first = false;
            place(m, depth + 1, c, alg, pos, next_col);
        }
    }
    for op in 0..6 {
        if is_carrier(alg, op) && pos[op].is_none() {
            let c = next_col;
            place(op, 0, c, alg, &mut pos, &mut next_col);
        }
    }
    let cols = next_col.max(1);
    let rows = pos.iter().flatten().map(|p| p.1).max().unwrap_or(0) + 1;
    let box_s = ((rect.width() - 20.0) / cols as f32 * 0.62).min((rect.height() - 34.0) / rows as f32 * 0.62).clamp(14.0, 30.0);
    let cell_w = (rect.width() - 16.0) / cols as f32;
    let cell_h = ((rect.height() - 34.0) / rows as f32).min(box_s * 1.9);
    let total_h = cell_h * rows as f32;
    let base_y = rect.center().y + total_h / 2.0 - cell_h / 2.0 - 6.0;
    let center_of = |op: usize| -> Pos2 {
        let (c, d) = pos[op].unwrap_or((0, 0));
        pos2(rect.left() + 8.0 + cell_w * (c as f32 + 0.5), base_y - d as f32 * cell_h)
    };
    // connections
    for op in 0..6 {
        for m in modulators_of(alg, op) {
            let a = center_of(m);
            let b = center_of(op);
            if m == op {
                // feedback loop on the right side of the box
                let h = box_s / 2.0;
                let pts = vec![a + vec2(0.0, -h), a + vec2(0.0, -h - 5.0), a + vec2(h + 6.0, -h - 5.0), a + vec2(h + 6.0, h + 5.0), a + vec2(0.0, h + 5.0)];
                p.add(Shape::line(pts, Stroke::new(1.2, t.accent2)));
            } else {
                p.line_segment([a + vec2(0.0, box_s / 2.0), b - vec2(0.0, box_s / 2.0)], Stroke::new(1.2, t.text_faint));
            }
        }
    }
    // output bus
    let carriers: Vec<usize> = (0..6).filter(|&o| is_carrier(alg, o)).collect();
    if let (Some(&f), Some(&l)) = (carriers.first(), carriers.last()) {
        let y = base_y + box_s / 2.0 + 8.0;
        let x0 = carriers.iter().map(|&c| center_of(c).x).fold(f32::MAX, f32::min);
        let x1 = carriers.iter().map(|&c| center_of(c).x).fold(f32::MIN, f32::max);
        let _ = (f, l);
        for &c in &carriers {
            let cc = center_of(c);
            p.line_segment([cc + vec2(0.0, box_s / 2.0), pos2(cc.x, y)], Stroke::new(1.2, t.accent));
        }
        p.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.2, t.accent));
    }
    let hover = resp.hover_pos();
    let mut hovered = None;
    for op in 0..6 {
        let c = center_of(op);
        let r = Rect::from_center_size(c, vec2(box_s, box_s));
        let carrier = is_carrier(alg, op);
        let lvl = levels[op].clamp(0.0, 1.0);
        let fill = if !enabled[op] { t.panel } else if carrier { t.accent.linear_multiply(0.25 + 0.6 * lvl) } else { t.raised };
        p.rect_filled(r, CornerRadius::same(3), fill);
        let stroke = if hover.map(|h| r.contains(h)).unwrap_or(false) {
            hovered = Some(op);
            Stroke::new(1.5, t.text)
        } else if carrier {
            Stroke::new(1.0, t.accent)
        } else {
            Stroke::new(1.0, t.border)
        };
        p.rect_stroke(r, CornerRadius::same(3), stroke, StrokeKind::Inside);
        let col = if enabled[op] { t.text } else { t.text_faint };
        p.text(c, Align2::CENTER_CENTER, format!("{}", op + 1), crate::theme::fm(box_s * 0.5), col);
    }
    p.text(pos2(rect.left() + 8.0, rect.top() + 6.0), Align2::LEFT_TOP, format!("ALGORITHM {}", alg + 1), crate::theme::fm(10.5), t.text_dim);
    let _ = ALGORITHMS;
    if resp.clicked() { hovered } else { None }
}

pub struct Vec2Size(pub f32, pub f32);

/// Envelope shape from four rates and four levels (0..99). `sustain_at` marks the key-up point.
pub fn envelope_graph(ui: &mut Ui, t: &Theme, rates: [u8; 4], levels: [u8; 4], size: egui::Vec2, start_level: Option<u8>, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, CornerRadius::same(3), t.panel);
    let seg = |r: u8| -> f32 { 0.15 + ((99 - r.min(99)) as f32 / 99.0).powf(1.6) * 2.0 };
    let w = [seg(rates[0]), seg(rates[1]), seg(rates[2]), 0.8, seg(rates[3])];
    let total: f32 = w.iter().sum();
    let inner = rect.shrink2(vec2(6.0, 6.0));
    let x_of = |acc: f32| inner.left() + acc / total * inner.width();
    let y_of = |l: u8| inner.bottom() - l.min(99) as f32 / 99.0 * inner.height();
    let start = start_level.unwrap_or(levels[3]);
    let mut acc = 0.0;
    let mut pts = vec![pos2(x_of(0.0), y_of(start))];
    acc += w[0];
    pts.push(pos2(x_of(acc), y_of(levels[0])));
    acc += w[1];
    pts.push(pos2(x_of(acc), y_of(levels[1])));
    acc += w[2];
    pts.push(pos2(x_of(acc), y_of(levels[2])));
    let key_up = acc + w[3];
    pts.push(pos2(x_of(key_up), y_of(levels[2])));
    pts.push(pos2(x_of(key_up + w[4]), y_of(levels[3])));
    // fill under curve
    let mut poly = pts.clone();
    poly.push(pos2(inner.right(), inner.bottom()));
    poly.push(pos2(inner.left(), inner.bottom()));
    p.add(Shape::convex_polygon(poly, color.linear_multiply(0.08), Stroke::NONE));
    p.line_segment([pos2(x_of(key_up), inner.top()), pos2(x_of(key_up), inner.bottom())], Stroke::new(1.0, t.border));
    p.add(Shape::line(pts.clone(), Stroke::new(1.5, color)));
    for q in &pts[1..pts.len() - 1] {
        p.circle_filled(*q, 2.2, color);
    }
    resp
}

/// Horizontal level bar (for operator outputs).
pub fn level_bar(ui: &mut Ui, t: &Theme, frac: f32, width: f32, on: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 6.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(2), t.panel);
    let mut r = rect;
    r.set_width(rect.width() * frac.clamp(0.0, 1.0));
    p.rect_filled(r, CornerRadius::same(2), if on { t.accent } else { t.text_faint });
}

/// Output peak meter.
pub fn peak_meter(ui: &mut Ui, t: &Theme, peak: f32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 8.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(2), t.bg);
    p.rect_stroke(rect, CornerRadius::same(2), Stroke::new(1.0, t.border), StrokeKind::Inside);
    let db = 20.0 * peak.max(1e-5).log10();
    let frac = ((db + 48.0) / 48.0).clamp(0.0, 1.0);
    let mut r = rect;
    r.set_width(rect.width() * frac);
    let col = if db > -1.0 { t.danger } else if db > -9.0 { t.star } else { t.accent };
    p.rect_filled(r, CornerRadius::same(2), col);
}

pub struct KeyboardOut {
    pub pressed: Option<(u8, u8)>,
    pub released: Option<u8>,
}

/// A flat piano keyboard. `held` notes light up. Returns presses from the mouse.
pub fn keyboard(ui: &mut Ui, t: &Theme, first_note: u8, octaves: u8, held: &BTreeSet<u8>, mouse_note: &mut Option<u8>, height: f32) -> KeyboardOut {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
    let p = ui.painter_at(rect);
    let whites_per_oct = 7.0;
    let n_white = (octaves as f32 * whites_per_oct + 1.0) as usize;
    let ww = rect.width() / n_white as f32;
    let is_black = |n: u8| matches!(n % 12, 1 | 3 | 6 | 8 | 10);
    let last = first_note + octaves * 12;
    let mut white_rects = Vec::new();
    let mut black_rects = Vec::new();
    let mut wi = 0usize;
    for n in first_note..=last {
        if !is_black(n) {
            let r = Rect::from_min_size(pos2(rect.left() + wi as f32 * ww, rect.top()), vec2(ww, rect.height()));
            white_rects.push((n, r));
            wi += 1;
        }
    }
    for n in first_note..=last {
        if is_black(n) {
            // sits between previous white and next white
            if let Some((_, wr)) = white_rects.iter().find(|(wn, _)| *wn == n - 1) {
                let bw = ww * 0.62;
                let r = Rect::from_min_size(pos2(wr.right() - bw / 2.0, rect.top()), vec2(bw, rect.height() * 0.6));
                black_rects.push((n, r));
            }
        }
    }
    for (n, r) in &white_rects {
        let on = held.contains(n);
        p.rect_filled(r.shrink2(vec2(0.5, 0.0)), CornerRadius { nw: 0, ne: 0, sw: 3, se: 3 }, if on { t.accent } else { t.key_white });
        if n % 12 == 0 {
            p.text(pos2(r.center().x, r.bottom() - 4.0), Align2::CENTER_BOTTOM, format!("C{}", (*n as i32) / 12 - 1), crate::theme::fp(9.5), Color32::from_gray(110));
        }
    }
    for (n, r) in &black_rects {
        let on = held.contains(n);
        p.rect_filled(*r, CornerRadius { nw: 0, ne: 0, sw: 2, se: 2 }, if on { t.accent.linear_multiply(0.8) } else { t.key_black });
    }
    p.line_segment([rect.left_top(), rect.right_top()], Stroke::new(2.0, t.accent2));

    let mut out = KeyboardOut { pressed: None, released: None };
    let pointer_down = resp.is_pointer_button_down_on();
    let find = |pos: Pos2| -> Option<(u8, f32)> {
        for (n, r) in &black_rects {
            if r.contains(pos) {
                return Some((*n, (pos.y - r.top()) / r.height()));
            }
        }
        for (n, r) in &white_rects {
            if r.contains(pos) {
                return Some((*n, (pos.y - r.top()) / r.height()));
            }
        }
        None
    };
    if pointer_down {
        if let Some(pos) = resp.interact_pointer_pos() {
            if let Some((n, y)) = find(pos) {
                if *mouse_note != Some(n) {
                    if let Some(old) = mouse_note.take() {
                        out.released = Some(old);
                    }
                    let vel = (40.0 + y.clamp(0.0, 1.0) * 87.0) as u8;
                    out.pressed = Some((n, vel.min(127)));
                    *mouse_note = Some(n);
                }
            }
        }
    } else if let Some(old) = mouse_note.take() {
        out.released = Some(old);
    }
    out
}
