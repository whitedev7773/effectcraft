//! Render the desktop splash headlessly for visual review (path defaults to target/splash.png).

use effectcraft_ui_egui::{splash, theme};
use egui_kittest::Harness;

fn main() -> Result<(), String> {
    let path = std::env::args().nth(1).unwrap_or_else(|| "target/splash.png".into());
    let mut harness = Harness::builder().with_size(splash::SIZE).with_pixels_per_point(2.0).wgpu().build_ui(|ui| {
        splash::paint(ui, "Loading settings, plug-ins and project…", 0.6);
    });
    theme::install(&harness.ctx, &theme::Tokens::for_kind(Default::default()));
    harness.step();
    harness.step();
    harness.render()?.save(&path).map_err(|e| format!("{path}: {e}"))?;
    Ok(())
}
