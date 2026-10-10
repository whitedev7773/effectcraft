# After Effects project migration (public scripting API)

This is a clean-room migration path for projects that can still be opened in After Effects. It uses only the documented project scripting objects. It does not read the AEP binary format or Adobe application files.

1. Open the source AEP in After Effects. Run `export.jsx` with **File ▸ Scripts ▸ Run Script File…**. It adds `EC_EXPORT_V1_*` folders containing a snapshot; the original AEP on disk is unchanged until you save it.
2. Use **File ▸ Save As ▸ Save a Copy As XML…** to write a separate `.aepx` copy. Keep the copy and snapshot in the gitignored `plan/aftereffects/` directory; they are local reference material and must not be committed.
3. Run `python tools/ae-migrate/reconstruct.py copy.aepx snapshot.json`.
4. Run `effectcraft-cli exec effect.list --empty --json > effect-specs.json` to capture EffectCraft's available effect parameters, then `cargo run -p effectcraft-project --example ae_snapshot -- snapshot.json output.ecproj effect-specs.json`. The last argument is optional; without it, effect settings are retained as generic properties and many effects render with defaults.

The converter retains compositions, source links, layers, timing, text (including per-character font and style runs on AE 24.3 or newer), basic transforms, keyframes, markers, masks, shapes, expressions, and effect property data. Render parity is limited by EffectCraft's effect implementations and by the fields exposed through the script snapshot. Treat the output as a migration candidate and compare it against the source project before replacing your working file.
