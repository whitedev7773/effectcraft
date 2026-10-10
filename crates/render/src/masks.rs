//! Layer masks: path coverage with expansion, feather, opacity, invert, combined by mode.

use effectcraft_effects::Buf;
use effectcraft_geom::{Mat3, vec2};
use effectcraft_keyframe::Value;
use effectcraft_path::{FillRule, StrokeStyle};
use effectcraft_project::{FeatherFalloff, GroupKind, Layer, MaskMode, MaskMotionBlur};
use effectcraft_raster::{Mask, box_blur, gaussian_blur};
use rayon::prelude::*;
use std::sync::Arc;

use crate::eval::EvalCtx;

/// What mask motion blur may do in a render (from the renderer's motion blur gate).
#[derive(Clone, Copy, Debug, Default)]
pub struct MaskBlur {
    /// Motion blur is on in this render: the render options, the comp's switch and the Motion
    /// Blur switches of the precomp layers above (when switches affect nested comps).
    pub allowed: bool,
    /// The layer's own Motion Blur switch, as those precomp layers limit it (Same As Layer).
    pub layer: bool,
    /// Sub-frame samples (the comp's Samples Per Frame; fewer in Draft).
    pub samples: usize,
}

/// Coverage of one mask in buffer pixels (before mode/opacity/invert). With mask motion blur
/// (Layer ▸ Mask ▸ Motion Blur: On, or Same As Layer with the layer's switch on) an animated
/// path is averaged over the comp's shutter.
fn coverage(
    ctx: &EvalCtx,
    layer: &Layer,
    g: &effectcraft_project::PropGroup,
    buf: &Buf,
    blur: MaskBlur,
    cache: Option<&crate::LayerCache>,
) -> Option<Arc<Mask>> {
    let mb = blur.allowed
        && match g.kind {
            GroupKind::Mask { motion_blur, .. } => match motion_blur {
                MaskMotionBlur::On => true,
                MaskMotionBlur::Off => false,
                MaskMotionBlur::SameAsLayer => blur.layer,
            },
            _ => false,
        };
    let animated = g.get("path").is_some_and(|p| p.keys.len() > 1 || p.has_expression());
    if mb && animated {
        let fd = ctx.comp.frame_duration().seconds();
        let (angle, phase) = (ctx.comp.shutter_angle / 360.0, ctx.comp.shutter_phase / 360.0);
        let mut acc: Option<Mask> = None;
        let n = blur.samples.max(2);
        let k = 1.0 / n as f32;
        for i in 0..n {
            let f = phase + angle * i as f64 / (n - 1) as f64;
            let sub = ctx.at(ctx.time + effectcraft_time::Tick::from_seconds_f64(f * fd));
            let Some(c) = coverage_at(&sub, layer, g, buf, cache) else { continue };
            match &mut acc {
                None => {
                    let mut c = (*c).clone();
                    c.data.iter_mut().for_each(|v| *v *= k);
                    acc = Some(c);
                }
                Some(a) => a.data.iter_mut().zip(&c.data).for_each(|(a, c)| *a += c * k),
            }
        }
        return acc.map(Arc::new);
    }
    coverage_at(ctx, layer, g, buf, cache)
}

fn coverage_at(ctx: &EvalCtx, layer: &Layer, g: &effectcraft_project::PropGroup, buf: &Buf, cache: Option<&crate::LayerCache>) -> Option<Arc<Mask>> {
    let Value::Path(sp) = ctx.group_value(layer, g, "path")? else { return None };
    let (w, h) = (buf.img.width, buf.img.height);
    let m = Mat3::translate(vec2(buf.offset[0], buf.offset[1])) * Mat3::scale(vec2(buf.scale, buf.scale));
    let exp = ctx.f(layer, g, "expansion", 0.0);
    let linear = matches!(g.kind, GroupKind::Mask { feather_falloff: FeatherFalloff::Linear, .. });
    let feather = ctx.v2(layer, g, "feather", [0.0; 2]);
    let key = cache.map(|_| crate::cache::mask_key(&sp, buf, exp, feather, linear));
    if let Some(cov) = key.and_then(|k| cache.and_then(|c| c.get_mask(k))) {
        return Some(cov);
    }
    let path = effectcraft_path::to_kurbo(&sp);
    // Variable-width feather (Mask Feather tool points): expansion and the per-point feather
    // are applied together from the signed distance to the path.
    let variable = !sp.feather.is_empty();
    let mut cov = if variable {
        effectcraft_path::feather::variable_feather_coverage(&sp, &m, w, h, exp, linear)
    } else {
        effectcraft_path::fill_coverage(std::slice::from_ref(&path), &m, w, h, FillRule::NonZero)
    };
    if exp.abs() > 0.01 && !variable {
        let ring = effectcraft_path::stroke_coverage(
            std::slice::from_ref(&path),
            &StrokeStyle { width: exp.abs() * 2.0, join: effectcraft_path::Join::Round, ..Default::default() },
            &m,
            w,
            h,
        );
        cov.data.par_iter_mut().zip(ring.data.par_iter()).for_each(|(c, r)| {
            *c = if exp > 0.0 { (*c + r - *c * r).min(1.0) } else { (*c * (1.0 - r)).max(0.0) };
        });
    }
    if feather[0] > 0.0 || feather[1] > 0.0 {
        let img = cov.to_image();
        let b = if linear {
            // One box pass: a straight ramp `feather` pixels wide across the edge.
            let r = |f: f64| ((f * buf.scale / 2.0).round() as usize).max(if f > 0.0 { 1 } else { 0 });
            box_blur(&img, r(feather[0]), r(feather[1]), 1, false)
        } else {
            gaussian_blur(&img, feather[0] * buf.scale / 2.0, feather[1] * buf.scale / 2.0, false)
        };
        cov = Mask::from_alpha(&b);
    }
    let cov = Arc::new(cov);
    if let (Some(cache), Some(key)) = (cache, key) {
        cache.insert_mask(key, cov.clone());
    }
    Some(cov)
}

/// The layer's enabled masks at the context time, flattened to polylines in layer space (for
/// effects that use masks as paths: Stroke, Scribble, Inner/Outer Key, Reshape…).
pub fn shapes(ctx: &EvalCtx, layer: &Layer) -> Vec<effectcraft_effects::MaskShape> {
    let Some(masks) = layer.masks() else { return Vec::new() };
    let mut out = Vec::new();
    for g in masks.groups().filter(|g| g.enabled) {
        let GroupKind::Mask { inverted, .. } = g.kind else { continue };
        let Some(Value::Path(sp)) = ctx.group_value(layer, g, "path") else { continue };
        let path = effectcraft_path::to_kurbo(&sp);
        let mut pts: Vec<[f64; 2]> = Vec::new();
        let mut closed = false;
        kurbo::flatten(path.iter(), 0.25, |el| match el {
            effectcraft_path::PathEl::MoveTo(p) | effectcraft_path::PathEl::LineTo(p) => {
                if pts.last() != Some(&[p.x, p.y]) {
                    pts.push([p.x, p.y]);
                }
            }
            effectcraft_path::PathEl::ClosePath => closed = true,
            _ => {}
        });
        if closed && pts.len() > 1 && pts.first() == pts.last() {
            pts.pop();
        }
        out.push(effectcraft_effects::MaskShape { name: g.name.clone(), points: pts, closed: closed || sp.closed, inverted });
    }
    out
}

/// Apply the layer's masks to its buffer.
/// Returns whether any mask was applied.
pub fn apply(ctx: &EvalCtx, layer: &Layer, buf: &mut Buf, blur: MaskBlur) -> bool {
    apply_cached(ctx, layer, buf, blur, None)
}

pub(crate) fn apply_cached(ctx: &EvalCtx, layer: &Layer, buf: &mut Buf, blur: MaskBlur, cache: Option<&crate::LayerCache>) -> bool {
    let Some(masks) = layer.masks() else { return false };
    let list: Vec<_> = masks.groups().filter(|g| g.enabled && matches!(g.kind, GroupKind::Mask { mode, .. } if mode != MaskMode::None)).collect();
    if list.is_empty() {
        return false;
    }
    // Masks may extend past the source: grow the buffer to cover them when feather/expansion pads.
    let n = (buf.img.width * buf.img.height) as usize;
    let first_sub = matches!(list[0].kind, GroupKind::Mask { mode: MaskMode::Subtract, .. });
    let mut acc = vec![if first_sub { 1.0f32 } else { 0.0 }; n];
    for g in list {
        let GroupKind::Mask { mode, inverted, .. } = g.kind else { continue };
        let Some(cov) = coverage(ctx, layer, g, buf, blur, cache) else { continue };
        let op = (ctx.f(layer, g, "opacity", 100.0) / 100.0) as f32;
        acc.par_iter_mut().zip(cov.data.par_iter()).for_each(|(a, &c)| {
            let c = if inverted { 1.0 - c } else { c } * op;
            *a = match mode {
                MaskMode::Add => *a + c - *a * c,
                MaskMode::Subtract => *a * (1.0 - c),
                MaskMode::Intersect => *a * c,
                MaskMode::Lighten => a.max(c),
                MaskMode::Darken => a.min(c),
                MaskMode::Difference => (*a - c).abs(),
                MaskMode::None => *a,
            };
        });
    }
    buf.img.mul_mask(&acc);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use effectcraft_color::Label;
    use effectcraft_keyframe::{Keyframe, ShapePath};
    use effectcraft_project::build::Ids;
    use effectcraft_project::{Comp, ItemKind, LayerSource, Project, build};
    use effectcraft_time::{FrameRate, Tick};

    #[test]
    fn coverage_reuses_exact_pixels_and_invalidates_every_geometry_input() {
        let mut p = Project::default();
        let comp = Comp::new(64, 48, FrameRate::FPS_30, Tick::from_seconds_f64(1.0));
        let mut layer = build::layer(&mut p, &comp, "masked", LayerSource::Shape, (64, 48), None);
        let mut next = p.next_id;
        let mut mask = build::mask(&mut Ids(&mut next), "mask", ShapePath::rect([30.0, 24.0], 32.0, 20.0), MaskMode::Add, [0; 3]);
        p.next_id = next;
        let cid = p.add_item("C", Label::Sandstone, None, ItemKind::Comp(comp.clone().into()));
        let ctx = EvalCtx::new(&p, cid, &comp, Tick::ZERO);
        let cache = crate::LayerCache::new(1 << 20);
        let mut buf = Buf { img: effectcraft_raster::Image::filled(64, 48, [1.0; 4]), offset: [0.0; 2], scale: 1.0 };
        mask.get_mut("feather").unwrap().value = Value::Vec2([6.0, 10.0]);
        let first = coverage_at(&ctx, &layer, &mask, &buf, Some(&cache)).unwrap();
        let again = coverage_at(&ctx, &layer, &mask, &buf, Some(&cache)).unwrap();
        assert!(Arc::ptr_eq(&first, &again));
        let check = |mask: &effectcraft_project::PropGroup, buf: &Buf| {
            let cached = coverage_at(&ctx, &layer, mask, buf, Some(&cache)).unwrap();
            let fresh = coverage_at(&ctx, &layer, mask, buf, None).unwrap();
            assert_eq!(cached.data, fresh.data);
            assert!(!Arc::ptr_eq(&first, &cached));
        };
        for (name, value) in
            [("expansion", Value::Scalar(3.0)), ("feather", Value::Vec2([8.0, 4.0])), ("path", Value::Path(ShapePath::rect([20.0, 24.0], 25.0, 18.0)))]
        {
            let mut edited = mask.clone();
            edited.get_mut(name).unwrap().value = value;
            check(&edited, &buf);
        }
        let mut linear = mask.clone();
        if let GroupKind::Mask { feather_falloff, .. } = &mut linear.kind {
            *feather_falloff = FeatherFalloff::Linear;
        }
        check(&linear, &buf);
        buf.offset = [3.0, -2.0];
        check(&mask, &buf);
        buf.scale = 0.5;
        check(&mask, &buf);
        buf.img = effectcraft_raster::Image::filled(32, 24, [1.0; 4]);
        check(&mask, &buf);

        // Mode/opacity/invert still evaluate at use time, without mutating shared coverage.
        mask.get_mut("opacity").unwrap().keys =
            vec![Keyframe::new(Tick::ZERO, Value::Scalar(20.0)), Keyframe::new(Tick::from_seconds_f64(1.0), Value::Scalar(80.0))];
        if let GroupKind::Mask { inverted, .. } = &mut mask.kind {
            *inverted = true;
        }
        layer.props.sub_mut("masks").unwrap().children.push(mask.into());
        for t in [Tick::ZERO, Tick::from_seconds_f64(1.0)] {
            let ctx = ctx.at(t);
            let mut cached = buf.clone();
            let mut fresh = buf.clone();
            apply_cached(&ctx, &layer, &mut cached, MaskBlur::default(), Some(&cache));
            apply(&ctx, &layer, &mut fresh, MaskBlur::default());
            assert_eq!(cached.img.data, fresh.img.data);
        }
    }

    #[test]
    fn animated_paths_are_keyed_by_evaluated_geometry() {
        let mut p = Project::default();
        let comp = Comp::new(64, 48, FrameRate::FPS_30, Tick::from_seconds_f64(1.0));
        let layer = build::layer(&mut p, &comp, "masked", LayerSource::Shape, (64, 48), None);
        let mut next = p.next_id;
        let mut mask = build::mask(&mut Ids(&mut next), "mask", ShapePath::default(), MaskMode::Add, [0; 3]);
        mask.get_mut("path").unwrap().keys = vec![
            Keyframe::new(Tick::ZERO, Value::Path(ShapePath::rect([15.0, 20.0], 16.0, 16.0))),
            Keyframe::new(Tick::from_seconds_f64(1.0), Value::Path(ShapePath::rect([45.0, 20.0], 16.0, 16.0))),
        ];
        let cid = p.add_item("C", Label::Sandstone, None, ItemKind::Comp(comp.clone().into()));
        let ctx = EvalCtx::new(&p, cid, &comp, Tick::ZERO);
        let cache = crate::LayerCache::new(1 << 20);
        let buf = Buf { img: effectcraft_raster::Image::filled(64, 48, [1.0; 4]), offset: [0.0; 2], scale: 1.0 };
        let mut previous = None;
        for t in [Tick::ZERO, Tick::from_seconds_f64(0.5), Tick::from_seconds_f64(1.0), Tick::ZERO] {
            let ctx = ctx.at(t);
            let cached = coverage_at(&ctx, &layer, &mask, &buf, Some(&cache)).unwrap();
            let fresh = coverage_at(&ctx, &layer, &mask, &buf, None).unwrap();
            assert_eq!(cached.data, fresh.data);
            if let Some(previous) = previous {
                assert_ne!(cached.data, previous);
            }
            previous = Some(cached.data.clone());
        }
    }
}
