//! Convert a project observation made through After Effects' public scripting API.
//! The observation is local migration data; it is not a parser for the AEP binary format.

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs,
    path::Path,
};

use effectcraft_color::{BlendMode, Label};
use effectcraft_keyframe::{Ease, Interp, Keyframe, ShapePath, StyleRun, TextDoc, Value};
use effectcraft_project::{
    Comp, Footage, FootageKind, ItemId, ItemKind, LayerId, LayerSource, Marker, MatteKind, Project, Solid, TrackMatte, build,
    props::{GroupKind, MaskMode, Node, PropGroup, Property},
};
use effectcraft_time::{FrameRate, Tick};
use serde_json::Value as Json;

fn num(v: &Json) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite())
}
fn field_num(v: &Json, key: &str, default: f64) -> f64 {
    v.get(key).and_then(num).unwrap_or(default)
}
fn field_u32(v: &Json, key: &str) -> u32 {
    v.get(key).and_then(Json::as_u64).and_then(|n| u32::try_from(n).ok()).unwrap_or(0)
}
fn field_str<'a>(v: &'a Json, key: &str) -> &'a str {
    v.get(key).and_then(Json::as_str).unwrap_or("")
}
fn blend_mode(v: &Json) -> BlendMode {
    v.get("blendingMode")
        .and_then(Json::as_u64)
        .and_then(|number| number.checked_sub(5212))
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| BlendMode::ALL.get(index).copied())
        .unwrap_or_default()
}
fn tick(seconds: f64) -> Tick {
    Tick::from_seconds_f64(seconds)
}
fn rate(v: &Json) -> FrameRate {
    FrameRate::from_f64(field_num(v, "frameRate", 30.0).clamp(0.001, 1000.0))
}
fn rgb(v: &Json) -> [f32; 3] {
    let a = v.as_array();
    [0, 1, 2].map(|i| a.and_then(|a| a.get(i)).and_then(num).unwrap_or(0.0).clamp(0.0, 1.0) as f32)
}
fn components(v: &Json, count: usize) -> Vec<f64> {
    let a = v.as_array();
    (0..count).map(|i| a.and_then(|a| a.get(i)).and_then(num).unwrap_or(0.0)).collect()
}
fn runs_match_text(runs: &[Json], text: &str) -> bool {
    let mut start = 0usize;
    for run in runs {
        let part = field_str(run, "text");
        let Some(end) = start.checked_add(part.len()) else { return false };
        if text.get(start..end) != Some(part) {
            return false;
        }
        start = end;
    }
    start == text.len()
}
fn text_doc(v: &Json) -> TextDoc {
    let mut doc = TextDoc { text: field_str(v, "text").to_string(), ..TextDoc::default() };
    if !field_str(v, "font").is_empty() {
        doc.font = field_str(v, "font").to_string();
    }
    doc.size = field_num(v, "fontSize", doc.size);
    if let Some(fill) = v.get("fillColor").and_then(Json::as_array) {
        for i in 0..3 {
            doc.fill[i] = fill.get(i).and_then(num).unwrap_or(1.0).clamp(0.0, 1.0) as f32;
        }
    }
    if let Some(stroke) = v.get("strokeColor").and_then(Json::as_array) {
        for i in 0..3 {
            doc.stroke[i] = stroke.get(i).and_then(num).unwrap_or(0.0).clamp(0.0, 1.0) as f32;
        }
    }
    doc.apply_fill = v.get("applyFill").and_then(Json::as_bool).unwrap_or(true);
    doc.apply_stroke = v.get("applyStroke").and_then(Json::as_bool).unwrap_or(false);
    doc.stroke_width = field_num(v, "strokeWidth", 0.0);
    doc.tracking = field_num(v, "tracking", 0.0);
    doc.leading = if v.get("autoLeading").and_then(Json::as_bool).unwrap_or(true) { None } else { Some(field_num(v, "leading", doc.size * 1.2)) };
    doc.baseline_shift = field_num(v, "baselineShift", 0.0);
    doc.faux_bold = v.get("fauxBold").and_then(Json::as_bool).unwrap_or(false);
    doc.faux_italic = v.get("fauxItalic").and_then(Json::as_bool).unwrap_or(false);
    doc.all_caps = v.get("allCaps").and_then(Json::as_bool).unwrap_or(false);
    doc.small_caps = v.get("smallCaps").and_then(Json::as_bool).unwrap_or(false);
    doc.justify = match v.get("justification").and_then(Json::as_i64) {
        Some(7414) => effectcraft_keyframe::Justify::Right,
        Some(7415) => effectcraft_keyframe::Justify::Center,
        _ => effectcraft_keyframe::Justify::Left,
    };
    if let Some(runs) = v.get("characterRuns").and_then(Json::as_array)
        && runs.len() <= 10_000
        && runs_match_text(runs, &doc.text)
    {
        let base = doc.base_style();
        let converted = runs
            .iter()
            .map(|run| {
                let source = &run["style"];
                let mut style = base.clone();
                let font = field_str(source, "font");
                if !font.is_empty() {
                    style.font = font.to_string();
                }
                if !field_str(source, "fontStyle").is_empty() {
                    style.style = field_str(source, "fontStyle").to_string();
                }
                style.size = field_num(source, "fontSize", style.size);
                if let Some(color) = source.get("fillColor").and_then(Json::as_array) {
                    for i in 0..3 {
                        style.fill[i] = color.get(i).and_then(num).unwrap_or(style.fill[i] as f64).clamp(0.0, 1.0) as f32;
                    }
                }
                if let Some(color) = source.get("strokeColor").and_then(Json::as_array) {
                    for i in 0..3 {
                        style.stroke[i] = color.get(i).and_then(num).unwrap_or(style.stroke[i] as f64).clamp(0.0, 1.0) as f32;
                    }
                }
                style.apply_fill = source.get("applyFill").and_then(Json::as_bool).unwrap_or(style.apply_fill);
                style.apply_stroke = source.get("applyStroke").and_then(Json::as_bool).unwrap_or(style.apply_stroke);
                style.stroke_width = field_num(source, "strokeWidth", style.stroke_width);
                style.tracking = field_num(source, "tracking", style.tracking);
                style.leading = if source.get("autoLeading").and_then(Json::as_bool).unwrap_or(style.leading.is_none()) {
                    None
                } else {
                    Some(field_num(source, "leading", style.size * 1.2))
                };
                style.baseline_shift = field_num(source, "baselineShift", style.baseline_shift);
                style.faux_bold = source.get("fauxBold").and_then(Json::as_bool).unwrap_or(style.faux_bold);
                style.faux_italic = source.get("fauxItalic").and_then(Json::as_bool).unwrap_or(style.faux_italic);
                style.all_caps = source.get("allCaps").and_then(Json::as_bool).unwrap_or(style.all_caps);
                style.small_caps = source.get("smallCaps").and_then(Json::as_bool).unwrap_or(style.small_caps);
                StyleRun { len: field_str(run, "text").chars().count(), style }
            })
            .collect();
        doc.set_runs(converted);
    }
    doc
}
fn value(v: &Json, template: &Value) -> Option<Value> {
    Some(match template {
        Value::Scalar(_) => Value::Scalar(num(v)?),
        Value::Vec2(_) => {
            let c = components(v, 2);
            Value::Vec2([c[0], c[1]])
        }
        Value::Vec3(_) => {
            let c = components(v, 3);
            Value::Vec3([c[0], c[1], c[2]])
        }
        Value::Color(_) => {
            let c = components(v, 4);
            Value::Color([c[0], c[1], c[2], if v.as_array().is_some_and(|a| a.len() < 4) { 1.0 } else { c[3] }])
        }
        Value::Bool(_) => Value::Bool(v.as_bool().unwrap_or_else(|| num(v).unwrap_or(0.0) != 0.0)),
        Value::Enum(_) => Value::Enum(num(v).unwrap_or(0.0).clamp(0.0, u32::MAX as f64) as u32),
        Value::Text(_) => Value::Text(Box::new(text_doc(v))),
        Value::Path(_) => {
            let a = |name: &str| -> Vec<[f64; 2]> {
                v.get(name)
                    .and_then(Json::as_array)
                    .map(|rows| {
                        rows.iter()
                            .take(100_000)
                            .map(|row| {
                                let c = components(row, 2);
                                [c[0], c[1]]
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            };
            Value::Path(ShapePath {
                vertices: a("vertices"),
                in_tangents: a("inTangents"),
                out_tangents: a("outTangents"),
                closed: v.get("closed").and_then(Json::as_bool).unwrap_or(false),
                feather: vec![],
            })
        }
        Value::Str(_) => Value::Str(v.as_str()?.to_string()),
        _ => return None,
    })
}
fn interp(v: &Json) -> Interp {
    match v.as_i64() {
        Some(6614) => Interp::Hold,
        Some(6613) => Interp::Bezier,
        _ => Interp::Linear,
    }
}
fn eases(v: &Json) -> Vec<Ease> {
    v.as_array()
        .map(|a| {
            a.iter()
                .take(4)
                .map(|x| Ease { speed: field_num(x, "speed", 0.0), influence: (field_num(x, "influence", 16.6667) / 100.0).clamp(0.0, 1.0) })
                .collect()
        })
        .unwrap_or_default()
}
fn keys(ae: &Json, template: &Value) -> Vec<Keyframe> {
    ae.get("keys")
        .and_then(Json::as_array)
        .map(|a| {
            a.iter()
                .take(100_000)
                .filter_map(|k| {
                    let time = k.get("time").and_then(num)?;
                    let val = value(k.get("value")?, template)?;
                    let mut out = Keyframe::new(tick(time), val);
                    out.in_interp = interp(&k["inInterpolation"]);
                    out.out_interp = interp(&k["outInterpolation"]);
                    out.in_ease = eases(&k["inEase"]);
                    out.out_ease = eases(&k["outEase"]);
                    out.continuous = k.get("temporalContinuous").and_then(Json::as_bool).unwrap_or(false);
                    out.auto_bezier = k.get("temporalAutoBezier").and_then(Json::as_bool).unwrap_or(false);
                    let si = components(&k["inSpatialTangent"], 3);
                    let so = components(&k["outSpatialTangent"], 3);
                    out.spatial_in = [si[0], si[1], si[2]];
                    out.spatial_out = [so[0], so[1], so[2]];
                    out.spatial_continuous = k.get("spatialContinuous").and_then(Json::as_bool).unwrap_or(false);
                    out.spatial_auto = k.get("spatialAutoBezier").and_then(Json::as_bool).unwrap_or(true);
                    Some(out)
                })
                .collect()
        })
        .unwrap_or_default()
}
fn child<'a>(group: &'a Json, match_name: &str) -> Option<&'a Json> {
    group.get("children")?.as_array()?.iter().find(|x| field_str(x, "matchName") == match_name)
}
fn put_props(dest: &mut PropGroup, source: &Json, mapping: &[(&str, &str)]) {
    for &(ae_name, ec_name) in mapping {
        if let (Some(ae_prop), Some(prop)) = (child(source, ae_name), dest.prop_mut(ec_name)) {
            if let Some(v) = value(&ae_prop["value"], &prop.value) {
                prop.value = v;
            }
            prop.keys = keys(ae_prop, &prop.value);
            let expression = field_str(ae_prop, "expression");
            if !expression.is_empty() {
                prop.expr = Some(effectcraft_project::props::Expression {
                    text: expression.to_string(),
                    enabled: ae_prop.get("expressionEnabled").and_then(Json::as_bool).unwrap_or(false),
                });
            }
        }
    }
}
fn shape_item(ids: &mut build::Ids<'_>, source: &Json) -> Option<PropGroup> {
    let name = field_str(source, "name");
    let mut group = match field_str(source, "matchName") {
        "ADBE Vector Shape - Rect" => build::shape_rect(ids, [100.0, 100.0], [0.0, 0.0], 0.0),
        "ADBE Vector Shape - Ellipse" => build::shape_ellipse(ids, [100.0, 100.0], [0.0, 0.0]),
        "ADBE Vector Graphic - Fill" => build::shape_fill(ids, [1.0, 1.0, 1.0, 1.0]),
        "ADBE Vector Graphic - Stroke" => build::shape_stroke(ids, [1.0, 1.0, 1.0, 1.0], 1.0),
        "ADBE Vector Group" => {
            let inner = child(source, "ADBE Vectors Group")?;
            let items = inner.get("children")?.as_array()?.iter().filter_map(|x| shape_item(ids, x)).collect();
            let mut g = build::shape_group(ids, name, items);
            if let Some(t) = child(source, "ADBE Vector Transform Group")
                && let Some(transform) = g.sub_mut("transform")
            {
                put_props(
                    transform,
                    t,
                    &[
                        ("ADBE Vector Anchor", "anchor"),
                        ("ADBE Vector Position", "position"),
                        ("ADBE Vector Scale", "scale"),
                        ("ADBE Vector Skew", "skew"),
                        ("ADBE Vector Skew Axis", "skewAxis"),
                        ("ADBE Vector Rotation", "rotation"),
                        ("ADBE Vector Group Opacity", "opacity"),
                    ],
                );
            }
            g
        }
        _ => return None,
    };
    group.name = name.to_string();
    group.enabled = source.get("enabled").and_then(Json::as_bool).unwrap_or(true);
    match field_str(source, "matchName") {
        "ADBE Vector Shape - Rect" => put_props(
            &mut group,
            source,
            &[
                ("ADBE Vector Shape Direction", "direction"),
                ("ADBE Vector Rect Size", "size"),
                ("ADBE Vector Rect Position", "position"),
                ("ADBE Vector Rect Roundness", "roundness"),
            ],
        ),
        "ADBE Vector Shape - Ellipse" => put_props(
            &mut group,
            source,
            &[("ADBE Vector Shape Direction", "direction"), ("ADBE Vector Ellipse Size", "size"), ("ADBE Vector Ellipse Position", "position")],
        ),
        "ADBE Vector Graphic - Fill" => put_props(
            &mut group,
            source,
            &[
                ("ADBE Vector Blend Mode", "blend"),
                ("ADBE Vector Composite Order", "composite"),
                ("ADBE Vector Fill Rule", "rule"),
                ("ADBE Vector Fill Color", "color"),
                ("ADBE Vector Fill Opacity", "opacity"),
            ],
        ),
        "ADBE Vector Graphic - Stroke" => put_props(
            &mut group,
            source,
            &[
                ("ADBE Vector Blend Mode", "blend"),
                ("ADBE Vector Composite Order", "composite"),
                ("ADBE Vector Stroke Color", "color"),
                ("ADBE Vector Stroke Opacity", "opacity"),
                ("ADBE Vector Stroke Width", "width"),
                ("ADBE Vector Stroke Line Cap", "cap"),
                ("ADBE Vector Stroke Line Join", "join"),
                ("ADBE Vector Stroke Miter Limit", "miter"),
            ],
        ),
        _ => {}
    }
    Some(group)
}
fn overlay_shapes(layer: &mut effectcraft_project::Layer, ae: &Json, project: &mut Project) {
    let Some(group) = root(ae, "ADBE Root Vectors Group") else { return };
    let Some(items) = group.get("children").and_then(Json::as_array) else { return };
    let mut ids = build::Ids(&mut project.next_id);
    let converted: Vec<_> = items.iter().filter_map(|x| shape_item(&mut ids, x)).collect();
    if let Some(contents) = layer.props.sub_mut("contents") {
        contents.children = converted.into_iter().map(Into::into).collect();
    }
}
fn overlay_masks(layer: &mut effectcraft_project::Layer, ae: &Json, project: &mut Project) {
    let Some(group) = root(ae, "ADBE Mask Parade") else { return };
    let Some(items) = group.get("children").and_then(Json::as_array) else { return };
    let mut ids = build::Ids(&mut project.next_id);
    let masks: Vec<_> = items
        .iter()
        .filter(|x| field_str(x, "matchName") == "ADBE Mask Atom")
        .map(|source| {
            let mode = MaskMode::from_name(field_str(source, "maskMode")).unwrap_or(MaskMode::Add);
            let mut mask = build::mask(&mut ids, field_str(source, "name"), ShapePath::default(), mode, [255, 255, 0]);
            mask.enabled = source.get("enabled").and_then(Json::as_bool).unwrap_or(true);
            if let GroupKind::Mask { inverted, .. } = &mut mask.kind {
                *inverted = source.get("inverted").and_then(Json::as_bool).unwrap_or(false);
            }
            put_props(
                &mut mask,
                source,
                &[("ADBE Mask Shape", "path"), ("ADBE Mask Feather", "feather"), ("ADBE Mask Opacity", "opacity"), ("ADBE Mask Offset", "expansion")],
            );
            mask
        })
        .collect();
    if let Some(dest) = layer.props.sub_mut("masks") {
        dest.children = masks.into_iter().map(Into::into).collect();
    }
}
fn generic_value(ae: &Json) -> Value {
    match ae.get("propertyValueType").and_then(Json::as_i64) {
        Some(6418) => {
            let c = components(&ae["value"], 4);
            Value::Color([c[0], c[1], c[2], c[3]])
        }
        Some(6413 | 6414) => {
            let c = components(&ae["value"], 3);
            Value::Vec3([c[0], c[1], c[2]])
        }
        Some(6415 | 6416) => {
            let c = components(&ae["value"], 2);
            Value::Vec2([c[0], c[1]])
        }
        Some(6424) => Value::Text(Box::new(text_doc(&ae["value"]))),
        Some(6423) => value(&ae["value"], &Value::Path(ShapePath::default())).unwrap_or_else(|| Value::Path(ShapePath::default())),
        _ => ae.get("value").and_then(num).map(Value::Scalar).unwrap_or_else(|| Value::Str(String::new())),
    }
}
fn generic_node(ids: &mut build::Ids<'_>, source: &Json, depth: usize) -> Option<Node> {
    if depth > 24 {
        return None;
    }
    let match_name = field_str(source, "matchName");
    let name = field_str(source, "name");
    if let Some(children) = source.get("children").and_then(Json::as_array) {
        let mut group = ids.group(match_name, name);
        group.enabled = source.get("enabled").and_then(Json::as_bool).unwrap_or(true);
        group.children = children.iter().take(10_000).filter_map(|x| generic_node(ids, x, depth + 1)).collect();
        Some(Node::Group(group))
    } else {
        let mut prop = Property::new(ids.alloc(), match_name, name, generic_value(source));
        prop.keys = keys(source, &prop.value);
        let expression = field_str(source, "expression");
        if !expression.is_empty() {
            prop.expr = Some(effectcraft_project::props::Expression {
                text: expression.to_string(),
                enabled: source.get("expressionEnabled").and_then(Json::as_bool).unwrap_or(false),
            });
        }
        Some(Node::Prop(prop))
    }
}
fn effect_id(match_name: &str) -> &str {
    match match_name {
        "ADBE Gaussian Blur 2" => "ec.blur.gaussian",
        "ADBE Solid Composite" => "ec.channel.solidcomposite",
        "ADBE Tint" => "ec.color.tint",
        "ADBE Easy Levels2" => "ec.color.levels",
        "ADBE Geometry2" => "ec.distort.transform",
        "ADBE Bulge" => "ec.distort.bulge",
        "ADBE Optics Compensation" => "ec.distort.opticscompensation",
        "ADBE Fill" => "ec.generate.fill",
        "ADBE Grid" => "ec.generate.grid",
        "ADBE AudSpect" => "ec.generate.audiospectrum",
        "ADBE Drop Shadow" => "ec.perspective.dropshadow",
        "ADBE Emboss" => "ec.stylize.emboss",
        "ADBE Tile" => "ec.stylize.motiontile",
        "CC RepeTile" => "ec.stylize.ccrepetile",
        "CC Bubbles" => "ec.sim.ccbubbles",
        _ => match_name,
    }
}
fn spec_template(default: &Json) -> Value {
    match default {
        Json::Bool(v) => Value::Bool(*v),
        Json::Number(v) if v.is_i64() || v.is_u64() => Value::Enum(v.as_u64().and_then(|n| u32::try_from(n).ok()).unwrap_or(0)),
        Json::Number(v) => Value::Scalar(v.as_f64().unwrap_or(0.0)),
        Json::Array(a) if a.len() == 2 => {
            let c = components(default, 2);
            Value::Vec2([c[0], c[1]])
        }
        Json::Array(a) if a.len() == 3 => {
            let c = components(default, 3);
            Value::Vec3([c[0], c[1], c[2]])
        }
        Json::Array(a) if a.len() == 4 => {
            let c = components(default, 4);
            Value::Color([c[0], c[1], c[2], c[3]])
        }
        Json::Null => Value::Layer(None),
        _ => Value::Str(String::new()),
    }
}
fn normal_name(name: &str) -> String {
    name.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}
// Levels reuses -0003..-0009 for whichever Channel is selected. An observer must
// switch the popup, capture all five sets, and restore it; positional matching
// would shift the controls by the non-value Histogram and lose the hidden sets.
fn levels_parameter<'a>(source: &'a Json, param_id: &str) -> Option<&'a Json> {
    let children = source.get("children")?.as_array()?;
    if param_id == "channel" {
        return children.iter().find(|p| field_str(p, "matchName") == "ADBE Easy Levels2-0001");
    }
    let (channel, suffix) = ["red", "green", "blue", "alpha"]
        .iter()
        .enumerate()
        .find_map(|(i, prefix)| param_id.strip_prefix(prefix).map(|suffix| (i as u32 + 2, suffix)))
        .unwrap_or((1, param_id));
    let index = match suffix {
        "inBlack" | "InBlack" => 3,
        "inWhite" | "InWhite" => 4,
        "gamma" | "Gamma" => 5,
        "outBlack" | "OutBlack" => 6,
        "outWhite" | "OutWhite" => 7,
        "clipToOutputBlack" | "ClipToOutputBlack" => 8,
        "clipToOutputWhite" | "ClipToOutputWhite" => 9,
        _ => return None,
    };
    let selected = levels_parameter(source, "channel").and_then(|p| p.get("value")).and_then(num).unwrap_or(1.0);
    let observed = source
        .get("levelsChannels")
        .and_then(Json::as_array)
        .and_then(|sets| sets.iter().find(|set| set.get("channel").and_then(Json::as_u64) == Some(u64::from(channel))))
        .and_then(|set| set.get("children"))
        .and_then(Json::as_array);
    let params = observed.or_else(|| (selected == f64::from(channel)).then_some(children))?;
    let match_name = format!("ADBE Easy Levels2-{index:04}");
    params.iter().find(|p| field_str(p, "matchName") == match_name)
}
fn levels_ui(param_id: &str) -> effectcraft_project::ParamUi {
    use effectcraft_project::ParamUi;
    let popup = |names: &[&str]| ParamUi::Popup { options: names.iter().map(|name| (*name).to_string()).collect() };
    if param_id == "channel" {
        popup(&["RGB", "Red", "Green", "Blue", "Alpha"])
    } else if param_id.ends_with("ClipToOutputBlack") || param_id.ends_with("ClipToOutputWhite") || param_id.starts_with("clipToOutput") {
        popup(&["Off", "On", "Off for 32 bpc Color"])
    } else if param_id == "noClip" {
        ParamUi::Hidden
    } else if param_id == "gamma" || param_id.ends_with("Gamma") {
        ParamUi::Slider { min: 0.1, max: 10.0, slider_min: 0.1, slider_max: 3.0, decimals: 2 }
    } else {
        ParamUi::Slider { min: -32768.0, max: 32768.0, slider_min: 0.0, slider_max: 1.0, decimals: 3 }
    }
}
fn effect_value(raw: &Json, template: &Value, effect_id: &str, param_id: &str) -> Option<Value> {
    let mut converted = match template {
        Value::Layer(_) => Value::Layer(raw.as_u64().filter(|v| *v > 0)),
        Value::Enum(_) => Value::Enum(num(raw)?.clamp(1.0, u32::MAX as f64) as u32 - 1),
        _ => value(raw, template)?,
    };
    // AE's Fill scripting property is 0..1; EffectCraft's Fill uses 0..100.
    if effect_id == "ec.generate.fill"
        && param_id == "opacity"
        && let Value::Scalar(ref mut opacity) = converted
    {
        *opacity *= 100.0;
    }
    Some(converted)
}
fn spec_effect(ids: &mut build::Ids<'_>, source: &Json, spec: &Json) -> Option<PropGroup> {
    let id = field_str(spec, "id");
    let params = spec.get("params")?.as_array()?;
    let ae_params = source.get("children")?.as_array()?;
    let mut group = ids.group(id, field_str(source, "name"));
    group.kind = GroupKind::Effect { effect: id.to_string() };
    group.enabled = source.get("enabled").and_then(Json::as_bool).unwrap_or(true);
    let mut used = HashSet::new();
    if id == "ec.color.levels" {
        used.extend(ae_params.iter().enumerate().filter_map(|(i, p)| field_str(p, "matchName").starts_with("ADBE Easy Levels2-").then_some(i)));
    }
    for (index, parameter) in params.iter().enumerate() {
        let param_id = field_str(parameter, "id");
        let name = field_str(parameter, "name");
        let template = spec_template(&parameter["default"]);
        let wanted = normal_name(name);
        let matched = if id == "ec.color.levels" {
            levels_parameter(source, param_id).map(|p| (None, p))
        } else {
            ae_params
                .iter()
                .enumerate()
                .find(|(j, ae)| !used.contains(j) && normal_name(field_str(ae, "name")) == wanted)
                .or_else(|| {
                    ae_params.iter().enumerate().find(|(j, ae)| !used.contains(j) && field_str(ae, "matchName").ends_with(&format!("-{:04}", index + 1)))
                })
                .map(|(i, p)| (Some(i), p))
        };
        let mut property = Property::new(ids.alloc(), param_id, name, template.clone());
        if id == "ec.color.levels" {
            property.ui = levels_ui(param_id);
        }
        if let Some((j, ae_param)) = matched {
            if let Some(j) = j {
                used.insert(j);
            }
            if let Some(v) = effect_value(&ae_param["value"], &template, id, param_id) {
                property.value = v;
            }
            property.keys = ae_param
                .get("keys")
                .and_then(Json::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|key| {
                            let time = key.get("time").and_then(num)?;
                            let v = effect_value(&key["value"], &template, id, param_id)?;
                            let mut frame = Keyframe::new(tick(time), v);
                            frame.in_interp = interp(&key["inInterpolation"]);
                            frame.out_interp = interp(&key["outInterpolation"]);
                            frame.in_ease = eases(&key["inEase"]);
                            frame.out_ease = eases(&key["outEase"]);
                            Some(frame)
                        })
                        .collect()
                })
                .unwrap_or_default();
            let expression = field_str(ae_param, "expression");
            if !expression.is_empty() {
                property.expr = Some(effectcraft_project::props::Expression {
                    text: expression.to_string(),
                    enabled: ae_param.get("expressionEnabled").and_then(Json::as_bool).unwrap_or(false),
                });
            }
        }
        group.children.push(Node::Prop(property));
    }
    for (index, ae_param) in ae_params.iter().enumerate() {
        if !used.contains(&index)
            && let Some(node) = generic_node(ids, ae_param, 1)
        {
            group.children.push(node);
        }
    }
    Some(group)
}
fn overlay_effects(layer: &mut effectcraft_project::Layer, ae: &Json, project: &mut Project, specs: &HashMap<String, Json>) {
    let Some(group) = root(ae, "ADBE Effect Parade") else { return };
    let Some(items) = group.get("children").and_then(Json::as_array) else { return };
    let mut ids = build::Ids(&mut project.next_id);
    let effects: Vec<_> = items
        .iter()
        .take(10_000)
        .filter_map(|source| {
            let id = effect_id(field_str(source, "matchName"));
            if let Some(spec) = specs.get(id) {
                return spec_effect(&mut ids, source, spec).map(Node::Group);
            }
            let mut node = generic_node(&mut ids, source, 0)?;
            if let Node::Group(ref mut g) = node {
                g.match_id = id.to_string();
                g.kind = GroupKind::Effect { effect: id.to_string() };
            }
            Some(node)
        })
        .collect();
    if let Some(dest) = layer.props.sub_mut("effects") {
        dest.children = effects;
    }
}
fn root<'a>(layer: &'a Json, match_name: &str) -> Option<&'a Json> {
    layer.get("properties")?.as_array()?.iter().find(|x| field_str(x, "matchName") == match_name)
}
fn overlay(layer: &mut effectcraft_project::Layer, ae: &Json) {
    const TRANSFORM: &[(&str, &str)] = &[
        ("ADBE Anchor Point", "anchor"),
        ("ADBE Position", "position"),
        ("ADBE Scale", "scale"),
        ("ADBE Orientation", "orientation"),
        ("ADBE Rotate X", "rotationX"),
        ("ADBE Rotate Y", "rotationY"),
        ("ADBE Rotate Z", "rotation"),
        ("ADBE Opacity", "opacity"),
    ];
    if let Some(group) = root(ae, "ADBE Transform Group") {
        for &(ae_name, ec_name) in TRANSFORM {
            if let (Some(source), Some(dest)) = (child(group, ae_name), layer.props.prop_mut(&format!("transform/{ec_name}"))) {
                if let Some(v) = value(&source["value"], &dest.value) {
                    dest.value = v;
                }
                dest.keys = keys(source, &dest.value);
                let expr = field_str(source, "expression");
                if !expr.is_empty() {
                    dest.expr = Some(effectcraft_project::props::Expression {
                        text: expr.to_string(),
                        enabled: source.get("expressionEnabled").and_then(Json::as_bool).unwrap_or(false),
                    });
                }
            }
        }
    }
    if let Some(group) = root(ae, "ADBE Text Properties")
        && let (Some(source), Some(dest)) = (child(group, "ADBE Text Document"), layer.props.prop_mut("text/sourceText"))
    {
        if let Some(v) = value(&source["value"], &dest.value) {
            dest.value = v;
        }
        dest.keys = keys(source, &dest.value);
        let expression = field_str(source, "expression");
        if !expression.is_empty() {
            dest.expr = Some(effectcraft_project::props::Expression {
                text: expression.to_string(),
                enabled: source.get("expressionEnabled").and_then(Json::as_bool).unwrap_or(false),
            });
        }
    }
    if let Some(marker) = root(ae, "ADBE Marker")
        && let Some(a) = marker.get("keys").and_then(Json::as_array)
    {
        layer.markers = a
            .iter()
            .take(100_000)
            .filter_map(|k| {
                let time = k.get("time").and_then(num)?;
                Some(Marker {
                    time: layer.layer_time(tick(time)),
                    duration: tick(field_num(&k["value"], "duration", 0.0)),
                    comment: field_str(&k["value"], "comment").to_string(),
                    chapter: field_str(&k["value"], "chapter").to_string(),
                    ..Marker::default()
                })
            })
            .collect();
    }
}
fn overlay_time_remap(layer: &mut effectcraft_project::Layer, ae: &Json, project: &mut Project) {
    if !ae.get("timeRemapEnabled").and_then(Json::as_bool).unwrap_or(false) {
        return;
    }
    let Some(source) = root(ae, "ADBE Time Remapping") else { return };
    let mut ids = build::Ids(&mut project.next_id);
    let mut property = ids.prop("timeRemap", "Time Remap", Value::Scalar(field_num(source, "value", 0.0)));
    property.keys = keys(source, &property.value);
    let expression = field_str(source, "expression");
    if !expression.is_empty() {
        property.expr = Some(effectcraft_project::props::Expression {
            text: expression.to_string(),
            enabled: source.get("expressionEnabled").and_then(Json::as_bool).unwrap_or(false),
        });
    }
    layer.props.children.push(Node::Prop(property));
}
fn source(ae: &Json, ids: &HashMap<u64, ItemId>, p: &Project) -> LayerSource {
    if root(ae, "ADBE Text Properties").is_some() {
        return LayerSource::Text;
    }
    if root(ae, "ADBE Root Vectors Group").is_some() {
        return LayerSource::Shape;
    }
    if let Some(ae_id) = ae.get("sourceId").and_then(Json::as_u64)
        && let Some(id) = ids.get(&ae_id)
        && let Some(item) = p.item(*id)
    {
        return match &item.kind {
            ItemKind::Comp(_) => LayerSource::Comp { item: *id },
            ItemKind::Solid(_) => LayerSource::Solid { item: *id },
            _ => LayerSource::Footage { item: *id },
        };
    }
    LayerSource::Null
}
fn fix_layer_refs(group: &mut PropGroup, ids: &[LayerId], depth: usize) {
    if depth > 32 {
        return;
    }
    for node in &mut group.children {
        match node {
            Node::Prop(prop) => {
                if let Value::Layer(ref mut reference) = prop.value {
                    *reference = reference.and_then(|index| usize::try_from(index.saturating_sub(1)).ok()).and_then(|index| ids.get(index)).map(|id| id.0);
                }
                for key in &mut prop.keys {
                    if let Value::Layer(ref mut reference) = key.value {
                        *reference = reference.and_then(|index| usize::try_from(index.saturating_sub(1)).ok()).and_then(|index| ids.get(index)).map(|id| id.0);
                    }
                }
            }
            Node::Group(child) => fix_layer_refs(child, ids, depth + 1),
        }
    }
}
/// AE's `keyTime` is in composition time; EffectCraft evaluates property keys in layer time.
fn fix_key_times(group: &mut PropGroup, start: Tick, stretch: f64, depth: usize) {
    if depth > 32 {
        return;
    }
    for node in &mut group.children {
        match node {
            Node::Prop(prop) => {
                for key in &mut prop.keys {
                    let relative = key.time.0.saturating_sub(start.0);
                    key.time = if (stretch - 100.0).abs() < 1e-9 { Tick(relative) } else { Tick((relative as f64 * 100.0 / stretch) as i64) };
                }
            }
            Node::Group(child) => fix_key_times(child, start, stretch, depth + 1),
        }
    }
}
fn matte_kind(ae: &Json) -> Option<MatteKind> {
    match ae.get("trackMatteType").and_then(Json::as_u64) {
        Some(5013) => Some(MatteKind::Alpha),
        Some(5014) => Some(MatteKind::AlphaInverted),
        Some(5015) => Some(MatteKind::Luma),
        Some(5016) => Some(MatteKind::LumaInverted),
        _ => None,
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let input = args.next().ok_or("usage: ae_snapshot INPUT.json OUTPUT.ecproj")?;
    let output = args.next().ok_or("usage: ae_snapshot INPUT.json OUTPUT.ecproj")?;
    let specs: HashMap<String, Json> = if let Some(path) = args.next() {
        if fs::metadata(&path)?.len() > 8 * 1024 * 1024 {
            return Err("effect specification is too large".into());
        }
        let list: Vec<Json> = serde_json::from_str(&fs::read_to_string(path)?)?;
        list.into_iter().filter_map(|spec| Some((spec.get("id")?.as_str()?.to_string(), spec))).collect()
    } else {
        HashMap::new()
    };
    if fs::metadata(&input)?.len() > 256 * 1024 * 1024 {
        return Err("observation is too large".into());
    }
    let text = fs::read_to_string(input)?;
    let doc: Json = serde_json::from_str(&text)?;
    let items = doc.get("items").and_then(Json::as_array).ok_or("missing items")?;
    if items.len() > 100_000 {
        return Err("too many items".into());
    }
    let mut project = Project::default();
    let mut ids = HashMap::new();
    for ae in items {
        let name = field_str(ae, "name");
        let width = field_u32(ae, "width");
        let height = field_u32(ae, "height");
        let item_type = field_str(ae, "type");
        if width > 32_768 || height > 32_768 || (item_type == "Comp" && (width == 0 || height == 0)) {
            return Err(format!("invalid dimensions for item {name}").into());
        }
        if item_type == "Comp" && !(0.0..=86_400.0).contains(&field_num(ae, "duration", -1.0)) {
            return Err(format!("invalid duration for composition {name}").into());
        }
        let kind = match field_str(ae, "type") {
            "Comp" => ItemKind::Comp(std::sync::Arc::new(Comp::new(width, height, rate(ae), tick(field_num(ae, "duration", 0.0))))),
            "Footage" if ae.get("solidColor").is_some_and(Json::is_array) => {
                ItemKind::Solid(Solid { color: rgb(&ae["solidColor"]), width, height, pixel_aspect: field_num(ae, "pixelAspect", 1.0) })
            }
            "Footage" => {
                let path = field_str(ae, "file").to_string();
                let ext = Path::new(&path).extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
                let audio = matches!(ext.as_str(), "mp3" | "wav" | "flac" | "aif" | "aiff" | "ogg");
                let still = matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tif" | "tiff" | "gif");
                let f = Footage {
                    path: path.clone(),
                    kind: if audio {
                        FootageKind::Audio
                    } else if still {
                        FootageKind::Still
                    } else {
                        FootageKind::Video
                    },
                    width: field_u32(ae, "width"),
                    height: field_u32(ae, "height"),
                    pixel_aspect: field_num(ae, "pixelAspect", 1.0),
                    frame_rate: rate(ae),
                    duration: tick(field_num(ae, "duration", 0.0)),
                    has_video: !audio,
                    has_audio: audio || !still,
                    missing: !Path::new(&path).exists(),
                    ..Footage::default()
                };
                ItemKind::Footage(f)
            }
            _ => ItemKind::Folder,
        };
        let id = project.add_item(name, Label::Red, None, kind);
        if let Some(ae_id) = ae.get("id").and_then(Json::as_u64) {
            ids.insert(ae_id, id);
        }
    }
    for (ae, id) in items.iter().filter_map(|ae| Some((ae, *ids.get(&ae.get("id")?.as_u64()?)?))) {
        if let Some(item) = project.item_mut(id) {
            item.parent = ae.get("parentId").and_then(Json::as_u64).and_then(|parent| ids.get(&parent).copied());
            item.comment = field_str(ae, "comment").to_string();
        }
        if field_str(ae, "type") != "Comp" {
            continue;
        }
        let mut comp = Comp::new(field_u32(ae, "width"), field_u32(ae, "height"), rate(ae), tick(field_num(ae, "duration", 0.0)));
        comp.pixel_aspect = field_num(ae, "pixelAspect", 1.0);
        comp.display_start = tick(field_num(ae, "displayStartTime", 0.0));
        comp.background = rgb(&ae["bgColor"]);
        comp.work_area = (tick(field_num(ae, "workAreaStart", 0.0)), tick(field_num(ae, "workAreaDuration", field_num(ae, "duration", 0.0))));
        let mut layer_ids = HashMap::new();
        if let Some(layers) = ae.get("layers").and_then(Json::as_array) {
            if layers.len() > 100_000 {
                return Err("too many layers".into());
            }
            for layer in layers {
                let src = source(layer, &ids, &project);
                let size = src.item().and_then(|item| project.item(item)).and_then(|item| item.dimensions()).unwrap_or((comp.width, comp.height));
                let mut built = build::layer(&mut project, &comp, field_str(layer, "name"), src, size, None);
                built.name = field_str(layer, "name").to_string();
                built.start_time = tick(field_num(layer, "startTime", 0.0));
                built.in_point = tick(field_num(layer, "inPoint", 0.0));
                built.out_point = tick(field_num(layer, "outPoint", field_num(ae, "duration", 0.0)));
                let stretch = field_num(layer, "stretch", 100.0);
                built.stretch = if stretch.abs() < 1e-9 { 100.0 } else { stretch };
                built.switches.video = layer.get("enabled").and_then(Json::as_bool).unwrap_or(true);
                built.switches.audio = layer.get("audioEnabled").and_then(Json::as_bool).unwrap_or(false);
                built.switches.three_d = layer.get("threeDLayer").and_then(Json::as_bool).unwrap_or(false);
                built.switches.adjustment = layer.get("adjustmentLayer").and_then(Json::as_bool).unwrap_or(false);
                built.switches.shy = layer.get("shy").and_then(Json::as_bool).unwrap_or(false);
                built.switches.solo = layer.get("solo").and_then(Json::as_bool).unwrap_or(false);
                built.switches.locked = layer.get("locked").and_then(Json::as_bool).unwrap_or(false);
                built.switches.motion_blur = layer.get("motionBlur").and_then(Json::as_bool).unwrap_or(false);
                built.switches.collapse = layer.get("collapseTransformation").and_then(Json::as_bool).unwrap_or(false);
                built.blend_mode = blend_mode(layer);
                overlay(&mut built, layer);
                overlay_time_remap(&mut built, layer, &mut project);
                overlay_shapes(&mut built, layer, &mut project);
                overlay_masks(&mut built, layer, &mut project);
                overlay_effects(&mut built, layer, &mut project, &specs);
                fix_key_times(&mut built.props, built.start_time, built.stretch, 0);
                if let Some(ae_id) = layer.get("id").and_then(Json::as_u64) {
                    layer_ids.insert(ae_id, built.id);
                }
                comp.layers.push(built);
            }
            for (built, ae_layer) in comp.layers.iter_mut().zip(layers) {
                built.parent = ae_layer.get("parentId").and_then(Json::as_u64).and_then(|parent| layer_ids.get(&parent).copied());
            }
            let ec_ids: Vec<LayerId> = comp.layers.iter().map(|layer| layer.id).collect();
            for (index, (built, ae_layer)) in comp.layers.iter_mut().zip(layers).enumerate() {
                let Some(kind) = matte_kind(ae_layer) else { continue };
                let matte = ae_layer.get("trackMatteLayerId").and_then(Json::as_u64).and_then(|id| layer_ids.get(&id).copied()).or_else(|| {
                    // Older snapshots did not record the matte source. Their disabled
                    // layer immediately above an enabled consumer is the AE matte pair.
                    let legacy = ae_layer.get("hasTrackMatte").is_none()
                        && ae_layer.get("enabled").and_then(Json::as_bool).unwrap_or(true)
                        && index > 0
                        && layers.get(index - 1).and_then(|above| above.get("enabled")).and_then(Json::as_bool) == Some(false);
                    legacy.then(|| ec_ids.get(index - 1).copied()).flatten()
                });
                if let Some(layer) = matte.filter(|id| *id != built.id) {
                    built.track_matte = Some(TrackMatte { layer, kind });
                }
            }
            for layer in &mut comp.layers {
                fix_layer_refs(&mut layer.props, &ec_ids, 0);
            }
        }
        if let Some(dest) = project.comp_mut(id) {
            *dest = comp;
        }
    }
    let output_text = project.to_file_json()?;
    fs::write(&output, output_text)?;
    eprintln!("converted {} items to {}", project.items.len(), Path::new(&output).display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ae_levels_imports_hidden_channels_without_histogram_offset() {
        let source = serde_json::json!({"name": "Levels", "children": [
            {"matchName":"ADBE Easy Levels2-0001", "name":"채널", "value":1},
            {"matchName":"ADBE Easy Levels2-0002", "name":"Histogram", "value":null},
            {"matchName":"ADBE Easy Levels2-0003", "name":"Input Black", "value":0}
        ], "levelsChannels": [
            {"channel":1,"children":[{"matchName":"ADBE Easy Levels2-0003","value":-1.01},
                {"matchName":"ADBE Easy Levels2-0004","value":8.19},
                {"matchName":"ADBE Easy Levels2-0005","value":5.0}]},
            {"channel":5,"children":[{"matchName":"ADBE Easy Levels2-0004","value":0.88},
                {"matchName":"ADBE Easy Levels2-0007","value":19.607843,"keys":[
                    {"time":2.0,"value":10.0,"inInterpolation":6612,"outInterpolation":6612}]},
                {"matchName":"ADBE Easy Levels2-0009","value":3}]}
        ]});
        let spec = serde_json::json!({"id":"ec.color.levels","params":[
            {"id":"channel","name":"Channel","default":0},
            {"id":"inBlack","name":"Input Black","default":0.0},
            {"id":"inWhite","name":"Input White","default":1.0},
            {"id":"gamma","name":"Gamma","default":1.0},
            {"id":"redInWhite","name":"Red Input White","default":1.0},
            {"id":"alphaInWhite","name":"Alpha Input White","default":1.0},
            {"id":"alphaOutWhite","name":"Alpha Output White","default":1.0},
            {"id":"alphaClipToOutputWhite","name":"Alpha Clip To Output White","default":2}
        ]});
        let mut next_id = 1;
        let group = spec_effect(&mut build::Ids(&mut next_id), &source, &spec).unwrap();
        assert_eq!(effect_id("ADBE Easy Levels2"), "ec.color.levels");
        assert_eq!(group.prop("channel").unwrap().value, Value::Enum(0));
        assert!(matches!(&group.prop("channel").unwrap().ui, effectcraft_project::ParamUi::Popup { options } if options.len() == 5));
        assert_eq!(group.prop("inBlack").unwrap().value, Value::Scalar(-1.01));
        assert_eq!(group.prop("inWhite").unwrap().value, Value::Scalar(8.19));
        assert_eq!(group.prop("gamma").unwrap().value, Value::Scalar(5.0));
        assert_eq!(group.prop("redInWhite").unwrap().value, Value::Scalar(1.0));
        assert_eq!(group.prop("alphaInWhite").unwrap().value, Value::Scalar(0.88));
        let alpha = group.prop("alphaOutWhite").unwrap();
        assert_eq!(alpha.value, Value::Scalar(19.607843));
        assert_eq!(alpha.keys[0].value, Value::Scalar(10.0));
        assert_eq!(alpha.keys[0].time, tick(2.0));
        assert_eq!(group.prop("alphaClipToOutputWhite").unwrap().value, Value::Enum(2));
        assert_eq!(group.children.len(), 8);
    }

    #[test]
    fn ae_levels_old_observation_applies_only_to_selected_channel() {
        let source = serde_json::json!({"children":[
            {"matchName":"ADBE Easy Levels2-0001","value":5},
            {"matchName":"ADBE Easy Levels2-0004","value":0.88}
        ]});
        assert_eq!(levels_parameter(&source, "alphaInWhite").unwrap()["value"], 0.88);
        assert!(levels_parameter(&source, "inWhite").is_none());
        assert!(levels_parameter(&source, "redInWhite").is_none());
        assert!(levels_parameter(&source, "noClip").is_none());
    }

    #[test]
    fn ae_fill_opacity_uses_fraction() {
        assert_eq!(effect_value(&Json::from(0.5), &Value::Scalar(100.0), "ec.generate.fill", "opacity"), Some(Value::Scalar(50.0)));
        assert_eq!(effect_value(&Json::from(50.0), &Value::Scalar(100.0), "ec.perspective.dropshadow", "opacity"), Some(Value::Scalar(50.0)));
    }

    #[test]
    fn ae_effect_point_values_stay_in_layer_pixels() {
        assert_eq!(effect_value(&serde_json::json!([960, 480]), &Value::Vec2([0.5, 0.5]), "ec.distort.transform", "anchor"), Some(Value::Vec2([960.0, 480.0])));
    }

    #[test]
    fn ae_blending_mode_uses_scripting_enum() {
        assert_eq!(blend_mode(&serde_json::json!({ "blendingMode": 5233 })), BlendMode::PinLight);
        assert_eq!(blend_mode(&serde_json::json!({ "blendingMode": 5228 })), BlendMode::Overlay);
        assert_eq!(blend_mode(&serde_json::json!({ "blendingMode": 99999 })), BlendMode::Normal);
    }

    #[test]
    fn ae_text_character_runs_keep_line_fonts() {
        let source = serde_json::json!({
            "text": "일본어\rroman\r한국어", "font": "NotoSansCJKkr-Light", "fontSize": 46,
            "characterRuns": [
                {"text":"일본어\r", "style":{"font":"NotoSansCJKkr-Light", "fontSize":46}},
                {"text":"roman\r한국어", "style":{"font":"A2Z-Bold", "fontSize":40}}
            ]
        });
        let doc = text_doc(&source);
        assert_eq!(doc.style_at(0).font, "NotoSansCJKkr-Light");
        assert_eq!(doc.style_at(4).font, "A2Z-Bold");
        assert_eq!(doc.style_at(10).font, "A2Z-Bold");
        assert_eq!(doc.style_at(4).size, 40.0);
    }

    #[test]
    fn ae_mask_header_preserves_mode_inversion_and_enabled() {
        let mut project = Project::default();
        let comp = Comp::new(100, 100, FrameRate::from_f64(30.0), tick(1.0));
        let mut layer = build::layer(&mut project, &comp, "Test", LayerSource::Shape, (100, 100), None);
        let ae = serde_json::json!({"properties": [{"matchName": "ADBE Mask Parade", "children": [
            {"matchName": "ADBE Mask Atom", "name": "Mask 1", "enabled": false,
             "maskMode": "SUBTRACT", "inverted": true, "children": []}
        ]}]});
        overlay_masks(&mut layer, &ae, &mut project);
        let masks = layer.props.sub("masks").expect("layer has masks");
        let mask = masks.groups().next().expect("mask was imported");
        assert_eq!(
            mask.kind,
            GroupKind::Mask {
                mode: MaskMode::Subtract,
                inverted: true,
                color: [255, 255, 0],
                locked: false,
                motion_blur: Default::default(),
                feather_falloff: Default::default(),
                roto_bezier: false,
            }
        );
        assert!(!mask.enabled);
    }

    #[test]
    fn shifted_layer_keys_use_layer_time() {
        let mut ids = 1_u64;
        let mut group = PropGroup::new(ids, "root", "Root");
        ids += 1;
        let mut opacity = Property::new(ids, "opacity", "Opacity", Value::Scalar(0.0));
        opacity.keys = vec![Keyframe::new(tick(26.4333333333333), Value::Scalar(0.0)), Keyframe::new(tick(27.1), Value::Scalar(100.0))];
        group.children.push(Node::Prop(opacity));
        fix_key_times(&mut group, tick(26.1), 100.0, 0);
        let keys = &group.prop("opacity").expect("opacity exists").keys;
        assert!((keys[0].time.seconds() - 1.0 / 3.0).abs() < 1e-6);
        assert!((keys[1].time.seconds() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ae_track_matte_kind_maps_all_supported_modes() {
        assert_eq!(matte_kind(&serde_json::json!({"trackMatteType": 5013})), Some(MatteKind::Alpha));
        assert_eq!(matte_kind(&serde_json::json!({"trackMatteType": 5014})), Some(MatteKind::AlphaInverted));
        assert_eq!(matte_kind(&serde_json::json!({"trackMatteType": 5015})), Some(MatteKind::Luma));
        assert_eq!(matte_kind(&serde_json::json!({"trackMatteType": 5016})), Some(MatteKind::LumaInverted));
        assert_eq!(matte_kind(&serde_json::json!({"trackMatteType": 5012})), None);
    }
}
