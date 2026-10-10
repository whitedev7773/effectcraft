//! Render queue cards expose the output file, format, range, quality and resolution directly.
//! Detailed render / encoding settings and templates remain available in labelled menus.
//! All edits dispatch engine commands and register stable automation ids by queue item id.

use effectcraft_engine::project::render_queue::{
    AlphaMode, AudioFormat, AudioOutput, Channels, ColorDepth, CurrentOrOff, EffectsMode, FieldRender, OpusApplication, OutputFormat, PostRenderAction,
    ProResProfile, Pulldown, RESIZE_PRESETS, RenderLog, RenderQuality, RenderQueueItem, RenderStatus, ResizeQuality, SwitchOverride, TimeSpan, WebmVideoCodec,
};
use effectcraft_engine::project::render_templates::RenderTemplates;
use egui::{Align2, Color32, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::{Value, json};

use crate::theme::Tokens;
use crate::widgets::check_label as mark;
use crate::{EffectcraftApp, widgets};

/// `h:mm:ss` for a duration in seconds.
pub fn hms(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// Unix seconds → `YYYY-MM-DD hh:mm:ss` (UTC).
pub fn utc_stamp(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let rem = unix % 86_400;
    // Civil-from-days (proleptic Gregorian).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", rem / 3600, rem / 60 % 60, rem % 60)
}

fn status_color(t: &Tokens, s: &RenderStatus) -> Color32 {
    match s {
        RenderStatus::Done => Color32::from_rgb(0x6c, 0xc0, 0x6c),
        RenderStatus::Rendering => t.accent_hover,
        RenderStatus::Failed(_) | RenderStatus::NeedsOutput => t.danger,
        RenderStatus::UserStopped => t.warning,
        RenderStatus::Unqueued => t.text_faint,
        RenderStatus::Queued => t.text,
    }
}

/// Render Settings menu: (label, params). `{"form": …}` entries open a form.
fn settings_menu(it: &RenderQueueItem, templates: &RenderTemplates) -> Vec<(String, Value)> {
    let s = &it.settings;
    let mut v = vec![];
    for t in templates.render_settings_list() {
        v.push((mark(s.name == t.name, &t.name), json!({"template": t.name})));
    }
    v.push(("   Make Template…".into(), json!({"form": "makeRenderTemplate"})));
    v.push(("-".into(), Value::Null));
    v.push((mark(s.quality == RenderQuality::Best, "Quality: Best"), json!({"quality": "best"})));
    v.push((mark(s.quality == RenderQuality::Draft, "Quality: Draft"), json!({"quality": "draft"})));
    for (l, k) in [("Full", "full"), ("Half", "half"), ("Third", "third"), ("Quarter", "quarter")] {
        v.push((mark(s.resolution_label() == l, &format!("Resolution: {l}")), json!({"resolution": k})));
    }
    v.push(("-".into(), Value::Null));
    for (u, k) in effectcraft_engine::project::render_queue::ProxyUse::ALL.iter().zip(["current", "all", "comp", "none"]) {
        v.push((mark(s.proxy_use == *u, &format!("Proxy Use: {}", u.label())), json!({"proxyUse": k})));
    }
    for (e, k) in EffectsMode::ALL.iter().zip(["current", "allOn", "allOff"]) {
        v.push((mark(s.effects == *e, &format!("Effects: {}", e.label())), json!({"effects": k})));
    }
    for (e, k) in [(CurrentOrOff::Current, "current"), (CurrentOrOff::AllOff, "allOff")] {
        v.push((mark(s.solo == e, &format!("Solo Switches: {}", e.label())), json!({"solo": k})));
    }
    for (e, k) in [(CurrentOrOff::Current, "current"), (CurrentOrOff::AllOff, "allOff")] {
        v.push((mark(s.guide_layers == e, &format!("Guide Layers: {}", e.label())), json!({"guideLayers": k})));
    }
    for (d, k) in ColorDepth::ALL.iter().zip(["current", "8", "16", "32"]) {
        v.push((mark(s.color_depth == *d, &format!("Color Depth: {}", d.label())), json!({"colorDepth": k})));
    }
    v.push(("-".into(), Value::Null));
    for (m, k) in SwitchOverride::ALL.iter().zip(["current", "onForChecked", "offForAll"]) {
        v.push((mark(s.frame_blending == *m, &format!("Frame Blending: {}", m.label())), json!({"frameBlending": k})));
    }
    for (m, k) in SwitchOverride::ALL.iter().zip(["current", "onForChecked", "offForAll"]) {
        v.push((mark(s.motion_blur_override() == *m, &format!("Motion Blur: {}", m.label())), json!({"motionBlur": k})));
    }
    for (f, k) in FieldRender::ALL.iter().zip(["off", "upper", "lower"]) {
        v.push((mark(s.field_render == *f, &format!("Field Render: {}", f.label())), json!({"fieldRender": k})));
    }
    if s.field_render != FieldRender::Off {
        for pd in Pulldown::ALL {
            v.push((mark(s.pulldown == pd, &format!("3:2 Pulldown: {}", pd.label())), json!({"pulldown": pd.label()})));
        }
    }
    v.push(("-".into(), Value::Null));
    v.push((mark(s.time_span == TimeSpan::WorkArea, "Time Span: Work Area Only"), json!({"timeSpan": "workArea"})));
    v.push((mark(s.time_span == TimeSpan::LengthOfComp, "Time Span: Length of Comp"), json!({"timeSpan": "comp"})));
    v.push((mark(matches!(s.time_span, TimeSpan::Custom { .. }), "Time Span: Custom…"), json!({"form": "timeSpan"})));
    v.push((mark(s.frame_rate.is_none(), "Frame Rate: Use comp's frame rate"), json!({"frameRate": null})));
    for fps in [12.0, 15.0, 23.976, 24.0, 25.0, 29.97, 30.0, 50.0, 60.0] {
        let on = s.frame_rate.is_some_and(|r| (r.as_f64() - fps).abs() < 0.01);
        v.push((mark(on, &format!("Frame Rate: {fps}")), json!({"frameRate": fps})));
    }
    v.push(("   Frame Rate: Custom…".into(), json!({"form": "frameRate"})));
    v.push(("-".into(), Value::Null));
    v.push((mark(s.skip_existing, "Skip Existing Files"), json!({"skipExisting": !s.skip_existing})));
    v.push((mark(s.storage_overflow, "Use Storage Overflow"), json!({"storageOverflow": !s.storage_overflow})));
    v
}

/// Output Module menu: (label, params). `{"form": …}` entries open a form.
fn output_menu(it: &RenderQueueItem, available: &[OutputFormat], templates: &RenderTemplates) -> Vec<(String, Value)> {
    let o = &it.output;
    let mut v = Vec::new();
    for t in templates.output_module_list() {
        if available.contains(&t.format) {
            v.push((mark(o.name == t.name, &t.name), json!({"template": t.name})));
        }
    }
    v.push(("   Make Template…".into(), json!({"form": "makeOutputTemplate"})));
    v.push(("-".into(), Value::Null));
    for f in OutputFormat::ALL {
        if available.contains(&f) {
            v.push((mark(o.format == f, &format!("Format: {}", f.label())), json!({"format": format!("{f:?}")})));
        }
    }
    v.push(("-".into(), Value::Null));
    v.push((mark(o.channels == Channels::Rgb, "Channels: RGB"), json!({"channels": "rgb"})));
    if o.supports_alpha() {
        v.push((mark(o.channels == Channels::Rgba, "Channels: RGB + Alpha"), json!({"channels": "rgba"})));
    }
    if !o.format.is_audio_only() {
        v.push((mark(o.channels == Channels::Alpha, "Channels: Alpha"), json!({"channels": "alpha"})));
    }
    if o.channels == Channels::Rgba {
        v.push((mark(o.alpha_mode == AlphaMode::Straight, "Color: Straight (Unmatted)"), json!({"color": "straight"})));
        v.push((mark(o.alpha_mode == AlphaMode::Premultiplied, "Color: Premultiplied (Matted)"), json!({"color": "premultiplied"})));
    }
    match o.format {
        OutputFormat::ProRes => {
            v.push(("-".into(), Value::Null));
            for p in ProResProfile::ALL {
                v.push((mark(o.prores_profile == p, p.label()), json!({"proresProfile": format!("{p:?}")})));
            }
        }
        OutputFormat::H264 => {
            v.push(("-".into(), Value::Null));
            for kbps in [2_000, 5_000, 10_000, 15_000, 20_000, 40_000] {
                v.push((mark(o.bitrate_kbps == kbps, &format!("Bitrate: {} Mbps", kbps / 1000)), json!({"bitrate": kbps})));
            }
        }
        OutputFormat::Hevc | OutputFormat::Av1 => codec_menu(o, &mut v),
        OutputFormat::WebM => {
            v.push(("-".into(), Value::Null));
            for c in [WebmVideoCodec::Vp9, WebmVideoCodec::Av1] {
                v.push((mark(o.webm_codec == c, &format!("Video Codec: {}", c.label())), json!({"webmCodec": c.label().to_ascii_lowercase()})));
            }
            if o.webm_codec == WebmVideoCodec::Av1 {
                codec_menu(o, &mut v);
            } else {
                v.push(("-".into(), Value::Null));
                for q in [60u8, 80, 90, 100] {
                    v.push((mark(o.quality == q && !o.webm_bitrate, &format!("Quality: {q}")), json!({"quality": q, "webmBitrate": false})));
                }
                v.push((mark(o.webm_bitrate, &format!("Target Bitrate ({} kbps)…", o.bitrate_kbps)), json!({"form": "bitrate"})));
                v.push((mark(o.keyframe_interval > 0, "Key Frame Interval…"), json!({"form": "keyframes"})));
            }
            v.push(("-".into(), Value::Null));
            for kbps in [24u32, 32, 64, 96, 128, 192, 256] {
                v.push((mark(o.opus_bitrate_kbps == kbps, &format!("Opus Bitrate: {kbps} kbps")), json!({"audioBitrate": kbps})));
            }
            for (a, k) in [(OpusApplication::Audio, "audio"), (OpusApplication::Voip, "voice")] {
                v.push((mark(o.opus_application == a, &format!("Opus Tuning: {}", a.label())), json!({"opusApplication": k})));
            }
        }
        OutputFormat::JpegSequence => {
            v.push(("-".into(), Value::Null));
            for q in [60u8, 80, 90, 100] {
                v.push((mark(o.quality == q, &format!("Quality: {q}")), json!({"quality": q})));
            }
        }
        OutputFormat::Gif => {
            v.push(("-".into(), Value::Null));
            v.push((mark(o.gif_loop, "Loop Forever"), json!({"loop": !o.gif_loop})));
        }
        _ => {}
    }
    if !o.format.is_audio_only() {
        v.push(("-".into(), Value::Null));
        v.push((mark(!o.crop.enabled, "Crop: Off"), json!({"crop": false})));
        v.push((mark(o.crop.enabled && o.crop.use_roi, "Crop: Use Region of Interest"), json!({"crop": {"useRoi": true}})));
        v.push((mark(o.crop.enabled && !o.crop.use_roi, "Crop: Edges…"), json!({"form": "crop"})));
        v.push((mark(!o.resize.enabled, "Resize: Off"), json!({"resize": false})));
        for (n, w, h) in RESIZE_PRESETS {
            let on = o.resize.enabled && o.resize.width == w && o.resize.height == h && !o.resize.lock_aspect;
            v.push((mark(on, &format!("Resize to: {n} ({w}×{h})")), json!({"resize": {"preset": n}})));
        }
        v.push((mark(o.resize.enabled, "Resize to: Custom…"), json!({"form": "resize"})));
        v.push((
            mark(o.resize.lock_aspect, "Resize: Lock Aspect Ratio"),
            json!({"resize": {"enabled": o.resize.enabled, "lockAspect": !o.resize.lock_aspect}}),
        ));
        for (q, k) in [(ResizeQuality::Low, "low"), (ResizeQuality::High, "high")] {
            v.push((mark(o.resize.quality == q, &format!("Resize Quality: {q:?}")), json!({"resize": {"enabled": o.resize.enabled, "quality": k}})));
        }
    }
    v.push(("-".into(), Value::Null));
    for (a, k) in [
        (PostRenderAction::None, "none"),
        (PostRenderAction::Import, "import"),
        (PostRenderAction::ImportAndReplace, "importAndReplace"),
        (PostRenderAction::SetProxy, "setProxy"),
    ] {
        v.push((mark(it.post_render == a, &format!("Post-Render Action: {}", a.label())), json!({"postRenderAction": k})));
    }
    v.push((mark(o.include_project_link, "Include Project Link"), json!({"includeProjectLink": !o.include_project_link})));
    if o.format.supports_audio() {
        v.push(("-".into(), Value::Null));
        for (a, l) in [(AudioOutput::Auto, "auto"), (AudioOutput::On, "on"), (AudioOutput::Off, "off")] {
            let label = match a {
                AudioOutput::Auto => "Audio Output: Auto",
                AudioOutput::On => "Audio Output: On",
                AudioOutput::Off => "Audio Output: Off",
            };
            v.push((mark(o.audio == a, label), json!({"audio": l})));
        }
        for sr in [44_100u32, 48_000, 96_000] {
            v.push((mark(o.audio_sample_rate == sr, &format!("Sample Rate: {:.1} kHz", sr as f64 / 1000.0)), json!({"sampleRate": sr})));
        }
        v.push((mark(o.audio_channels == 1, "Audio Channels: Mono"), json!({"audioChannels": "mono"})));
        v.push((mark(o.audio_channels != 1, "Audio Channels: Stereo"), json!({"audioChannels": "stereo"})));
        if matches!(o.format, OutputFormat::ProRes | OutputFormat::Wav | OutputFormat::Aiff) {
            for f in [AudioFormat::S16, AudioFormat::S24, AudioFormat::F32] {
                v.push((mark(o.audio_format == f, &format!("Sample Format: {}", f.label())), json!({"audioFormat": f.label()})));
            }
        }
    }
    v
}

/// HEVC / AV1 options: profile, level, rate control (bitrate or constant quality), key frames.
fn codec_menu(o: &effectcraft_engine::project::render_queue::OutputModule, v: &mut Vec<(String, Value)>) {
    use effectcraft_engine::project::render_queue::{CodecProfile, RateControlMode, VideoCodecOptions};
    let c = &o.codec;
    v.push(("-".into(), Value::Null));
    for p in CodecProfile::ALL {
        v.push((mark(c.profile == p, &format!("Profile: {}", p.label())), json!({"profile": format!("{p:?}").to_ascii_lowercase()})));
    }
    v.push(("-".into(), Value::Null));
    v.push((mark(c.level.is_none(), "Level: Auto"), json!({"level": "auto"})));
    // The common HEVC / AV1 levels (every level is reachable over the control channel).
    for l in [30u8, 31, 40, 41, 50, 51, 52] {
        debug_assert!(VideoCodecOptions::HEVC_LEVELS.contains(&l) && VideoCodecOptions::AV1_LEVELS.contains(&l));
        let name = format!("{}.{}", l / 10, l % 10);
        v.push((mark(c.level == Some(l), &format!("Level: {name}")), json!({"level": name})));
    }
    v.push(("-".into(), Value::Null));
    for r in [RateControlMode::Bitrate, RateControlMode::Quality] {
        v.push((mark(c.rate_control == r, &format!("Rate Control: {}", r.label())), json!({"rateControl": format!("{r:?}").to_ascii_lowercase()})));
    }
    if c.rate_control == RateControlMode::Bitrate {
        for kbps in [1_000u32, 2_000, 5_000, 10_000, 20_000, 40_000] {
            let l = if kbps < 2_000 { format!("Bitrate: {kbps} kbps") } else { format!("Bitrate: {} Mbps", kbps / 1000) };
            v.push((mark(o.bitrate_kbps == kbps, &l), json!({"bitrate": kbps})));
        }
        v.push((mark(false, "Bitrate: Custom…"), json!({"form": "codecBitrate"})));
    } else {
        for q in [40u8, 60, 70, 80, 90, 100] {
            v.push((mark(c.quality == q, &format!("Quality: {q}")), json!({"quality": q})));
        }
    }
    v.push(("-".into(), Value::Null));
    for k in [0u32, 1, 30, 60, 120, 250] {
        let l = match k {
            0 => "Key Frames: Auto (2 s)".to_string(),
            1 => "Key Frames: Every Frame (all intra)".to_string(),
            k => format!("Key Frames: Every {k} Frames"),
        };
        v.push((mark(o.keyframe_interval == k, &l), json!({"keyframeInterval": k})));
    }
    v.push((mark(false, "Key Frame Interval…"), json!({"form": "keyframes"})));
}

/// Open the form behind a `{"form": …}` menu entry.
fn open_menu_form(app: &mut EffectcraftApp, it: &RenderQueueItem, form: &str) {
    use crate::panels::forms::{Field, form as open};
    let id = it.id;
    let o = &it.output;
    match form {
        "makeRenderTemplate" => open(
            app,
            "Make Render Settings Template",
            "renderQueue.saveTemplate",
            json!({"kind": "renderSettings", "item": id}),
            vec![Field::text("name", "Template name", "")],
        ),
        "makeOutputTemplate" => open(
            app,
            "Make Output Module Template",
            "renderQueue.saveTemplate",
            json!({"kind": "outputModule", "item": id}),
            vec![Field::text("name", "Template name", "")],
        ),
        "timeSpan" => {
            let comp = app.session.project.comp(it.comp);
            let (a, b) = comp.map(|c| it.settings.span(c)).unwrap_or_default();
            open(
                app,
                "Custom Time Span",
                "renderQueue.setRenderSettings",
                json!({"item": id, "timeSpan": "custom"}),
                vec![Field::num("start", "Start (s)", a.seconds()), Field::num("duration", "Duration (s)", (b.seconds() - a.seconds()).max(0.04))],
            )
        }
        "frameRate" => {
            let r = app.session.project.comp(it.comp).map(|c| it.settings.rate(c).as_f64()).unwrap_or(30.0);
            open(app, "Use This Frame Rate", "renderQueue.setRenderSettings", json!({"item": id}), vec![Field::num("frameRate", "Frames per second", r)])
        }
        "crop" => open(
            app,
            "Crop",
            "renderQueue.setOutputModule",
            json!({"item": id}),
            vec![
                Field::num("cropTop", "Top", o.crop.top as f64),
                Field::num("cropLeft", "Left", o.crop.left as f64),
                Field::num("cropBottom", "Bottom", o.crop.bottom as f64),
                Field::num("cropRight", "Right", o.crop.right as f64),
            ],
        ),
        "resize" => open(
            app,
            "Resize to",
            "renderQueue.setOutputModule",
            json!({"item": id}),
            vec![Field::num("resizeWidth", "Width", o.resize.width as f64), Field::num("resizeHeight", "Height", o.resize.height as f64)],
        ),
        "bitrate" => open(
            app,
            "WebM Target Bitrate",
            "renderQueue.setOutputModule",
            json!({"item": id, "webmBitrate": true}),
            vec![Field::num("bitrate", "Bitrate (kbps)", o.bitrate_kbps as f64)],
        ),
        "codecBitrate" => open(
            app,
            "Target Bitrate",
            "renderQueue.setOutputModule",
            json!({"item": id, "rateControl": "bitrate"}),
            vec![Field::num("bitrate", "Bitrate (kbps)", o.bitrate_kbps as f64)],
        ),
        "keyframes" => open(
            app,
            "Key Frame Interval",
            "renderQueue.setOutputModule",
            json!({"item": id}),
            vec![Field::num("keyframeInterval", "Frames between key frames (0 = every 2 s)", o.keyframe_interval as f64)],
        ),
        _ => {}
    }
}

const OUTPUT_TEMPLATES: [&str; 4] = [
    "[compName].[fileExtension]",
    "[compName]_[width]x[height].[fileExtension]",
    "[projectName]_[compName].[fileExtension]",
    "[compName]/[compName].[fileExtension]",
];

fn output_to_menu(it: &RenderQueueItem) -> Vec<(String, Value)> {
    let mut v: Vec<(String, Value)> = OUTPUT_TEMPLATES
        .iter()
        .map(|t| {
            let t = if it.output.format.is_sequence() { t.replace(".[fileExtension]", "_[#####].[fileExtension]") } else { t.to_string() };
            (t.clone(), json!({"path": t}))
        })
        .collect();
    v.push(("-".into(), Value::Null));
    v.push(("Choose File…".into(), json!({"choose": true})));
    v
}

/// Output To's save dialog, as After Effects' Output Movie To: it opens at the current output
/// (folder and name) and offers the output format's extension. `None` when the host has no
/// save dialog for other file kinds; `Some(None)` when the dialog was cancelled.
fn pick_output(app: &EffectcraftApp, it: &RenderQueueItem) -> Option<Option<String>> {
    let pick = app.hooks.pick_save_file.as_ref()?;
    let default = app.session.resolve_output(it).unwrap_or_default();
    Some(pick(&default, it.output.format.extension()))
}

// A labelled control shares the same 26-point height and 8-point text inset as editor fields.
#[allow(clippy::too_many_arguments)]
fn choice(
    app: &mut EffectcraftApp,
    ui: &mut egui::Ui,
    rect: Rect,
    it: &RenderQueueItem,
    key: &str,
    label: &str,
    shown: &str,
    options: &[(String, Value)],
    enabled: bool,
) -> Option<Value> {
    let t = app.tokens;
    widgets::text_fit(ui.painter(), rect.min, Align2::LEFT_TOP, label, Tokens::ui(11.0), rect.width(), t.text_dim);
    let control = Rect::from_min_max(rect.min + vec2(0.0, 17.0), rect.max);
    let id = egui::Id::new(("rq-choice", it.id, key));
    ui.scope(|ui| {
        if !enabled {
            ui.disable();
        }
        if widgets::dropdown(ui, control, shown, &t, id).clicked() {
            widgets::open_popup(ui, id);
        }
    });
    app.auto.add(&format!("renderQueue.item.{}.{}", it.id, key), control, shown);
    let labels: Vec<String> = options.iter().map(|(label, _)| label.clone()).collect();
    widgets::popup_menu(ui, id, control.left_bottom(), &labels, None).and_then(|i| options.get(i).map(|(_, v)| v.clone())).filter(|_| enabled)
}
#[allow(clippy::too_many_arguments)]
fn button(app: &mut EffectcraftApp, ui: &mut egui::Ui, rect: Rect, id: &str, text: &str, primary: bool, enabled: bool) -> bool {
    let t = app.tokens;
    app.auto.add(id, rect, text);
    ui.scope(|ui| {
        if !enabled {
            ui.disable();
        }
        widgets::text_button(ui, rect, text, primary, &t, egui::Id::new(id)).clicked()
    })
    .inner
}
fn blocked_reason(app: &EffectcraftApp, it: &RenderQueueItem, available: &[OutputFormat]) -> Option<String> {
    if app.session.project.comp(it.comp).is_none() {
        return Some("Composition is missing. Remove this item or add another composition.".into());
    }
    if !available.contains(&it.output.format) {
        return Some(format!("{} export is unavailable in this build. Choose an available format.", it.output.format.label()));
    }
    match app.session.resolve_output(it) {
        Some(path) if !path.trim().is_empty() => None,
        _ => Some("Choose an output file before rendering.".into()),
    }
}
pub fn show(app: &mut EffectcraftApp, ui: &mut egui::Ui, rect: Rect) {
    let t = app.tokens;
    let p = ui.painter().with_clip_rect(rect);
    let queue = app.session.project.render_queue.clone();
    let progress = app.session.render_progress();
    let rendering = app.session.is_rendering();
    let available = app.session.exporter.as_ref().map(|e| e.formats()).unwrap_or_default();
    let mut actions: Vec<(&'static str, Value)> = Vec::new();
    let mut forms: Vec<(RenderQueueItem, String)> = Vec::new();
    let state_id = egui::Id::new("rq-ui");
    let (mut open, mut selected): (Vec<u64>, Option<u64>) = ui.data(|d| d.get_temp(state_id)).unwrap_or_else(|| {
        let first = queue.first().map(|it| it.id);
        (first.into_iter().collect(), first)
    });
    if selected.is_none()
        && open.is_empty()
        && let Some(first) = queue.first()
    {
        open.push(first.id);
        selected = Some(first.id);
    }
    app.auto.add("renderQueue.drop", rect, "Drop compositions to queue");
    if !rendering
        && let Some(crate::panels::DragPayload::Item(id)) = egui::DragAndDrop::payload::<crate::panels::DragPayload>(ui.ctx()).as_deref()
        && app.session.project.comp(effectcraft_engine::project::ItemId(*id)).is_some()
        && ui.input(|i| i.pointer.hover_pos()).is_some_and(|pos| rect.contains(pos))
    {
        let ids: Vec<u64> = if app.session.state.project_selection.contains(&effectcraft_engine::project::ItemId(*id)) {
            app.session.state.project_selection.iter().filter(|i| app.session.project.comp(**i).is_some()).map(|i| i.0).collect()
        } else {
            vec![*id]
        };
        if ui.input(|i| i.pointer.any_released()) {
            for comp in ids {
                actions.push(("renderQueue.add", json!({"comp":comp})));
            }
            egui::DragAndDrop::clear_payload(ui.ctx());
        } else {
            p.rect_stroke(rect.shrink(2.0), 3.0, Stroke::new(2.0, t.accent), StrokeKind::Inside);
        }
    }
    let pending = |it: &&RenderQueueItem| it.is_queued() || (it.render && it.status == RenderStatus::NeedsOutput);
    let queued = queue.iter().filter(pending).count();
    let block = queue.iter().filter(pending).find_map(|it| blocked_reason(app, it, &available));
    let can_render = !rendering && queued > 0 && block.is_none();
    let toolbar = Rect::from_min_size(rect.min + vec2(12.0, 10.0), vec2((rect.width() - 24.0).max(0.0), 28.0));
    let action_width = 112.0_f32.min(toolbar.width() * 0.45);
    let render_button = Rect::from_min_max(pos2(toolbar.max.x - action_width, toolbar.min.y), toolbar.max);
    let add_button = Rect::from_min_size(toolbar.min, vec2((toolbar.width() - action_width - 8.0).clamp(0.0, 164.0), 28.0));
    if button(app, ui, add_button, "renderQueue.add", "Add composition", false, !rendering && app.session.active_comp().is_some()) {
        actions.push(("renderQueue.add", json!({})));
    }
    if rendering {
        if button(app, ui, render_button, "renderQueue.stop", "Stop render", false, true) {
            actions.push(("renderQueue.stop", json!({})));
        }
    } else if button(app, ui, render_button, "renderQueue.render", &format!("Render ({queued})"), true, can_render) {
        actions.push(("renderQueue.render", json!({"wait":false})));
    }
    let message = if let Some(pr) = progress.as_ref().filter(|_| rendering) {
        let name = pr
            .current
            .and_then(|id| queue.iter().find(|it| it.id == id))
            .and_then(|it| app.session.project.item(it.comp))
            .map(|it| it.name.as_str())
            .unwrap_or("Composition");
        format!(
            "{name} · {} / {} frames · elapsed {} · remaining {}",
            pr.done,
            pr.total,
            hms(pr.elapsed),
            pr.remaining.map(hms).unwrap_or_else(|| "Calculating…".into())
        )
    } else if available.is_empty() {
        "Export is unavailable in this build".into()
    } else if let Some(reason) = block {
        reason
    } else if queued == 0 {
        "Add a composition or drag one here to start.".into()
    } else {
        format!("{queued} ready to render · choose the format and output file below")
    };
    let info_y = toolbar.max.y + 14.0;
    widgets::text_fit(
        &p,
        pos2(toolbar.min.x, info_y),
        Align2::LEFT_CENTER,
        &message,
        Tokens::ui(11.5),
        toolbar.width(),
        if !rendering && !can_render && queued > 0 { t.warning } else { t.text_dim },
    );
    let bar = Rect::from_min_size(pos2(toolbar.min.x, info_y + 12.0), vec2(toolbar.width(), 4.0));
    p.rect_filled(bar, 2.0, t.field_bg);
    let fraction =
        progress.as_ref().filter(|_| rendering).filter(|pr| pr.total > 0).map(|pr| (pr.done as f32 / pr.total as f32).clamp(0.0, 1.0)).unwrap_or(0.0);
    if fraction > 0.0 {
        p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * fraction, 4.0)), 2.0, t.accent);
    }
    app.auto.add("renderQueue.progress", bar, &message);
    let foot = Rect::from_min_max(pos2(rect.min.x, (rect.max.y - 28.0).max(bar.max.y + 8.0)), rect.max);
    let list = Rect::from_min_max(pos2(rect.min.x, bar.max.y + 10.0), pos2(rect.max.x, foot.min.y.max(bar.max.y + 10.0)));
    let panel_clip = ui.clip_rect();
    ui.set_clip_rect(list.intersect(panel_clip));
    let lp = p.with_clip_rect(list.intersect(panel_clip));
    let scroll = widgets::PanelScroll::begin(ui, egui::Id::new("rq-scroll"), list);
    let mut y = list.min.y - scroll.offset;
    let editing_id = egui::Id::new("rq-edit-output");
    let mut editing: Option<(u64, String)> = ui.data(|d| d.get_temp(editing_id));
    for (index, it) in queue.iter().enumerate() {
        let aid = |key: &str| format!("renderQueue.item.{}.{}", it.id, key);
        let is_open = open.contains(&it.id);
        let width = (list.width() - 44.0).max(0.0);
        let columns = if width >= 560.0 { 2 } else { 1 };
        let settings_height = 48.0 * (4 / columns) as f32;
        let advanced_height = 48.0 * 3_usize.div_ceil(columns) as f32;
        let reason = if it.render { blocked_reason(app, it, &available) } else { None };
        let error = match &it.status {
            RenderStatus::Failed(e) => Some(e.clone()),
            _ => reason,
        };
        let status_height = if width < 420.0 { 24.0 } else { 0.0 };
        let extra = if error.is_some() { 28.0 } else { 0.0 };
        let height = if is_open { 40.0 + status_height + 56.0 + settings_height + advanced_height + 30.0 + extra + 12.0 } else { 68.0 };
        let card = Rect::from_min_size(pos2(list.min.x + 8.0, y), vec2((list.width() - 20.0).max(0.0), height));
        y += height + 8.0;
        lp.rect_filled(card, 4.0, t.row);
        lp.rect_stroke(card, 4.0, Stroke::new(1.0, if selected == Some(it.id) { t.field_border } else { t.separator }), StrokeKind::Inside);
        let header = Rect::from_min_size(card.min, vec2(card.width(), 36.0));
        let response = ui.interact(header, egui::Id::new(("rq-row", it.id)), Sense::click());
        if response.clicked() {
            selected = Some(it.id);
            app.ui.focused = crate::dock::PanelKind::RenderQueue;
        }
        response.context_menu(|ui| {
            ui.add_enabled_ui(!rendering, |ui| {
                for (label, command, params) in [
                    ("Duplicate", "renderQueue.duplicate", json!({"item":it.id})),
                    ("Remove", "renderQueue.remove", json!({"item":it.id})),
                    ("Move Up", "renderQueue.move", json!({"item":it.id,"to":index})),
                    ("Move Down", "renderQueue.move", json!({"item":it.id,"to":index + 2})),
                ] {
                    if ((label != "Move Up" || index > 0) && (label != "Move Down" || index + 1 < queue.len())) && ui.button(label).clicked() {
                        actions.push((command, params));
                        ui.close();
                    }
                }
            });
        });
        app.auto.add(&aid("row"), header, &format!("Item {}", index + 1));
        let twirl = Rect::from_min_size(header.min + vec2(8.0, 6.0), vec2(24.0, 24.0));
        if widgets::twirl(ui, twirl, is_open, egui::Id::new(("rq-twirl", it.id)), &t).clicked() {
            selected = Some(it.id);
            if is_open {
                open.retain(|id| *id != it.id);
            } else {
                open.push(it.id);
            }
        }
        app.auto.add(&aid("twirl"), twirl, if is_open { "Collapse settings" } else { "Expand settings" });
        let checkbox = Rect::from_min_size(pos2(twirl.max.x + 4.0, twirl.min.y), vec2(24.0, 24.0));
        ui.scope(|ui| {
            if rendering {
                ui.disable();
            }
            if widgets::checkbox(ui, checkbox, it.render, &t, egui::Id::new(("rq-check", it.id))).clicked() {
                actions.push(("renderQueue.setRender", json!({"item":it.id,"render":!it.render})));
            }
        });
        app.auto.add(&aid("render"), checkbox, if it.render { "Include in render" } else { "Skip render" });
        let remove = Rect::from_min_size(pos2(header.max.x - 32.0, twirl.min.y), vec2(24.0, 24.0));
        ui.scope(|ui| {
            if rendering {
                ui.disable();
            }
            if widgets::icon_button(ui, remove, crate::icons::Icon::Trash, false, &t, egui::Id::new(("rq-remove", it.id)))
                .on_hover_text("Remove from queue")
                .clicked()
            {
                actions.push(("renderQueue.remove", json!({"item":it.id})));
            }
        });
        app.auto.add(&aid("remove"), remove, "Remove from queue");
        let status_rect = if width >= 420.0 {
            Rect::from_min_max(pos2((remove.min.x - 112.0).max(checkbox.max.x + 8.0), header.min.y + 5.0), pos2(remove.min.x - 8.0, header.max.y - 5.0))
        } else {
            Rect::from_min_size(pos2(card.min.x + 12.0, header.max.y), vec2((card.width() - 24.0).max(0.0), 24.0))
        };
        let status = if rendering && progress.as_ref().is_some_and(|pr| pr.current == Some(it.id)) {
            format!("Rendering {:.0}%", fraction * 100.0)
        } else {
            it.status.label().to_owned()
        };
        let name_end = if width >= 420.0 { status_rect.min.x - 8.0 } else { remove.min.x - 8.0 };
        let name = app.session.project.item(it.comp).map(|it| it.name.as_str()).unwrap_or("Missing composition");
        widgets::text_fit(
            &lp,
            pos2(checkbox.max.x + 8.0, header.center().y),
            Align2::LEFT_CENTER,
            name,
            Tokens::semibold(12.5),
            (name_end - checkbox.max.x - 8.0).max(0.0),
            t.text,
        );
        if width >= 420.0 || is_open {
            let retry = matches!(it.status, RenderStatus::Done | RenderStatus::Failed(_) | RenderStatus::UserStopped);
            if retry && !rendering {
                let label = if it.status == RenderStatus::Done { "Render again" } else { "Retry render" };
                if button(app, ui, status_rect, &aid("requeue"), label, false, true) {
                    actions.push(("renderQueue.setRender", json!({"item":it.id,"render":true})));
                }
            } else {
                widgets::text_fit(
                    &lp,
                    status_rect.right_center(),
                    Align2::RIGHT_CENTER,
                    &status,
                    Tokens::ui(11.5),
                    status_rect.width(),
                    status_color(&t, &it.status),
                );
            }
        }
        app.auto.add(&aid("status"), status_rect, &status);
        let content_x = card.min.x + 12.0;
        let content_right = card.max.x - 12.0;
        let content_width = (content_right - content_x).max(0.0);
        if !is_open {
            let summary =
                format!("{} · {} · {}", status, it.output.format.label(), app.session.resolve_output(it).unwrap_or_else(|| "Choose output file".into()));
            widgets::text_fit(&lp, pos2(content_x, header.max.y + 13.0), Align2::LEFT_CENTER, &summary, Tokens::ui(11.5), content_width, t.text_dim);
            continue;
        }
        let output_y = header.max.y + status_height + 4.0;
        widgets::text_fit(&lp, pos2(content_x, output_y), Align2::LEFT_TOP, "Output file", Tokens::ui(11.0), content_width, t.text_dim);
        let browse = Rect::from_min_size(pos2((content_right - 84.0).max(content_x), output_y + 17.0), vec2(84.0_f32.min(content_width), 26.0));
        let template = Rect::from_min_size(pos2((browse.min.x - 32.0).max(content_x), browse.min.y), vec2(24.0, 26.0));
        let path_rect = Rect::from_min_max(pos2(content_x, browse.min.y), pos2((template.min.x - 8.0).max(content_x), browse.max.y));
        let mut choose_file = button(app, ui, browse, &aid("browse"), "Browse…", false, !rendering);
        let oid = egui::Id::new(("rq-out", it.id));
        if widgets::dropdown(ui, template, "", &t, oid).clicked() && !rendering {
            widgets::open_popup(ui, oid);
        }
        app.auto.add(&aid("outputTo"), template, "Filename templates and edit path");
        let mut menu = output_to_menu(it);
        menu.push(("Edit path / template…".into(), json!({"edit":true})));
        let labels: Vec<String> = menu.iter().map(|(l, _)| l.clone()).collect();
        if !rendering
            && let Some(i) = widgets::popup_menu(ui, oid, template.left_bottom(), &labels, None)
            && let Some((_, params)) = menu.get(i)
        {
            if params.get("choose").is_some() {
                choose_file = true;
            } else if params.get("edit").is_some() {
                editing = Some((it.id, it.output.output.clone()));
            } else {
                let mut params = params.clone();
                params["item"] = json!(it.id);
                actions.push(("renderQueue.setOutput", params));
            }
        }
        if editing.as_ref().is_some_and(|(id, _)| *id == it.id) {
            let mut buf = editing.as_ref().map(|(_, b)| b.clone()).unwrap_or_default();
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(path_rect));
            let response = child.add(
                egui::TextEdit::singleline(&mut buf)
                    .id_salt(("rq-editor", it.id))
                    .desired_width(path_rect.width())
                    .font(Tokens::ui(12.0))
                    .margin(egui::Margin::symmetric(8, 4)),
            );
            app.auto.add(&aid("outputPath"), path_rect, &buf);
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                editing = None;
                response.surrender_focus();
            } else if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if buf.trim().is_empty() {
                    app.ui.status = "Output path cannot be empty".into();
                    widgets::keep_focus(ui.ctx(), &response, &buf);
                    editing = Some((it.id, buf));
                } else {
                    actions.push(("renderQueue.setOutput", json!({"item":it.id,"path":buf.trim()})));
                    editing = None;
                    response.surrender_focus();
                }
            } else {
                widgets::keep_focus(ui.ctx(), &response, &buf);
                editing = Some((it.id, buf));
            }
        } else {
            let shown = app.session.resolve_output(it).unwrap_or_else(|| "Choose an output file…".into());
            lp.rect_filled(path_rect, 3.0, t.field_bg);
            lp.rect_stroke(path_rect, 3.0, Stroke::new(1.0, t.field_border), StrokeKind::Inside);
            widgets::text_fit(
                &lp,
                pos2(path_rect.min.x + 8.0, path_rect.center().y),
                Align2::LEFT_CENTER,
                &shown,
                Tokens::ui(12.0),
                (path_rect.width() - 16.0).max(0.0),
                t.hot_text,
            );
            let response = ui.interact(path_rect, egui::Id::new(("rq-path", it.id)), Sense::click());
            if response.clicked() && !rendering {
                if ui.input(|i| i.modifiers.alt) {
                    editing = Some((it.id, it.output.output.clone()));
                } else {
                    choose_file = true;
                }
            }
            response.on_hover_text(format!("{shown}\nTemplate: {}\nClick to choose a file; use the arrow menu to edit the path.", it.output.output));
            app.auto.add(&aid("outputPath"), path_rect, &shown);
        }
        if choose_file {
            let picked = pick_output(app, it).or_else(|| {
                let default = app.session.resolve_output(it).unwrap_or_default();
                let name = std::path::Path::new(&default).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                app.hooks.pick_save.as_ref().map(|f| f(&name))
            });
            match picked {
                Some(Some(path)) => actions.push(("renderQueue.setOutput", json!({"item":it.id,"path":path}))),
                Some(None) => {}
                None => editing = Some((it.id, it.output.output.clone())),
            }
        }
        let quality = vec![
            (mark(it.settings.quality == RenderQuality::Best, "Best"), json!({"quality":"best"})),
            (mark(it.settings.quality == RenderQuality::Draft, "Draft"), json!({"quality":"draft"})),
        ];
        let resolutions: Vec<_> = [("Full", "full"), ("Half", "half"), ("Third", "third"), ("Quarter", "quarter")]
            .into_iter()
            .map(|(l, v)| (mark(it.settings.resolution_label() == l, l), json!({"resolution":v})))
            .collect();
        let formats: Vec<_> = available.iter().map(|f| (mark(it.output.format == *f, f.label()), json!({"format":format!("{f:?}")}))).collect();
        let spans = vec![
            (mark(it.settings.time_span == TimeSpan::WorkArea, "Work area"), json!({"timeSpan":"workArea"})),
            (mark(it.settings.time_span == TimeSpan::LengthOfComp, "Entire composition"), json!({"timeSpan":"comp"})),
            (mark(matches!(it.settings.time_span, TimeSpan::Custom { .. }), "Custom range…"), json!({"form":"timeSpan"})),
        ];
        let span_label = match it.settings.time_span {
            TimeSpan::WorkArea => "Work area",
            TimeSpan::LengthOfComp => "Entire composition",
            TimeSpan::Custom { .. } => "Custom range",
        };
        let quality_label = format!("{:?}", it.settings.quality);
        let fields = [
            ("format", "Format", it.output.format.label(), formats, "renderQueue.setOutputModule"),
            ("timeSpan", "Render range", span_label, spans, "renderQueue.setRenderSettings"),
            ("quality", "Quality", quality_label.as_str(), quality, "renderQueue.setRenderSettings"),
            ("resolution", "Resolution", it.settings.resolution_label(), resolutions, "renderQueue.setRenderSettings"),
        ];
        let start_y = output_y + 56.0;
        let column_width = (content_width - (columns - 1) as f32 * 12.0) / columns as f32;
        for (i, (key, label, shown, options, command)) in fields.into_iter().enumerate() {
            let r = Rect::from_min_size(
                pos2(content_x + (i % columns) as f32 * (column_width + 12.0), start_y + (i / columns) as f32 * 48.0),
                vec2(column_width, 43.0),
            );
            if let Some(mut params) = choice(app, ui, r, it, key, label, shown, &options, !rendering && (key != "format" || !available.is_empty())) {
                if let Some(form) = params.get("form").and_then(Value::as_str) {
                    forms.push((it.clone(), form.into()));
                } else {
                    params["item"] = json!(it.id);
                    actions.push((command, params));
                }
            }
        }
        let advanced_y = start_y + settings_height;
        for (i, (key, label, shown, options, command)) in [
            (
                "renderSettings",
                "Render settings / templates",
                it.settings.name.as_str(),
                settings_menu(it, &app.session.project.render_templates),
                "renderQueue.setRenderSettings",
            ),
            (
                "outputModule",
                "Encoding / output templates",
                if it.output.name.is_empty() { "Custom encoding…" } else { it.output.name.as_str() },
                output_menu(it, &available, &app.session.project.render_templates),
                "renderQueue.setOutputModule",
            ),
            (
                "log",
                "Render log",
                it.log.label(),
                RenderLog::ALL.iter().map(|log| (mark(it.log == *log, log.label()), json!({"log":log.label()}))).collect(),
                "renderQueue.setLog",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let r = Rect::from_min_size(
                pos2(content_x + (i % columns) as f32 * (column_width + 12.0), advanced_y + (i / columns) as f32 * 48.0),
                vec2(column_width, 43.0),
            );
            if let Some(mut params) = choice(app, ui, r, it, key, label, shown, &options, !rendering) {
                if let Some(form) = params.get("form").and_then(Value::as_str) {
                    forms.push((it.clone(), form.into()));
                } else {
                    params["item"] = json!(it.id);
                    actions.push((command, params));
                }
            }
        }
        let details_y = advanced_y + advanced_height + 14.0;
        let info = app
            .session
            .project
            .comp(it.comp)
            .map(|c| {
                let (w, h) = it.settings.output_size(c);
                let (a, b) = it.settings.span(c);
                format!(
                    "{w} × {h} · {:.3} fps · {:.2} s · {} frames",
                    it.settings.rate(c).as_f64(),
                    (b.seconds() - a.seconds()).max(0.0),
                    it.settings.frame_count(c)
                )
            })
            .unwrap_or_else(|| "Composition missing".into());
        widgets::text_fit(&lp, pos2(content_x, details_y), Align2::LEFT_CENTER, &info, Tokens::ui(11.0), content_width, t.text_dim);
        let log_rect = Rect::from_min_size(pos2(content_x, details_y - 10.0), vec2(content_width, 20.0));
        ui.interact(log_rect, egui::Id::new(("rq-details", it.id)), Sense::hover()).on_hover_text(format!(
            "{info}\nStarted: {}\nRender time: {}\nLog: {}",
            it.started.map(utc_stamp).unwrap_or_else(|| "Not started".into()),
            it.render_time.map(hms).unwrap_or_else(|| "—".into()),
            it.log.label()
        ));
        if let Some(error) = error {
            widgets::text_fit(&lp, pos2(content_x, details_y + 26.0), Align2::LEFT_CENTER, &error, Tokens::ui(11.5), content_width, t.warning);
            ui.interact(Rect::from_min_size(pos2(content_x, details_y + 16.0), vec2(content_width, 24.0)), egui::Id::new(("rq-error", it.id)), Sense::hover())
                .on_hover_text(error);
        }
    }
    if queue.is_empty() {
        widgets::text_fit(
            &lp,
            pos2(list.min.x + 12.0, list.min.y + 22.0),
            Align2::LEFT_CENTER,
            "No renders yet. Add a composition above.",
            Tokens::ui(12.0),
            (list.width() - 24.0).max(0.0),
            t.text_dim,
        );
    }
    ui.set_clip_rect(panel_clip);
    let content_height = y + scroll.offset - list.min.y;
    scroll.end(ui, &mut app.auto, "renderQueue.scroll", content_height, &t);
    ui.data_mut(|d| match editing.as_ref() {
        Some(edit) => {
            d.insert_temp(editing_id, edit.clone());
        }
        None => d.remove::<(u64, String)>(editing_id),
    });
    if app.ui.focused == crate::dock::PanelKind::RenderQueue
        && editing.is_none()
        && !rendering
        && app.dialog.is_none()
        && !ui.ctx().egui_wants_keyboard_input()
        && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        && let Some(id) = selected
    {
        actions.push(("renderQueue.remove", json!({"item":id})));
        selected = None;
    }
    p.rect_filled(foot, 0.0, t.panel_bg);
    p.line_segment([foot.left_top(), foot.right_top()], Stroke::new(1.0, t.separator));
    let notify = app.session.project.render_prefs.notify;
    let nb = Rect::from_min_size(foot.min + vec2(12.0, 5.0), vec2(18.0, 18.0));
    let full = Rect::from_min_size(nb.min, vec2(112.0, 18.0));
    let square = widgets::checkbox(ui, nb, notify, &t, egui::Id::new("rq-notify"));
    let label = ui.interact(Rect::from_min_max(pos2(nb.max.x, nb.min.y), full.max), egui::Id::new("rq-notify-label"), Sense::click());
    if square.clicked() || label.clicked() {
        actions.push(("renderQueue.setNotify", json!({"notify":!notify})));
    }
    widgets::text_fit(&p, pos2(nb.max.x + 6.0, nb.center().y), Align2::LEFT_CENTER, "Notify on finish", Tokens::ui(11.0), 88.0, t.text_dim);
    app.auto.add("renderQueue.notify", full, "Notify on finish");
    let done = queue.iter().filter(|it| it.status == RenderStatus::Done).count();
    let total = progress.as_ref().filter(|_| rendering).map(|pr| pr.elapsed).unwrap_or_else(|| queue.iter().filter_map(|it| it.render_time).sum());
    widgets::text_fit(
        &p,
        pos2(foot.max.x - 12.0, foot.center().y),
        Align2::RIGHT_CENTER,
        &format!("{done} / {} completed · {}", queue.len(), hms(total)),
        Tokens::ui(11.0),
        (foot.width() - 148.0).max(0.0),
        t.text_dim,
    );
    open.retain(|id| queue.iter().any(|it| it.id == *id));
    ui.data_mut(|d| d.insert_temp(state_id, (open, selected)));
    for (item, form) in forms {
        open_menu_form(app, &item, &form);
    }
    let ctx = ui.ctx().clone();
    for (id, params) in actions {
        if let Err(error) = crate::menus::invoke(app, &ctx, id, params) {
            app.ui.status = error;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn stamps() {
        assert_eq!(super::utc_stamp(0), "1970-01-01 00:00:00");
        assert_eq!(super::utc_stamp(1_700_000_000), "2023-11-14 22:13:20");
        assert_eq!(super::hms(3725.4), "1:02:05");
    }
}
