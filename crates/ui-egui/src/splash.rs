//! Original procedural launch artwork and the borderless desktop-to-editor transition.

use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2, pos2, vec2};
use web_time::{Duration, Instant};

pub const SIZE: Vec2 = vec2(760.0, 460.0);

pub struct Splash {
    started: Instant,
    paints: u8,
    last_frame: Option<u64>,
    finished: bool,
}

impl Default for Splash {
    fn default() -> Self {
        Self { started: Instant::now(), paints: 0, last_frame: None, finished: false }
    }
}

impl Splash {
    /// Wait for two submitted frames before starting work that might block the event loop.
    pub fn presented(&self) -> bool {
        self.paints >= 2
    }

    pub fn ready(&self) -> bool {
        self.paints >= 3 && self.started.elapsed() >= Duration::from_millis(650)
    }

    pub fn finished(&self) -> bool {
        self.finished
    }

    pub fn finish(&mut self, ctx: &egui::Context) {
        if self.finished {
            return;
        }
        self.finished = true;
        let desired = vec2(1680.0, 1020.0);
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(vec2(960.0, 600.0)));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(desired));
        let (monitor, outer) = ctx.input(|i| (i.viewport().monitor_size, i.viewport().outer_rect));
        if monitor.is_some_and(|m| desired.x > m.x || desired.y > m.y - 64.0) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        } else if let Some(outer) = outer {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(outer.center() - desired * 0.5));
        }
        ctx.request_repaint();
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, status: &str) {
        let frame = ui.ctx().cumulative_frame_nr();
        if self.last_frame != Some(frame) {
            self.paints = self.paints.saturating_add(1);
            self.last_frame = Some(frame);
        }
        paint(ui, status, self.started.elapsed().as_secs_f32());
        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }
}

/// No bitmap or third-party artwork: all ribbons are calculated from our own parametric curves.
pub fn paint(ui: &mut egui::Ui, status: &str, seconds: f32) {
    let rect = ui.ctx().content_rect();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(17, 20, 25));
    let scale = (rect.width() / SIZE.x).min(rect.height() / SIZE.y).max(0.01);
    let origin = rect.center() - SIZE * (scale * 0.5);
    let point = |x: f32, y: f32| origin + vec2(x, y) * scale;
    let accent = Color32::from_rgb(112, 226, 200);
    let text = Color32::from_rgb(238, 242, 246);
    let dim = Color32::from_rgb(148, 158, 174);

    for band in 0..3 {
        let mut mesh = egui::Mesh::default();
        let color = match band {
            0 => Color32::from_rgb(97, 118, 226),
            1 => Color32::from_rgb(92, 211, 192),
            _ => Color32::from_rgb(199, 169, 243),
        };
        for step in 0..=100 {
            let t = step as f32 / 100.0;
            let phase = t * 5.2 + band as f32 * 0.75;
            let x = 405.0 + t * 370.0;
            let y = 195.0 + phase.sin() * 97.0 + band as f32 * 32.0;
            let width = 18.0 + (t * std::f32::consts::PI).sin() * 51.0;
            let alpha = (110.0 + t * 145.0) as u8;
            let tint = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);
            mesh.colored_vertex(point(x, y - width * 0.5), tint);
            mesh.colored_vertex(point(x + 11.0, y + width * 0.5), tint);
            if step > 0 {
                let n = step * 2;
                mesh.add_triangle(n - 2, n - 1, n);
                mesh.add_triangle(n - 1, n + 1, n);
            }
        }
        painter.add(egui::Shape::mesh(mesh));
    }

    // A small motion-path symbol, drawn from first principles rather than a third-party mark.
    let mark = Rect::from_min_size(point(36.0, 34.0), vec2(38.0, 38.0) * scale);
    painter.rect_stroke(mark, 9.0 * scale, Stroke::new(1.5 * scale, accent), egui::StrokeKind::Inside);
    painter.line_segment([point(45.0, 59.0), point(54.0, 46.0)], Stroke::new(2.0 * scale, accent));
    painter.line_segment([point(54.0, 46.0), point(66.0, 53.0)], Stroke::new(2.0 * scale, accent));
    for (x, y) in [(45.0, 59.0), (54.0, 46.0), (66.0, 53.0)] {
        painter.circle_filled(point(x, y), 2.6 * scale, accent);
    }
    let label = |at: Pos2, size: f32, value: &str, color| {
        painter.text(at, egui::Align2::LEFT_TOP, value, FontId::proportional(size * scale), color);
    };
    label(point(36.0, 110.0), 37.0, "EffectCraft", text);
    label(point(38.0, 161.0), 14.0, "Motion, made yours.", dim);
    label(point(38.0, 201.0), 12.0, &format!("Version {}", env!("CARGO_PKG_VERSION")), dim);
    label(point(38.0, 346.0), 12.0, status, text);
    let track = Rect::from_min_size(point(38.0, 376.0), vec2(282.0, 2.0) * scale);
    painter.rect_filled(track, 0.0, Color32::from_rgb(47, 55, 65));
    // Indeterminate, with no fabricated percentage or promise about remaining load time.
    let phase = if seconds.is_finite() { (seconds * 0.55).rem_euclid(1.0) } else { 0.0 };
    let x = track.left() + (track.width() + 70.0 * scale) * phase - 70.0 * scale;
    let segment = Rect::from_min_size(pos2(x, track.top()), vec2(70.0, 2.0) * scale).intersect(track);
    painter.rect_filled(segment, 0.0, accent);
    label(point(38.0, 411.0), 10.0, "Open-source motion graphics · MIT / Apache-2.0", dim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_waits_for_presented_frames_and_editor_transition_is_once() {
        let ctx = egui::Context::default();
        let mut splash = Splash { started: Instant::now() - Duration::from_secs(1), ..Default::default() };
        assert!(!splash.presented());
        assert!(!splash.ready());
        let _ = ctx.run_ui(Default::default(), |ui| {
            splash.ui(ui, "Loading…");
            splash.ui(ui, "Loading…"); // Repainting within one frame cannot advance startup.
        });
        assert!(!splash.presented());
        let _ = ctx.run_ui(Default::default(), |ui| splash.ui(ui, "Loading…"));
        assert!(splash.presented());
        assert!(!splash.ready());
        let _ = ctx.run_ui(Default::default(), |ui| splash.ui(ui, "Preparing…"));
        assert!(splash.ready());
        let output = ctx.run_ui(Default::default(), |ui| {
            splash.finish(ui.ctx());
            splash.finish(ui.ctx());
        });
        assert!(splash.finished());
        let commands = &output.viewport_output.get(&egui::ViewportId::ROOT).unwrap().commands;
        assert_eq!(commands.iter().filter(|c| **c == egui::ViewportCommand::Decorations(true)).count(), 1);
        assert!(commands.contains(&egui::ViewportCommand::Resizable(true)));
        assert!(commands.contains(&egui::ViewportCommand::InnerSize(vec2(1680.0, 1020.0))));
    }
}
