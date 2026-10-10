//! Small immediate-mode controls shared by the Lumetri Scopes, Footage, Media Browser, Metadata,
//! Progress and Content-Aware Fill panels. Each registers its automation id.

use egui::{Align2, Rect, pos2, vec2};

use crate::theme::Tokens;
use crate::{EffectcraftApp, widgets};

/// A dropdown; returns the newly chosen index.
pub fn dropdown(app: &mut EffectcraftApp, ui: &mut egui::Ui, r: Rect, auto: &str, options: &[&str], cur: usize) -> Option<usize> {
    let t = app.tokens;
    let id = egui::Id::new(("kit-dd", auto));
    let text = options.get(cur).copied().unwrap_or("");
    if widgets::dropdown(ui, r, text, &t, id).clicked() {
        widgets::open_popup(ui, id);
    }
    app.auto.add(auto, r, text);
    let opts: Vec<String> = options.iter().map(|s| s.to_string()).collect();
    widgets::popup_menu(ui, id, r.left_bottom(), &opts, Some(cur)).filter(|i| *i != cur)
}

/// A checkbox with a label; returns the new state.
pub fn checkbox(app: &mut EffectcraftApp, ui: &mut egui::Ui, at: egui::Pos2, auto: &str, text: &str, on: bool) -> bool {
    let t = app.tokens;
    let r = Rect::from_min_size(at, vec2(16.0, 16.0));
    let width = ui.painter().layout_no_wrap(text.to_owned(), Tokens::ui(12.0), t.text).size().x;
    let full = Rect::from_min_size(at, vec2((22.0 + width).min((ui.clip_rect().right() - at.x).max(16.0)), 16.0));
    // Both the square and its label are clickable, with the same hit area exposed to agents.
    let response = ui.interact(full, egui::Id::new(("kit-cb", auto)), egui::Sense::click());
    let square = widgets::checkbox(ui, r, on, &t, egui::Id::new(("kit-cb-paint", auto)));
    widgets::text_fit(ui.painter(), pos2(r.max.x + 6.0, r.center().y), Align2::LEFT_CENTER, text, Tokens::ui(12.0), (full.width() - 22.0).max(0.0), t.text);
    let clicked = response.clicked() || square.clicked();
    app.auto.add(auto, full, text);
    if clicked { !on } else { on }
}

/// A text button; true when clicked.
pub fn button(app: &mut EffectcraftApp, ui: &mut egui::Ui, r: Rect, auto: &str, text: &str, primary: bool) -> bool {
    let t = app.tokens;
    let clicked = widgets::text_button(ui, r, text, primary, &t, egui::Id::new(("kit-btn", auto))).clicked();
    app.auto.add(auto, r, text);
    clicked
}

/// A dimmed label at (x, y-row).
pub fn label(ui: &egui::Ui, at: egui::Pos2, text: &str, t: &Tokens) {
    widgets::text_fit(
        ui.painter(),
        pos2(at.x, at.y + 9.0),
        Align2::LEFT_CENTER,
        text,
        Tokens::ui(12.0),
        (ui.clip_rect().right() - at.x - 8.0).max(0.0),
        t.text_dim,
    );
}

/// A draggable number; returns the new value.
#[allow(clippy::too_many_arguments)]
pub fn number(app: &mut EffectcraftApp, ui: &mut egui::Ui, at: egui::Pos2, auto: &str, v: f64, speed: f64, range: (f64, f64), dec: usize, suffix: &str) -> f64 {
    let t = app.tokens;
    let (r, nv, _) = widgets::hot_number_at(ui, at, egui::Id::new(("kit-n", auto)), v, speed, range, dec, suffix, &t);
    app.auto.add(auto, r, &format!("{v}"));
    nv.unwrap_or(v)
}

/// Run a command, reporting errors in the status bar.
pub fn exec(app: &mut EffectcraftApp, id: &str, p: serde_json::Value) -> Option<serde_json::Value> {
    match app.session.execute(id, p) {
        Ok(v) => Some(v),
        Err(e) => {
            app.ui.status = e.to_string();
            None
        }
    }
}

/// Format seconds as `m:ss.ff`.
pub fn seconds(s: f64) -> String {
    let m = (s / 60.0).floor();
    format!("{}:{:05.2}", m as i64, s - m * 60.0)
}
