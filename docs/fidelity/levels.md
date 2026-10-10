# Levels: channel order and observed-project migration

Levels applies the Red, Green and Blue corrections before the master RGB correction.
Alpha has its own correction. The Channel popup chooses the visible controls; it does
not disable the other channels' corrections. CPU and GPU use this order for both Levels
and Levels (Individual Controls).

An observation made through After Effects' public scripting API used an original solid
colour of `(0.2, 0.4, 0.6)`, RGB input black/white `0.1 / 0.9`, gamma `2`, output
black/white `0.1 / 0.8`, Red output white `0.5`, and Blue gamma `0.5`. Its opaque 8-bpc
PNG centre was `(26, 135, 127)`. The independent formula produces approximately
`(0.1, 0.52866, 0.49906)`, within one integer code of the observation. Applying the
master first would instead produce approximately `(0.17374, 0.52866, 0.42693)`.
The synthetic CPU regression checks the observed result; GPU fixtures exercise both
effect variants with combined channel/master controls and with colour and alpha gain
above one. Levels' numeric input/output controls retain their convenient 0–1 slider span while
allowing values from -32768 to 32768, including imported HDR values above the previous
upper bound of two.

## Migration

`crates/project/examples/ae_snapshot.rs` maps `ADBE Easy Levels2` to `ec.color.levels`.
Levels' scripting controls reuse `-0003` through `-0009` for the selected channel;
`-0002` is the Histogram, not a numeric control. Matching by parameter position can
therefore shift values or assign a selected channel's controls to RGB.

An observer can add `levelsChannels` to the effect observation:

```json
{
  "matchName": "ADBE Easy Levels2",
  "children": [{"matchName": "ADBE Easy Levels2-0001", "value": 1}],
  "levelsChannels": [
    {"channel": 1, "children": [{"matchName": "ADBE Easy Levels2-0005", "value": 2}]},
    {"channel": 5, "children": [{"matchName": "ADBE Easy Levels2-0004", "value": 0.88}]}
  ]
}
```

Each channel's `children` uses the existing property observation format, including
keys, interpolation, temporal ease and expression fields. Capture channels 1–5 by
selecting the popup through the public API and restore its saved selection in `finally`.
The converter maps explicit match-name suffixes, converts popup values from one-based
to zero-based, and preserves numeric values without treating them as 0–255 codes.
Older observations without `levelsChannels` import only their selected channel; the
missing hidden channel values cannot be recovered from that observation alone.

The local user-project repair updates only its Levels group and next-id counter and
is saved as a separate project. It includes all five observed channel sets. Local AE
observations, user media and project renders remain under ignored `plan/aftereffects/`
or next to the user's project; none are shipped. Full bubble-frame position/trajectory
parity and 32-bpc clipping-popup fidelity are outside this verification.

Validation on Windows x86_64: the converter's nine example tests, ten CPU tests selected
by `levels`, and the GPU `tests_fx_tone::levels_and_gamma` fixtures pass. A fresh CLI
render of the original opaque solid produces `(26, 135, 127, 255)` on CPU and GPU,
matching the observed centre exactly. The repaired user's 1920×960 bubble composition
at 22 seconds produces identical CPU/GPU PNGs; this comparison does not establish
AE bubble position, coverage or edge parity. `cargo fmt --all -- --check` passes.
`cargo clippy -p effectcraft-effects --lib --no-deps -- -D warnings` also passes;
this is a scoped check, not the workspace CI gate.

The public [Adobe colour correction reference](https://helpx.adobe.com/sg/after-effects/desktop/adjust-colors/work-with-color-correction-effects/color-correction-effects.html)
describes the colour/alpha remapping controls. No application implementation or plug-in
was inspected.
