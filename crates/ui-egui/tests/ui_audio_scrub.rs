//! Audio scrubbing: Ctrl/Cmd-dragging the current time plays one frame of the comp's mix at each
//! new frame (each snippet replacing what is still queued); holding still repeats nothing; silent
//! comps open no output; an idle scrub closes it. Audio previews keep playing when video renders slowly.

use std::sync::{Arc, Mutex};

use effectcraft_engine::Session;
use effectcraft_engine::project::ItemId;
use effectcraft_engine::time::Tick;
use effectcraft_ui_egui::EffectcraftApp;
use effectcraft_ui_egui::audio::{AudioDevice, AudioFeed};
use egui::{Event, Modifiers, pos2};
use egui_kittest::Harness;
use serde_json::json;

/// An output that records the feed it is given and whether it was stopped.
#[derive(Clone, Default)]
struct Fake(Arc<Mutex<(Option<Arc<AudioFeed>>, bool)>>);

impl AudioDevice for Fake {
    fn sample_rate(&self) -> u32 {
        48000
    }
    fn start(&mut self, feed: Arc<AudioFeed>) -> Result<(), String> {
        self.0.lock().unwrap().0 = Some(feed);
        Ok(())
    }
    fn stop(&mut self) {
        self.0.lock().unwrap().1 = true;
    }
}

/// A 2 s, 30 fps comp whose solid sounds a tone (Audio switch on).
fn setup(audible: bool, fake: &Fake) -> (EffectcraftApp, ItemId) {
    setup_long(audible, fake, 2.0)
}

/// A `duration`-second, 30 fps comp whose solid sounds a tone (Audio switch on).
fn setup_long(audible: bool, fake: &Fake, duration: f64) -> (EffectcraftApp, ItemId) {
    let mut s = Session::default();
    s.execute("comp.new", json!({"name": "A", "width": 64, "height": 36, "frameRate": 30, "duration": duration})).unwrap();
    let cid = s.active_comp_id().unwrap();
    let l = s.execute("layer.newSolid", json!({"color": "#808080"})).unwrap()["layer"].as_u64().unwrap();
    if audible {
        s.execute("effect.apply", json!({"layers": [l], "effect": "ec.audio.tone"})).unwrap();
        s.execute("layer.setSwitch", json!({"layers": [l], "switch": "audio", "value": true})).unwrap();
    }
    let mut app = EffectcraftApp::new(s);
    let f = fake.clone();
    app.hooks.audio_device = Some(Box::new(move |_| Some(Box::new(f.clone()) as Box<dyn AudioDevice>)));
    (app, cid)
}

#[test]
fn scrubbing_plays_one_frame_per_new_frame() {
    let fake = Fake::default();
    let (mut app, cid) = setup(true, &fake);
    let t = |s: f64| Tick::from_seconds_f64(s);
    app.scrub_audio(cid, t(0.5), 0.0);
    let feed = fake.0.lock().unwrap().0.clone().expect("the output opened");
    // One 30 fps frame at 48 kHz, and not silence.
    assert_eq!(feed.queued_frames(), 1600);
    let mut out = vec![0.0f32; 3200];
    feed.pull(&mut out);
    assert!(out.iter().any(|v| v.abs() > 0.01), "the tone is there");
    // Faded in: the first sample is silent.
    assert_eq!(out[0], 0.0);
    // The same frame again plays nothing; the next frame replaces what is left.
    app.scrub_audio(cid, t(0.51), 0.1);
    assert_eq!(feed.queued_frames(), 0);
    feed.push(&[0.0; 1000]);
    app.scrub_audio(cid, t(0.6), 0.2);
    assert_eq!(feed.queued_frames(), 1600, "replaced, not piled up");
    // A silent comp opens nothing.
    let silent = Fake::default();
    let (mut quiet, qid) = setup(false, &silent);
    quiet.scrub_audio(qid, t(0.5), 0.0);
    assert!(silent.0.lock().unwrap().0.is_none() && quiet.scrub.is_none());
}

#[test]
fn cmd_dragging_the_current_time_scrubs_and_an_idle_scrub_closes() {
    let fake = Fake::default();
    let (app, _) = setup(true, &fake);
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|_| app);
    h.run_steps(3);
    let cti = h.state().auto.find("timeline.cti").expect("current time indicator").clone();
    let from = pos2(cti.rect[0] + cti.rect[2] / 2.0, cti.rect[1] + cti.rect[3] / 2.0);
    let cmd = Modifiers::COMMAND;
    h.event(Event::ModifiersChanged(cmd));
    h.event(Event::PointerMoved(from));
    h.step();
    h.event(Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: cmd });
    h.step();
    for k in 1..=4 {
        h.event(Event::PointerMoved(from + egui::vec2(40.0 * k as f32, 0.0)));
        h.step();
    }
    assert!(h.state().scrub.is_some(), "Cmd-drag opened the scrub output");
    assert!(fake.0.lock().unwrap().0.as_ref().is_some_and(|f| f.queued_frames() > 0));
    h.event(Event::PointerButton { pos: from + egui::vec2(160.0, 0.0), button: egui::PointerButton::Primary, pressed: false, modifiers: cmd });
    h.step();
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.step();
    // An idle second closes the output.
    for _ in 0..90 {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(15));
        if h.state().scrub.is_none() {
            break;
        }
    }
    assert!(h.state().scrub.is_none() && fake.0.lock().unwrap().1, "closed and stopped");
}

/// A slow composition must not remain silent until its video cache is ready.
#[test]
fn preview_with_audio_starts_without_cached_video_and_survives_stalls() {
    let fake = Fake::default();
    let (app, cid) = setup_long(true, &fake, 30.0);
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|_| app);
    h.run_steps(2);
    h.state().frames.set_budget(0);
    h.state().frames.clear();
    let now = h.ctx.input(|i| i.time);
    h.state_mut().play(now);
    assert!(h.state().audio.is_some() && !h.state().playback.audio_held, "sound starts without a video cache");
    let feed = fake.0.lock().unwrap().0.clone().unwrap();
    let mut samples = vec![0.0; 3200];
    feed.pull(&mut samples);
    assert!(samples.iter().any(|v| v.abs() > 0.01), "the real mix reaches the output");
    h.step();
    assert!(h.state().audio.is_some() && h.state().playback.playing && !h.state().playback.audio_held, "a missing video frame does not mute sound");
    assert!(Arc::ptr_eq(&feed, &fake.0.lock().unwrap().0.clone().unwrap()), "the same audio stream keeps playing");
    let app = h.state();
    let audio = app.audio.as_ref().unwrap();
    let frame = app.session.project.comp(cid).unwrap().frame_rate.frame_at(app.session.time());
    assert_eq!(frame, effectcraft_ui_egui::audio::frame_of_sample(audio.clock(), audio.rate, app.session.project.comp(cid).unwrap().frame_rate));
    assert!(frame > 0, "the CTI follows the consumed audio through a video stall");
    h.state_mut().stop();
    assert!(fake.0.lock().unwrap().1, "stop closes the device");
}

/// Explicit Cache Before Playback still waits for video before opening the output.
#[test]
fn cache_before_playback_still_waits_and_then_starts_audio() {
    let fake = Fake::default();
    let (mut app, _) = setup(true, &fake);
    app.session.execute("playback.settings.set", json!({"shortcut":"spacebar", "cacheBeforePlayback":true})).unwrap();
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|_| app);
    h.run_steps(2);
    let now = h.ctx.input(|i| i.time);
    h.state_mut().play(now);
    assert!(h.state().playback.caching && h.state().playback.audio_held && h.state().audio.is_none());
    for _ in 0..2000 {
        h.step();
        if h.state().audio.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(h.state().audio.is_some() && !h.state().playback.audio_held && !h.state().playback.caching);
}

#[test]
fn a_failed_audio_output_is_reported_and_stopped() {
    struct Fails(Fake);
    impl AudioDevice for Fails {
        fn sample_rate(&self) -> u32 {
            48000
        }
        fn start(&mut self, _: Arc<AudioFeed>) -> Result<(), String> {
            Err("test output unavailable".into())
        }
        fn stop(&mut self) {
            self.0.stop();
        }
    }
    let fake = Fake::default();
    let (mut app, _) = setup(true, &fake);
    let output = fake.clone();
    app.hooks.audio_device = Some(Box::new(move |_| Some(Box::new(Fails(output.clone())))));
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|_| app);
    h.run_steps(2);
    let now = h.ctx.input(|i| i.time);
    h.state_mut().play(now);
    h.step();
    assert!(h.state().audio.is_none());
    assert!(h.state().auto.find("toast").is_some_and(|element| element.label.contains("test output unavailable")));
    assert!(fake.0.lock().unwrap().1, "a failed start closes the output and feeder");
}
