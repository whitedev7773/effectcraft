# Gravity UI icons

Unmodified SVGs from https://github.com/gravity-ui/icons at commit
`5fade87690de42eca78b419e2509df39b325517d`, licensed under MIT (see `LICENSE.txt`).
Each SVG has an attribution sidecar and an entry in the repository's `ATTRIBUTION.md`.
Only generic interface artwork is included; no third-party brand marks are bundled.

`crates/ui-egui/src/gravity_icons.rs` maps the existing icon identifiers to these assets.
EffectCraft's SVG renderer creates white alpha textures at bounded DPI sizes, cached in each
egui context and tinted by the painter for dark/light themes and button states.
Tools without a matching Gravity icon retain EffectCraft's original vector drawings.
