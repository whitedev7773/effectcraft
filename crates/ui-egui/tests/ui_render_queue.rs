//! Render Queue panel input: choosing a Render Settings template from an item's menu (#117).

use effectcraft_engine::project::render_queue::{OutputFormat, RenderQuality, TimeSpan};
use effectcraft_engine::{ExportJob, ExportResult, Exporter, Session};
use effectcraft_ui_egui::EffectcraftApp;
use effectcraft_ui_egui::dock::PanelKind;
use egui::{Event, Modifiers, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct ReviewExporter {
    jobs: Mutex<Vec<(String, OutputFormat, RenderQuality, u64)>>,
}
impl Exporter for ReviewExporter {
    fn formats(&self) -> Vec<OutputFormat> {
        vec![OutputFormat::PngSequence, OutputFormat::Gif]
    }
    fn export(&self, job: &ExportJob, progress: &mut dyn FnMut(u64, u64) -> bool) -> Result<ExportResult, String> {
        let comp = job.project.comp(job.item.comp).ok_or("Missing composition")?;
        let frames = job.item.settings.frame_count(comp);
        self.jobs.lock().unwrap().push((job.path.into(), job.item.output.format, job.item.settings.quality, frames));
        if !progress(frames, frames) {
            return Err("cancelled".into());
        }
        Ok(ExportResult { path: job.path.into(), frames, ..Default::default() })
    }
}
fn review_app() -> EffectcraftApp {
    let mut s = Session::default();
    s.execute("comp.new", json!({"name":"A very long composition name for narrow panels","width":64,"height":64,"duration":2,"frameRate":24})).unwrap();
    s.execute("renderQueue.add", json!({"format":"png","output":"test-[#####].png"})).unwrap();
    s.exporter = Some(Arc::new(ReviewExporter::default()));
    EffectcraftApp::new(s)
}
fn click_id(h: &mut Harness<'_, EffectcraftApp>, id: &str) {
    let r = h.state().auto.find(id).unwrap().rect;
    click_at(h, pos2(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0));
}
fn queue_harness(width: f32) -> Harness<'static, EffectcraftApp> {
    let mut app = Some(review_app());
    let mut h = Harness::builder().with_size(egui::vec2(width, 900.0)).build_eframe(move |_| app.take().unwrap());
    h.state_mut().show_panel(PanelKind::RenderQueue);
    h.state_mut().ui.maximized = Some(PanelKind::RenderQueue);
    h.run_steps(4);
    h
}

#[test]
fn quick_settings_change_format_range_quality_and_resolution() {
    let mut h = queue_harness(1000.0);
    for (key, label) in [("format", "GIF"), ("timeSpan", "Entire composition"), ("quality", "Draft"), ("resolution", "Half")] {
        click_id(&mut h, &format!("renderQueue.item.1.{key}"));
        let rect = h.query_by_label_contains(label).unwrap().rect();
        click_at(&mut h, rect.center());
    }
    let item = &h.state().session.project.render_queue[0];
    assert_eq!(item.output.format, OutputFormat::Gif);
    assert!(item.output.output.ends_with(".gif"));
    assert_eq!(item.settings.time_span, TimeSpan::LengthOfComp);
    assert_eq!(item.settings.quality, RenderQuality::Draft);
    assert_eq!(item.settings.resolution, 0.5);
}

#[test]
fn narrow_queue_controls_fit_and_reflow_without_overlap() {
    for width in [320.0, 480.0, 1024.0] {
        let h = queue_harness(width);
        let panel = h.state().auto.find("panel.RenderQueue").unwrap().rect;
        let keys = ["outputPath", "outputTo", "browse", "format", "timeSpan", "quality", "resolution", "renderSettings", "outputModule"];
        let rects: Vec<_> = keys
            .iter()
            .map(|key| {
                let r = h.state().auto.find(&format!("renderQueue.item.1.{key}")).unwrap().rect;
                let r = egui::Rect::from_min_size(pos2(r[0], r[1]), egui::vec2(r[2], r[3]));
                assert!(r.width() > 0.0 && r.min.x >= panel[0] && r.max.x <= panel[0] + panel[2], "{key}: {r:?} at width {width}");
                r
            })
            .collect();
        for (i, a) in rects.iter().enumerate() {
            for b in rects.iter().skip(i + 1) {
                assert!(!a.intersects(*b), "overlap at width {width}: {a:?} / {b:?}");
            }
        }
    }
}

#[test]
fn inline_path_commits_on_click_away_and_escape_cancels() {
    let mut h = queue_harness(1000.0);
    let original = h.state().session.project.render_queue[0].output.output.clone();
    let path = h.state().auto.find("renderQueue.item.1.outputPath").unwrap().rect;
    let at = pos2(path[0] + 20.0, path[1] + path[3] / 2.0);
    click_with(&mut h, at, Modifiers::ALT);
    h.input_mut().events.push(Event::Text("changed-[#####].png".into()));
    h.run_steps(2);
    click_id(&mut h, "renderQueue.item.1.quality");
    assert_eq!(h.state().session.project.render_queue[0].output.output, "changed-[#####].png");
    // Close the menu before opening another inline editor.
    h.ctx.data_mut(|d| d.insert_temp(egui::Id::new(("rq-choice", 1_u64, "quality")).with("open"), false));
    click_with(&mut h, at, Modifiers::ALT);
    h.input_mut().events.push(Event::Text(original));
    h.input_mut().events.push(Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
    assert_eq!(h.state().session.project.render_queue[0].output.output, "changed-[#####].png");
}

#[test]
fn add_remove_and_render_use_the_engine_queue() {
    let mut h = queue_harness(1000.0);
    let exporter = Arc::new(ReviewExporter::default());
    h.state_mut().session.exporter = Some(exporter.clone());
    click_id(&mut h, "renderQueue.add");
    assert_eq!(h.state().session.project.render_queue.len(), 2);
    let id = h.state().session.project.render_queue[1].id;
    click_id(&mut h, &format!("renderQueue.item.{id}.remove"));
    assert_eq!(h.state().session.project.render_queue.len(), 1);
    click_id(&mut h, "renderQueue.render");
    for _ in 0..20 {
        h.state_mut().session.poll_render();
        h.run_steps(2);
        if !h.state().session.is_rendering() {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(h.state().session.project.render_queue[0].status, effectcraft_engine::project::render_queue::RenderStatus::Done);
    let jobs = exporter.jobs.lock().unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].1, OutputFormat::PngSequence);
    assert_eq!(jobs[0].3, 48);
    drop(jobs);
    click_id(&mut h, "renderQueue.item.1.requeue");
    assert_eq!(h.state().session.project.render_queue[0].status, effectcraft_engine::project::render_queue::RenderStatus::Queued);
}

#[test]
fn missing_output_is_explained_and_cannot_start_a_render() {
    let mut h = queue_harness(1000.0);
    let project = Arc::make_mut(&mut h.state_mut().session.project);
    project.render_queue[0].output.output.clear();
    project.render_queue[0].status = effectcraft_engine::project::render_queue::RenderStatus::NeedsOutput;
    h.run_steps(3);
    click_id(&mut h, "renderQueue.render");
    assert!(!h.state().session.is_rendering());
    assert!(h.state().auto.find("renderQueue.progress").unwrap().label.contains("Choose an output file"));
}

#[test]
fn unsupported_format_disables_render_and_explains_why() {
    let mut h = queue_harness(1000.0);
    h.state_mut().session.execute("renderQueue.setOutputModule", json!({"item":1,"format":"h264"})).unwrap();
    h.run_steps(3);
    click_id(&mut h, "renderQueue.render");
    assert!(!h.state().session.is_rendering());
    assert!(h.state().auto.find("renderQueue.progress").unwrap().label.contains("unavailable"));
}

fn click_at(h: &mut Harness<'_, EffectcraftApp>, p: Pos2) {
    h.input_mut().events.push(Event::PointerMoved(p));
    h.input_mut().events.push(Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    h.input_mut().events.push(Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(2);
}

/// The Render Settings menu is taller than the room below the item, so it is moved up to fit
/// the window; its entries must still take the click instead of the menu closing on the press.
#[test]
fn render_settings_template_applies_from_a_menu_moved_to_fit_the_window() {
    let mut s = Session::default();
    s.execute("file.openDemoProject", json!({})).unwrap();
    s.execute("renderQueue.add", json!({})).unwrap();
    let id = s.project.render_queue[0].id;
    assert_eq!(s.project.render_queue[0].settings.name, "Best Settings");
    let mut app = Some(EffectcraftApp::new(s));
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).build_eframe(|_| app.take().expect("app"));
    // Twirl the item open, as clicking its twirl does.
    h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("rq-ui"), (vec![id], None::<u64>)));
    h.state_mut().show_panel(PanelKind::RenderQueue);
    h.state_mut().ui.maximized = Some(PanelKind::RenderQueue);
    h.run_steps(4);
    let dd = h.state().auto.find(&format!("renderQueue.item.{id}.renderSettings")).expect("Render Settings menu").clone();
    let dd_center = pos2(dd.rect[0] + dd.rect[2] / 2.0, dd.rect[1] + dd.rect[3] / 2.0);
    click_at(&mut h, dd_center);
    let entry = h.query_by_label_contains("Draft Settings").expect("the Draft Settings entry").rect();
    assert!(entry.max.y < dd_center.y, "the menu was moved up to fit the window (entry {entry:?}, menu button at {dd_center:?})");
    click_at(&mut h, entry.center());
    let it = &h.state().session.project.render_queue[0];
    assert_eq!(it.settings.name, "Draft Settings");
    assert_eq!(it.settings.quality, RenderQuality::Draft);
    assert_eq!(it.settings.resolution, 0.5);
    assert!(h.query_by_label_contains("Quality: Draft").is_none(), "the menu closed after the choice");
}

fn click_with(h: &mut Harness<'_, EffectcraftApp>, p: Pos2, modifiers: Modifiers) {
    h.input_mut().events.push(Event::ModifiersChanged(modifiers));
    h.input_mut().events.push(Event::PointerMoved(p));
    h.input_mut().events.push(Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers });
    h.step();
    h.input_mut().events.push(Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers });
    h.step();
    h.input_mut().events.push(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(2);
}

/// Issue #192: clicking the Output To file name opens a save dialog (at the current output, for
/// the module's extension) and the chosen file becomes the output, as After Effects' Output
/// Movie To; Alt-click still edits the name template in place.
#[test]
fn clicking_the_output_file_name_opens_the_save_dialog() {
    let mut s = Session::default();
    s.execute("file.openDemoProject", json!({})).unwrap();
    s.execute("renderQueue.add", json!({})).unwrap();
    let id = s.project.render_queue[0].id;
    let expected_default = s.resolve_output(&s.project.render_queue[0]).unwrap();
    let ext = s.project.render_queue[0].output.format.extension();
    let mut app = EffectcraftApp::new(s);
    let asked: Rc<RefCell<Vec<(String, String)>>> = Rc::default();
    let log = asked.clone();
    let chosen = std::env::temp_dir().join(format!("chosen.{ext}")).to_string_lossy().to_string();
    let answer = chosen.clone();
    app.hooks.pick_save_file = Some(Box::new(move |name: &str, ext: &str| {
        log.borrow_mut().push((name.to_string(), ext.to_string()));
        Some(answer.clone())
    }));
    let mut app = Some(app);
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).build_eframe(|_| app.take().expect("app"));
    h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("rq-ui"), (vec![id], None::<u64>)));
    h.state_mut().show_panel(PanelKind::RenderQueue);
    h.state_mut().ui.maximized = Some(PanelKind::RenderQueue);
    h.run_steps(4);
    let path = h.state().auto.find(&format!("renderQueue.item.{id}.outputPath")).expect("the output file name").clone();
    let at = pos2(path.rect[0] + 20.0, path.rect[1] + path.rect[3] / 2.0);
    click_with(&mut h, at, Modifiers::NONE);
    assert_eq!(*asked.borrow(), [(expected_default, ext.to_string())], "the dialog opened at the current output");
    assert_eq!(h.state().session.project.render_queue[0].output.output, chosen);
    let editing = |h: &Harness<'_, EffectcraftApp>| h.ctx.data(|d| d.get_temp::<(u64, String)>(egui::Id::new("rq-edit-output")));
    assert_eq!(editing(&h), None, "no inline editor");
    // Alt-click edits the template text instead.
    click_with(&mut h, at, Modifiers::ALT);
    assert_eq!(asked.borrow().len(), 1, "Alt-click does not open the dialog");
    assert_eq!(editing(&h).map(|(item, _)| item), Some(id), "the template editor is open");
}
