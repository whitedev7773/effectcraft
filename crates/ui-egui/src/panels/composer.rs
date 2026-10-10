//! Original Motion Composer browser, transition editor and animation tool launcher.

use effectcraft_engine::composer;
use egui::{Color32, Rect, Response, Sense, Stroke, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::panel_kit;
use crate::EffectcraftApp;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ComposerState {
    pub tab: String,
    pub search: String,
    pub preset: String,
    pub mode: String,
    pub duration: f64,
    pub amount: f64,
    pub influence: f64,
    pub stagger: f64,
    pub descending: bool,
    pub at_current: bool,
    pub playing: bool,
    pub shift: f64,
}

impl Default for ComposerState {
    fn default() -> Self {
        Self { tab: "browse".into(), search: String::new(), preset: "fade".into(), mode: "in".into(),
            duration: 0.6, amount: 100.0, influence: 60.0, stagger: 0.0, descending: false,
            at_current: false, playing: true, shift: 0.1 }
    }
}

fn auto(app: &mut EffectcraftApp, ui: &egui::Ui, id: &str, r: &Response, label: &str) {
    let rect = r.rect.intersect(ui.clip_rect());
    if rect.is_positive() { app.auto.add(id, rect, label); }
}

fn button(app: &mut EffectcraftApp, ui: &mut egui::Ui, id: &str, label: &str) -> bool {
    let r = ui.button(label);
    auto(app, ui, id, &r, label);
    r.clicked()
}

fn number(app: &mut EffectcraftApp, ui: &mut egui::Ui, id: &str, label: &str, value: &mut f64, range: std::ops::RangeInclusive<f64>) {
    ui.horizontal(|ui| {
        ui.label(label);
        let r = ui.add(egui::DragValue::new(value).speed(0.01).range(range));
        auto(app, ui, id, &r, &value.to_string());
    });
}

fn settings(app: &mut EffectcraftApp, ui: &mut egui::Ui, st: &mut ComposerState) {
    number(app, ui, "composer.duration", "Duration (s)", &mut st.duration, 0.01..=3600.0);
    number(app, ui, "composer.amount", "Distance / angle", &mut st.amount, -100000.0..=100000.0);
    number(app, ui, "composer.influence", "Easing (%)", &mut st.influence, 0.1..=100.0);
    number(app, ui, "composer.stagger", "Stagger (s)", &mut st.stagger, 0.0..=3600.0);
    let r = ui.checkbox(&mut st.descending, "Reverse layer order");
    auto(app, ui, "composer.descending", &r, "Reverse layer order");
    let r = ui.checkbox(&mut st.at_current, "Place at current time");
    auto(app, ui, "composer.atCurrent", &r, "Place at current time");
}

fn params(st: &ComposerState) -> serde_json::Value {
    json!({"preset": st.preset, "mode": st.mode, "duration": st.duration, "amount": st.amount,
        "influence": st.influence, "stagger": st.stagger,
        "order": if st.descending {"descending"} else {"ascending"}, "at": if st.at_current {"current"} else {"edge"}})
}

fn preview(app: &mut EffectcraftApp, ui: &mut egui::Ui, st: &ComposerState) {
    let (r, response) = ui.allocate_exact_size(vec2(ui.available_width().max(0.0), 100.0), Sense::hover());
    let p = ui.painter_at(r);
    let bg = Color32::from_rgb(25, 31, 41);
    p.rect_filled(r, 4.0, bg);
    let u = if st.playing { ui.input(|i| (i.time % 1.6).min(1.0)) } else { 1.0 };
    let [x, y, scale, opacity, rotation] = composer::preview(&st.preset, &st.mode, u);
    let center = r.center() + vec2(x as f32, y as f32) * 32.0;
    let half = 13.0 * scale.max(0.0) as f32;
    let (sn, cs) = (rotation as f32).to_radians().sin_cos();
    let points = [vec2(-half, -half), vec2(half, -half), vec2(half, half), vec2(-half, half)].map(|v|
        center + vec2(v.x * cs - v.y * sn, v.x * sn + v.y * cs));
    p.add(egui::Shape::convex_polygon(points.to_vec(), app.tokens.accent.gamma_multiply(opacity.clamp(0.0, 1.0) as f32), Stroke::NONE));
    auto(app, ui, "composer.preview", &response, &st.preset);
    if st.playing { ui.ctx().request_repaint_after(std::time::Duration::from_millis(33)); }
}

fn browse(app: &mut EffectcraftApp, ui: &mut egui::Ui, st: &mut ComposerState) {
    let r = ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("Search motion presets").desired_width(f32::INFINITY));
    auto(app, ui, "composer.search", &r, &st.search);
    preview(app, ui, st);
    let r = ui.checkbox(&mut st.playing, "Play preview");
    auto(app, ui, "composer.playing", &r, "Play preview");
    let search = st.search.to_lowercase();
    let presets: Vec<_> = composer::PRESETS.iter().filter(|p| format!("{} {}", p.name, p.category).to_lowercase().contains(&search)).collect();
    for row in presets.chunks(2) {
        ui.horizontal(|ui| {
            for preset in row {
                let r = ui.add_sized(vec2((ui.available_width() / row.len() as f32).max(70.0), 28.0),
                    egui::Button::new(preset.name).selected(st.preset == preset.id));
                auto(app, ui, &format!("composer.preset.{}", preset.id), &r, preset.name);
                if r.clicked() { st.preset = preset.id.into(); }
            }
        });
    }
    if presets.is_empty() { ui.label("No matching presets"); }
    ui.separator();
    settings(app, ui, st);
    ui.horizontal(|ui| {
        for (mode, label) in [("in", "Apply In"), ("out", "Apply Out")] {
            if button(app, ui, &format!("composer.apply.{mode}"), label) {
                st.mode = mode.into();
                if let Some(v) = panel_kit::exec(app, "composer.apply", params(st)) {
                    app.ui.status = format!("Applied {} {mode} to {} layer(s)", st.preset, v.get("layers").unwrap_or(&json!(0)));
                }
            }
        }
    });
    ui.label("Select layers, choose a motion, then apply In or Out.");
}

fn edit(app: &mut EffectcraftApp, ui: &mut egui::Ui, st: &mut ComposerState) {
    let result = app.session.execute("composer.inspect", json!({}));
    match result {
        Ok(v) => {
            let transitions = v.as_array().cloned().unwrap_or_default();
            if transitions.is_empty() { ui.label("Select a layer with a Motion Composer transition."); }
            for (i, t) in transitions.iter().enumerate() {
                ui.label(format!("{} · {} · {:.2}–{:.2} s", t.get("preset").and_then(serde_json::Value::as_str).unwrap_or(""),
                    t.get("mode").and_then(serde_json::Value::as_str).unwrap_or(""),
                    t.get("start").and_then(serde_json::Value::as_f64).unwrap_or(0.0), t.get("end").and_then(serde_json::Value::as_f64).unwrap_or(0.0)));
                if button(app, ui, &format!("composer.load.{i}"), "Load settings") {
                    st.preset = t.get("preset").and_then(serde_json::Value::as_str).unwrap_or("fade").into();
                    st.mode = t.get("mode").and_then(serde_json::Value::as_str).unwrap_or("in").into();
                    st.amount = t.get("amount").and_then(serde_json::Value::as_f64).unwrap_or(100.0);
                    st.influence = t.get("influence").and_then(serde_json::Value::as_f64).unwrap_or(60.0);
                    st.duration = t.get("end").and_then(serde_json::Value::as_f64).unwrap_or(0.6) - t.get("start").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
                }
            }
        }
        Err(e) => { ui.label(e.to_string()); }
    }
    ui.horizontal(|ui| {
        for mode in ["in", "out"] {
            let r = ui.selectable_label(st.mode == mode, mode.to_uppercase());
            auto(app, ui, &format!("composer.mode.{mode}"), &r, mode);
            if r.clicked() { st.mode = mode.into(); }
        }
    });
    settings(app, ui, st);
    if button(app, ui, "composer.replace", "Replace transition") {
        let mut p = params(st);
        p["replace"] = json!(true);
        panel_kit::exec(app, "composer.apply", p);
    }
    if button(app, ui, "composer.remove", "Remove transition") { panel_kit::exec(app, "composer.remove", json!({"mode": st.mode})); }
}

fn tools(app: &mut EffectcraftApp, ui: &mut egui::Ui, st: &mut ComposerState) {
    ui.label("Keyframe easing");
    for (label, id) in [("Easy Ease", "keys.easyEase"), ("Ease In", "keys.easyEaseIn"), ("Ease Out", "keys.easyEaseOut")] {
        if button(app, ui, &format!("composer.tool.{id}"), label) { panel_kit::exec(app, id, json!({})); }
    }
    if button(app, ui, "composer.tool.curves", "Open Ease Presets") { app.show_panel(crate::dock::PanelKind::EasePresets); }
    ui.separator();
    ui.label("Keyframe actions");
    for (label, id) in [("Copy keyframes", "keys.copy"), ("Paste keyframes", "keys.paste"), ("Reverse keyframes", "keys.timeReverse")] {
        if button(app, ui, &format!("composer.tool.{id}"), label) { panel_kit::exec(app, id, json!({})); }
    }
    number(app, ui, "composer.shift", "Shift (s)", &mut st.shift, -3600.0..=3600.0);
    if button(app, ui, "composer.tool.shift", "Shift selected keys") { panel_kit::exec(app, "keys.transform", json!({"timeOffset": st.shift})); }
    if button(app, ui, "composer.tool.anchor", "Center anchor point") { panel_kit::exec(app, "layer.centerAnchor", json!({})); }
    ui.separator();
    if button(app, ui, "composer.tool.text", "Open text controls") { app.show_panel(crate::dock::PanelKind::Character); }
    if button(app, ui, "composer.tool.library", "Open Media Browser") { app.show_panel(crate::dock::PanelKind::MediaBrowser); }
}

pub fn show(app: &mut EffectcraftApp, ui: &mut egui::Ui, rect: Rect) {
    let mut st = app.ui.composer.clone();
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(8.0)), |ui| {
        ui.set_clip_rect(rect.intersect(ui.clip_rect()));
        ui.horizontal(|ui| {
            for (tab, label) in [("browse", "Browse"), ("edit", "Edit"), ("tools", "Tools")] {
                let r = ui.selectable_label(st.tab == tab, label);
                auto(app, ui, &format!("composer.tab.{tab}"), &r, label);
                if r.clicked() { st.tab = tab.into(); }
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().id_salt("composer.content").show(ui, |ui| match st.tab.as_str() {
            "edit" => edit(app, ui, &mut st),
            "tools" => tools(app, ui, &mut st),
            _ => browse(app, ui, &mut st),
        });
    });
    app.ui.composer = st;
}
