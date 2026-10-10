//! Motion Composer: original, editable In/Out motion presets. No third-party preset data.
//! Ownership lives in a versioned layer marker so save/load and undo preserve transitions.
//! Existing keys are never overwritten; externally edited owned keys must be reconciled first.

use effectcraft_keyframe::{EaseCurve, Keyframe, Value as KV, evaluate, set_key};
use effectcraft_project::{CuePoint, Layer, Marker, Property};
use effectcraft_time::Tick;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::commands::{CommandSpec, bad, has_comp, layers_p, str_p};
use crate::{EngineError, Result, Session, cmd, query};

const TAG: &str = "effectcraft.motionComposer.v1";
const MAX_LAYERS: usize = 4096;
const MAX_STATE: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
}

/// Original motion definitions, licensed with the project (see the attribution sidecar).
pub const PRESETS: &[Preset] = &[
    Preset { id: "fade", name: "Fade", category: "Opacity" },
    Preset { id: "slide-left", name: "Slide Left", category: "Position" },
    Preset { id: "slide-right", name: "Slide Right", category: "Position" },
    Preset { id: "slide-up", name: "Slide Up", category: "Position" },
    Preset { id: "slide-down", name: "Slide Down", category: "Position" },
    Preset { id: "zoom", name: "Zoom", category: "Scale" },
    Preset { id: "pop", name: "Pop", category: "Scale" },
    Preset { id: "spin", name: "Spin", category: "Rotation" },
    Preset { id: "bounce-left", name: "Bounce Left", category: "Position" },
    Preset { id: "bounce-right", name: "Bounce Right", category: "Position" },
    Preset { id: "bounce-up", name: "Bounce Up", category: "Position" },
    Preset { id: "bounce-down", name: "Bounce Down", category: "Position" },
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Track {
    path: String,
    keys: Vec<Keyframe>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Transition {
    preset: String,
    mode: String,
    amount: f64,
    influence: f64,
    tracks: Vec<Track>,
}

fn is_owned(m: &Marker) -> bool {
    m.cue_point.as_ref().is_some_and(|c| c.name == TAG)
}

fn decode(m: &Marker) -> Result<Transition> {
    let text = m.cue_point.as_ref().and_then(|c| c.params.iter().find(|(k, _)| k == "state")).map(|(_, v)| v)
        .ok_or_else(|| bad("composer", "transition marker has no state"))?;
    if text.len() > MAX_STATE {
        return Err(bad("composer", "transition marker exceeds 64 KiB"));
    }
    let t: Transition = serde_json::from_str(text).map_err(|e| bad("composer", format!("damaged transition marker: {e}")))?;
    if !PRESETS.iter().any(|p| p.id == t.preset) || !["in", "out"].contains(&t.mode.as_str())
        || !t.amount.is_finite() || !t.influence.is_finite() || t.tracks.is_empty() || t.tracks.len() > 3
        || t.tracks.iter().any(|tr| !["transform/position", "transform/scale", "transform/rotation", "transform/opacity"].contains(&tr.path.as_str())
            || tr.keys.len() < 2 || tr.keys.len() > 8)
    {
        return Err(bad("composer", "invalid transition state; leave the marker intact and restore a valid project"));
    }
    Ok(t)
}

fn encode(start: Tick, end: Tick, t: &Transition) -> Result<Marker> {
    let text = serde_json::to_string(t).map_err(|e| bad("composer", format!("cannot encode transition: {e}")))?;
    if text.len() > MAX_STATE {
        return Err(bad("composer", "transition state exceeds 64 KiB"));
    }
    Ok(Marker {
        time: start, duration: end - start, comment: format!("Motion {} · {}", t.mode, t.preset),
        cue_point: Some(CuePoint { name: TAG.into(), params: vec![("state".into(), text)], ..Default::default() }),
        ..Default::default()
    })
}

fn number(p: &Value, key: &str, default: f64, lo: f64, hi: f64) -> Result<f64> {
    let v = match p.get(key) {
        Some(v) => v.as_f64().ok_or_else(|| bad("composer", format!("`{key}` must be a number")))?,
        None => default,
    };
    if !v.is_finite() || !(lo..=hi).contains(&v) {
        return Err(bad("composer", format!("`{key}` must be finite and between {lo} and {hi}")));
    }
    Ok(v)
}

fn mode(p: &Value) -> Result<&str> {
    match str_p(p, "mode").unwrap_or("in") {
        m @ ("in" | "out") => Ok(m),
        _ => Err(bad("composer", "`mode` must be in or out")),
    }
}

fn check_layer(l: &Layer) -> Result<()> {
    if l.switches.locked || l.markers_locked {
        return Err(bad("composer", format!("{}: unlock the layer and its markers first", l.name)));
    }
    if !l.stretch.is_finite() || l.stretch <= 0.0 {
        return Err(bad("composer", format!("{}: positive finite time stretch is required", l.name)));
    }
    Ok(())
}

/// Original normalised samples (time, displacement or scale factor).
fn samples(id: &str) -> Vec<(f64, f64)> {
    match id {
        "pop" => vec![(0.0, 0.0), (0.58, 1.16), (0.82, 0.96), (1.0, 1.0)],
        id if id.starts_with("bounce") => vec![(0.0, 1.0), (0.57, -0.12), (0.8, 0.04), (1.0, 0.0)],
        "zoom" => vec![(0.0, 0.0), (1.0, 1.0)],
        _ => vec![(0.0, 1.0), (1.0, 0.0)],
    }
}

fn affected(id: &str) -> Vec<&'static str> {
    match id {
        "fade" => vec!["transform/opacity"],
        "zoom" | "pop" => vec!["transform/scale", "transform/opacity"],
        "spin" => vec!["transform/rotation", "transform/opacity"],
        _ => vec!["transform/position", "transform/opacity"],
    }
}

fn value(id: &str, path: &str, base: &KV, f: f64, amount: f64) -> KV {
    let mut c = base.components();
    match path {
        "transform/opacity" => {
            if let Some(v) = c.first_mut() { *v *= 1.0 - f; }
        }
        "transform/scale" => {
            // Preserve a 3D layer's depth scale; animate X/Y only.
            for v in c.iter_mut().take(2) { *v *= f; }
        }
        "transform/rotation" => {
            if let Some(v) = c.first_mut() { *v += amount * f; }
        }
        _ => {
            let (dim, sign) = if id.ends_with("left") { (0, -1.0) } else if id.ends_with("right") { (0, 1.0) }
                else if id.ends_with("up") { (1, -1.0) } else { (1, 1.0) };
            if let Some(v) = c.get_mut(dim) { *v += sign * amount * f; }
        }
    }
    base.with_components(&c)
}

fn track(id: &str, path: &str, pr: &Property, base: &KV, start: Tick, end: Tick, mode: &str, amount: f64, influence: f64) -> Track {
    let points = if path == "transform/opacity" { vec![(0.0, 1.0), (1.0, 0.0)] } else { samples(id) };
    let mut keys: Vec<_> = points.into_iter().map(|(u, f)| {
        let u = if mode == "out" { 1.0 - u } else { u };
        let time = start + Tick::from_seconds_f64((end - start).seconds() * u);
        let mut k = Keyframe::new(time, value(id, path, base, f, amount));
        k.spatial_auto = false;
        k
    }).collect();
    keys.sort_by_key(|k| k.time);
    let curve = EaseCurve { out_influence: influence, out_speed: 0.0, in_influence: influence, in_speed: 0.0 };
    for i in 0..keys.len().saturating_sub(1) { curve.apply(&mut keys, i, pr.spatial); }
    Track { path: path.into(), keys }
}

fn remove_owned(l: &mut Layer, mode: &str) -> Result<usize> {
    check_layer(l)?;
    let mut removed = 0;
    for m in &l.markers {
        if !is_owned(m) { continue; }
        let t = decode(m)?;
        if t.mode != mode { continue; }
        for tr in &t.tracks {
            let pr = l.props.prop_mut(&tr.path).ok_or_else(|| bad("composer", "transition property is missing"))?;
            if tr.keys.iter().any(|key| !pr.keys.contains(key)) {
                return Err(bad("composer", format!("{}: {} transition keys were edited; undo those edits before replacing/removing it", l.name, mode)));
            }
            pr.keys.retain(|key| !tr.keys.contains(key));
        }
        removed += 1;
    }
    // Validate every marker before removing any; Session::edit is atomic on failure.
    let mut keep = Vec::with_capacity(l.markers.len());
    for m in &l.markers {
        if !is_owned(m) || decode(m)?.mode != mode { keep.push(m.clone()); }
    }
    l.markers = keep;
    Ok(removed)
}

fn apply(s: &mut Session, p: &Value) -> Result<Value> {
    let id = str_p(p, "preset").unwrap_or("fade");
    if !PRESETS.iter().any(|pr| pr.id == id) { return Err(bad("composer.apply", "unknown preset; see composer.list")); }
    let mode = mode(p)?;
    let duration = number(p, "duration", 0.6, 0.001, 3600.0)?;
    let amount = number(p, "amount", 100.0, -100000.0, 100000.0)?;
    let influence = number(p, "influence", 60.0, 0.1, 100.0)?;
    let stagger = number(p, "stagger", 0.0, -3600.0, 3600.0)?;
    let at = match str_p(p, "at").unwrap_or("edge") { a @ ("edge" | "current") => a, _ => return Err(bad("composer", "`at` must be edge or current")) };
    let (cid, mut ids) = layers_p(s, p)?;
    ids.sort_unstable();
    ids.dedup();
    // Use the comp's stacking order, rather than numeric ids or click order.
    let comp = s.project.comp(cid).ok_or(EngineError::NoComp)?;
    ids.sort_by_key(|id| comp.layers.iter().position(|l| l.id == *id).unwrap_or(usize::MAX));
    if str_p(p, "order").unwrap_or("ascending") == "descending" { ids.reverse(); }
    else if !["ascending", "descending"].contains(&str_p(p, "order").unwrap_or("ascending")) { return Err(bad("composer", "`order` must be ascending or descending")); }
    if ids.is_empty() || ids.len() > MAX_LAYERS { return Err(bad("composer", "select 1–4096 layers")); }
    let t = s.time();
    let replace = p.get("replace").and_then(Value::as_bool).unwrap_or(false);
    let mut work = vec![];
    // Prepare on clones first; an error never mutates a project or an existing transition.
    for (i, lid) in ids.iter().enumerate() {
        let mut l = comp.layer(*lid).cloned().ok_or_else(|| bad("composer", "layer is missing"))?;
        check_layer(&l)?;
        if replace { remove_owned(&mut l, mode)?; }
        let offset = Tick::from_seconds_f64(stagger * i as f64);
        let anchor = if at == "current" { t } else if mode == "in" { l.in_point } else { l.out_point };
        let (a, b) = if mode == "in" { (anchor + offset, anchor + offset + Tick::from_seconds_f64(duration)) }
            else { (anchor + offset - Tick::from_seconds_f64(duration), anchor + offset) };
        if a < l.in_point || b > l.out_point || b <= a { return Err(bad("composer", format!("{}: transition must fit inside the layer's visible span", l.name))); }
        let (start, end) = (l.layer_time(a), l.layer_time(b));
        if end <= start { return Err(bad("composer", "duration is too short at this time stretch")); }
        let mut tracks = vec![];
        for path in affected(id) {
            if path == "transform/position" && l.props.prop("transform/positionX").is_some() { return Err(bad("composer", "join separate Position dimensions before applying a position preset")); }
            let pr = l.props.prop(path).ok_or_else(|| bad("composer", format!("{}: no {path}", l.name)))?;
            if pr.expr.is_some() || pr.static_only { return Err(bad("composer", format!("{}: {path} is expression-driven or static", l.name))); }
            if pr.keys.iter().any(|k| k.time >= start && k.time <= end) { return Err(bad("composer", format!("{}: existing keys overlap {mode} {path}; adjust duration or use replace for a Composer transition", l.name))); }
            let base = pr.value_at(if mode == "in" { end } else { start });
            if base.components().is_empty() || base.components().iter().any(|v| !v.is_finite()) { return Err(bad("composer", "transition requires finite numeric property values")); }
            tracks.push(track(id, path, pr, &base, start, end, mode, amount, influence));
        }
        let transition = Transition { preset: id.into(), mode: mode.into(), amount, influence, tracks };
        for tr in &transition.tracks {
            let pr = l.props.prop_mut(&tr.path).ok_or_else(|| bad("composer", "property is missing"))?;
            for k in &tr.keys { set_key(&mut pr.keys, k.clone()); }
        }
        l.markers.push(encode(start, end, &transition)?);
        l.markers.sort_by_key(|m| m.time);
        work.push((*lid, l));
    }
    s.edit("Apply Motion Preset", None, |proj, _| {
        let c = proj.comp_mut(cid).ok_or(EngineError::NoComp)?;
        for (id, l) in work { *c.layer_mut(id).ok_or_else(|| bad("composer", "layer vanished"))? = l; }
        Ok(())
    })?;
    Ok(json!({"layers": ids.len(), "preset": id, "mode": mode}))
}

fn remove(s: &mut Session, p: &Value) -> Result<Value> {
    let mode = mode(p)?;
    let (cid, ids) = layers_p(s, p)?;
    if ids.is_empty() { return Err(bad("composer.remove", "select a layer")); }
    s.edit("Remove Motion Preset", None, |proj, _| {
        let c = proj.comp_mut(cid).ok_or(EngineError::NoComp)?;
        let mut removed = 0;
        for id in ids { removed += remove_owned(c.layer_mut(id).ok_or_else(|| bad("composer", "no layer"))?, mode)?; }
        Ok(json!({"removed": removed}))
    })
}

fn inspect(s: &mut Session, p: &Value) -> Result<Value> {
    let (cid, ids) = layers_p(s, p)?;
    let c = s.project.comp(cid).ok_or(EngineError::NoComp)?;
    let mut out = vec![];
    for id in ids {
        let l = c.layer(id).ok_or_else(|| bad("composer", "no layer"))?;
        for m in &l.markers {
            if !is_owned(m) { continue; }
            let t = decode(m)?;
            out.push(json!({"layer": id, "preset": t.preset, "mode": t.mode, "amount": t.amount, "influence": t.influence,
                "start": l.comp_time(m.time).seconds(), "end": l.comp_time(m.time + m.duration).seconds()}));
        }
    }
    Ok(json!(out))
}

fn list(_: &mut Session, p: &Value) -> Result<Value> {
    let search = str_p(p, "search").unwrap_or("").to_lowercase();
    Ok(json!(PRESETS.iter().filter(|p| format!("{} {}", p.name, p.category).to_lowercase().contains(&search)).collect::<Vec<_>>()))
}

/// Sample exactly the same original motion and easing as the applied keys. Preview is read-only.
pub fn preview(id: &str, mode: &str, u: f64) -> [f64; 5] {
    let u = if u.is_finite() { u.clamp(0.0, 1.0) } else { 0.0 };
    let base = |path| if path == "transform/scale" { KV::Vec2([1.0, 1.0]) } else if path == "transform/opacity" { KV::Scalar(1.0) } else if path == "transform/rotation" { KV::Scalar(0.0) } else { KV::Vec2([0.0, 0.0]) };
    let mut v = [0.0, 0.0, 1.0, 1.0, 0.0];
    for path in affected(id) {
        let pr = Property::new(0, "", "", base(path));
        let tr = track(id, path, &pr, &pr.value, Tick::ZERO, Tick::from_seconds_f64(1.0), mode, if id == "spin" {90.0} else {1.0}, 60.0);
        let x = evaluate(&tr.keys, Tick::from_seconds_f64(u), false);
        if let Some(x) = x {
            match path { "transform/position" => { let [a, b] = x.as_vec2(); v[0] = a; v[1] = b; }, "transform/scale" => v[2] = x.as_vec2()[0], "transform/opacity" => v[3] = x.as_f64(), "transform/rotation" => v[4] = x.as_f64(), _ => {} }
        }
    }
    v
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        query!("composer.list", "Motion Presets", "{search?} — original built-in motion presets", list),
        query!("composer.inspect", "Inspect Motion Transitions", "{layers?} — transitions on selected layers, with comp-time spans", inspect),
        cmd!("composer.apply", "Apply Motion Preset", [], None,
            "{preset, mode?: in|out, layers?, duration?: seconds, amount?, influence?: percent, at?: edge|current, stagger?: seconds, order?: ascending|descending, replace?: bool} — non-destructive, one undo step", has_comp, apply),
        cmd!("composer.remove", "Remove Motion Preset", [], None, "{layers?, mode?: in|out} — removes only intact Composer-owned keys", has_comp, remove),
    ]
}
