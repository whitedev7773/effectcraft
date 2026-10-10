//! Gravity UI icons (MIT); the original SVGs and attribution live in assets/icons/gravity.

use super::Icon;
use egui::{Color32, ColorImage, Painter, Rect, TextureHandle, pos2, vec2};

pub(super) const ASSETS: &[(Icon, &str, &[u8])] = &[
    (Icon::Home, "house", include_bytes!("../../../assets/icons/gravity/house.svg")),
    (Icon::Hand, "hand", include_bytes!("../../../assets/icons/gravity/hand.svg")),
    (Icon::Zoom, "magnifier", include_bytes!("../../../assets/icons/gravity/magnifier.svg")),
    (Icon::Rotate, "arrow-rotate-left", include_bytes!("../../../assets/icons/gravity/arrow-rotate-left.svg")),
    (Icon::Rectangle, "square", include_bytes!("../../../assets/icons/gravity/square.svg")),
    (Icon::Ellipse, "circle", include_bytes!("../../../assets/icons/gravity/circle.svg")),
    (Icon::Star, "star", include_bytes!("../../../assets/icons/gravity/star.svg")),
    (Icon::Type, "font", include_bytes!("../../../assets/icons/gravity/font.svg")),
    (Icon::Brush, "paintbrush", include_bytes!("../../../assets/icons/gravity/paintbrush.svg")),
    (Icon::Clone, "copy", include_bytes!("../../../assets/icons/gravity/copy.svg")),
    (Icon::Eraser, "eraser", include_bytes!("../../../assets/icons/gravity/eraser.svg")),
    (Icon::Eye, "eye", include_bytes!("../../../assets/icons/gravity/eye.svg")),
    (Icon::Speaker, "volume", include_bytes!("../../../assets/icons/gravity/volume.svg")),
    (Icon::Solo, "circle-fill", include_bytes!("../../../assets/icons/gravity/circle-fill.svg")),
    (Icon::Lock, "lock", include_bytes!("../../../assets/icons/gravity/lock.svg")),
    (Icon::Shy, "eye-closed", include_bytes!("../../../assets/icons/gravity/eye-closed.svg")),
    (Icon::Collapse, "arrows-opposite-to-dots", include_bytes!("../../../assets/icons/gravity/arrows-opposite-to-dots.svg")),
    (Icon::Fx, "function", include_bytes!("../../../assets/icons/gravity/function.svg")),
    (Icon::Cube, "cube", include_bytes!("../../../assets/icons/gravity/cube.svg")),
    (Icon::Graph, "chart-line", include_bytes!("../../../assets/icons/gravity/chart-line.svg")),
    (Icon::Stopwatch, "stopwatch", include_bytes!("../../../assets/icons/gravity/stopwatch.svg")),
    (Icon::Keyframe, "diamond-fill", include_bytes!("../../../assets/icons/gravity/diamond-fill.svg")),
    (Icon::Flowchart, "nodes-down", include_bytes!("../../../assets/icons/gravity/nodes-down.svg")),
    (Icon::Folder, "folder", include_bytes!("../../../assets/icons/gravity/folder.svg")),
    (Icon::Comp, "filmstrip", include_bytes!("../../../assets/icons/gravity/filmstrip.svg")),
    (Icon::Footage, "video", include_bytes!("../../../assets/icons/gravity/video.svg")),
    (Icon::Image, "picture", include_bytes!("../../../assets/icons/gravity/picture.svg")),
    (Icon::Audio, "music-note", include_bytes!("../../../assets/icons/gravity/music-note.svg")),
    (Icon::Solid, "square-fill", include_bytes!("../../../assets/icons/gravity/square-fill.svg")),
    (Icon::TextLayer, "font", include_bytes!("../../../assets/icons/gravity/font.svg")),
    (Icon::ShapeLayer, "shapes-3", include_bytes!("../../../assets/icons/gravity/shapes-3.svg")),
    (Icon::Null, "square-dashed", include_bytes!("../../../assets/icons/gravity/square-dashed.svg")),
    (Icon::Camera, "camera", include_bytes!("../../../assets/icons/gravity/camera.svg")),
    (Icon::Light, "bulb", include_bytes!("../../../assets/icons/gravity/bulb.svg")),
    (Icon::Search, "magnifier", include_bytes!("../../../assets/icons/gravity/magnifier.svg")),
    (Icon::Trash, "trash-bin", include_bytes!("../../../assets/icons/gravity/trash-bin.svg")),
    (Icon::NewFolder, "folder-plus", include_bytes!("../../../assets/icons/gravity/folder-plus.svg")),
    (Icon::NewComp, "square-plus", include_bytes!("../../../assets/icons/gravity/square-plus.svg")),
    (Icon::Gear, "gear", include_bytes!("../../../assets/icons/gravity/gear.svg")),
    (Icon::Hamburger, "bars", include_bytes!("../../../assets/icons/gravity/bars.svg")),
    (Icon::ChevronDown, "chevron-down", include_bytes!("../../../assets/icons/gravity/chevron-down.svg")),
    (Icon::ChevronRight, "chevron-right", include_bytes!("../../../assets/icons/gravity/chevron-right.svg")),
    (Icon::ChevronLeft, "chevron-left", include_bytes!("../../../assets/icons/gravity/chevron-left.svg")),
    (Icon::Play, "play-fill", include_bytes!("../../../assets/icons/gravity/play-fill.svg")),
    (Icon::Pause, "pause-fill", include_bytes!("../../../assets/icons/gravity/pause-fill.svg")),
    (Icon::StepBack, "backward-step", include_bytes!("../../../assets/icons/gravity/backward-step.svg")),
    (Icon::StepFwd, "forward-step", include_bytes!("../../../assets/icons/gravity/forward-step.svg")),
    (Icon::First, "arrow-left-to-line", include_bytes!("../../../assets/icons/gravity/arrow-left-to-line.svg")),
    (Icon::Last, "arrow-right-to-line", include_bytes!("../../../assets/icons/gravity/arrow-right-to-line.svg")),
    (Icon::Loop, "arrows-rotate-right", include_bytes!("../../../assets/icons/gravity/arrows-rotate-right.svg")),
    (Icon::Snapshot, "camera", include_bytes!("../../../assets/icons/gravity/camera.svg")),
    (Icon::ShowSnapshot, "picture", include_bytes!("../../../assets/icons/gravity/picture.svg")),
    (Icon::Grid, "layout-cells", include_bytes!("../../../assets/icons/gravity/layout-cells.svg")),
    (Icon::Region, "crop", include_bytes!("../../../assets/icons/gravity/crop.svg")),
    (Icon::ExtendedViewer, "arrows-expand", include_bytes!("../../../assets/icons/gravity/arrows-expand.svg")),
    (Icon::Plus, "plus", include_bytes!("../../../assets/icons/gravity/plus.svg")),
    (Icon::Minus, "minus", include_bytes!("../../../assets/icons/gravity/minus.svg")),
    (Icon::Close, "xmark", include_bytes!("../../../assets/icons/gravity/xmark.svg")),
    (Icon::Link, "link", include_bytes!("../../../assets/icons/gravity/link.svg")),
    (Icon::Fullscreen, "arrows-expand", include_bytes!("../../../assets/icons/gravity/arrows-expand.svg")),
    (Icon::Channel, "circles-intersection", include_bytes!("../../../assets/icons/gravity/circles-intersection.svg")),
    (Icon::Globe, "globe", include_bytes!("../../../assets/icons/gravity/globe.svg")),
    (Icon::Code, "code", include_bytes!("../../../assets/icons/gravity/code.svg")),
    (Icon::Sparkle, "sparkles", include_bytes!("../../../assets/icons/gravity/sparkles.svg")),
];

/// Four fixed raster sizes bound cache growth even while a window moves between DPI scales.
/// Theme colour is applied by the painter, so hover/selection changes reuse the same texture.
fn raster_side(logical_size: f32, pixels_per_point: f32) -> Option<u32> {
    if !logical_size.is_finite() || logical_size <= 0.0 || !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
        return None;
    }
    let pixels = logical_size * pixels_per_point;
    Some(if pixels <= 16.0 {
        16
    } else if pixels <= 32.0 {
        32
    } else if pixels <= 64.0 {
        64
    } else {
        128
    })
}

fn raster_icon(bytes: &[u8], side: u32) -> Result<ColorImage, String> {
    if ![16, 32, 64, 128].contains(&side) {
        return Err("unsupported icon raster size".into());
    }
    let doc = effectcraft_svg::parse(bytes).map_err(|e| format!("parsing Gravity icon: {e}"))?;
    // Bundled Gravity SVGs use a 16×16 viewport. Reject unexpected dimensions before rendering.
    if doc.width != 16.0 || doc.height != 16.0 || !doc.skipped.is_empty() {
        return Err("Gravity icon must have a supported 16×16 viewport".into());
    }
    let image = effectcraft_svg::rasterize(&doc, side, side, f64::from(side) / 16.0);
    let pixels = image
        .data
        .iter()
        .map(|pixel| {
            let alpha = pixel.get(3).copied().filter(|a| a.is_finite()).unwrap_or(0.0);
            Color32::from_white_alpha((alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
        })
        .collect();
    Ok(ColorImage::new([side as usize, side as usize], pixels))
}

/// Returns false when no matching icon exists or decoding failed: use the original drawing.
pub(super) fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) -> bool {
    let Some((_, name, bytes)) = ASSETS.iter().find(|(candidate, _, _)| *candidate == icon) else {
        return false;
    };
    let size = rect.width().min(rect.height());
    let ctx = painter.ctx();
    let Some(side) = raster_side(size, ctx.pixels_per_point()) else {
        return false;
    };
    let id = egui::Id::new(("gravity-icon", name, side));
    let texture = ctx.data(|d| d.get_temp::<Option<TextureHandle>>(id)).unwrap_or_else(|| {
        let texture = match raster_icon(bytes, side) {
            Ok(image) => Some(ctx.load_texture(format!("gravity-{name}-{side}"), image, egui::TextureOptions::LINEAR)),
            Err(error) => {
                log::warn!("Gravity icon {name}: {error}; using the original drawing");
                None
            }
        };
        ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
        texture
    });
    let Some(texture) = texture else {
        return false;
    };
    painter.image(texture.id(), Rect::from_center_size(rect.center(), vec2(size, size)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), color);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_gravity_icons_render_visible_coverage() {
        for (_, name, bytes) in ASSETS {
            let image = raster_icon(bytes, 32).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(image.size, [32, 32], "{name}");
            assert_eq!(image.pixels.len(), 32 * 32, "{name}");
            assert!(image.pixels.iter().any(|p| p.a() > 0), "blank icon: {name}");
            assert!(image.pixels.iter().any(|p| p.a() == 0), "opaque background: {name}");
        }
    }

    #[test]
    fn invalid_icon_data_and_sizes_return_errors() {
        assert!(raster_icon(b"invalid", 32).is_err());
        assert!(raster_icon(br#"<svg width="0" height="0"/>"#, 32).is_err());
        assert!(raster_icon(br#"<svg width="16" height="16"><text>?</text></svg>"#, 32).is_err());
        assert!(raster_icon(b"", u32::MAX).is_err());
        for size in [f32::NAN, f32::INFINITY, -1.0, 0.0] {
            assert_eq!(raster_side(size, 1.0), None);
            assert_eq!(raster_side(16.0, size), None);
        }
        assert_eq!(raster_side(f32::MAX, f32::MAX), Some(128));
    }
}
