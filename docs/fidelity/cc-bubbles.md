# CC Bubbles observations

Compared on Windows on 10 October 2026 through the running host's scripting API,
using original constant-gray solid layers. No plug-in implementation or application
bundle contents were read. Reference frames stay in gitignored `plan/aftereffects/bubbles/`.
The behavior descriptions are in the [Cycore FX HD 1.8.9 manual, pages 63–64](https://cycorefx.com/downloads/cfx_hd_std/CycoreFX%20HD%201.8.9%20Manual.pdf).

Observed defaults are Amount 150, Speed 1, Wobble Amplitude 10, Wobble Frequency 1,
Size 4, Liquid reflection and Fade Inwards shading. Existing stored project values
are retained; these defaults apply when adding a new effect.

At the same Size, an isolated bubble's diameter stays approximately 87 pixels when
the source changes from 1920 × 960 to 512 × 256 or 960 × 1920. Amount 120 on the
1920 × 960 source at Size 1 produced 24 connected components at time zero, with the
largest diameter approximately 26 pixels. Many particles start outside the frame.
Larger bubbles rise faster; size changes do not multiply their travel speed.

For normalized radial distance `d`, constant-source observations give these curves:

| Shading | Straight RGB | Alpha inside the disc |
|---|---|---|
| None | source RGB | 1 |
| Lighten | `c + (1 - c) * d²` | 1 |
| Darken | `c * (1 - d²)` | 1 |
| Fade Inwards | source RGB | `d⁴` |
| Fade Outwards | source RGB | `1 - d²` |

At half-radius on a 20% gray source, the measured Lighten/Darken values are
approximately 40%/15%, and the two fade alphas are approximately 1/16 and 3/4.
None adds no white highlight. CPU regression tests check these values, source-size
independence, duplicate stability, reduced-resolution scaling and invalid inputs.
GPU comparisons cover all five shading modes, both reflection modes, zero Size,
full/half resolution and 8/32-bit compositing.

The particle distribution remains an original deterministic approximation. Its size,
density and rise speed are calibrated to observations; individual positions and
trajectories do not reproduce the host's random sequence. Reflection of textured
sources remains approximate, and motion-blur parity has not been measured. This
comparison does not establish full effect or project parity.

Validation on Windows: five focused CPU tests, the simulation golden test (including
20 refreshed CC Bubbles hashes), the GPU simulation comparison, formatting and
`cargo clippy -p effectcraft-effects --lib --no-deps -- -D warnings` pass. The
unrestricted Clippy command is blocked by an existing `collapsible_if` warning in
`crates/keyframe/src/text_doc.rs`. macOS golden hashes have not been checked locally
after this correction.

On the user's 1920 × 960 composition at 22 seconds, mean alpha changes from
0.18049 to 0.00306; the host reference is 0.00292. This measures overall coverage,
not positional parity. Final CPU/GPU PNGs differ at one of 1,843,200 pixels, by two
RGB code values after 8-bit quantization and straight-alpha export; alpha is identical.
