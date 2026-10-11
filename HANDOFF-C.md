# HANDOFF slice C (WindBuffer), branch perf/c-wind-buffer

## Design
Global wind (5 vec4: wind, motion, rate, front, front_shape) leaves `FloraUniform` and lives in ONE
storage buffer: resource `FloraWindBuffer(Handle<ShaderBuffer>)` (pub, exported), bound at `@binding(102)`
(`#[storage(102, read_only)] pub wind: Handle<ShaderBuffer>`) in `FloraExtension`, `GrowthExtension`,
`ImpostorMaterial` (100 growth nodes, 101 `FloraUniform` unchanged). `common.wesl` declares
`flora_wind: array<vec4<f32>>` once; `gust_front/flutter_offset/wind_bend` read `flora_wind[0..4]` (same math).
`sync_wind` (run_if resource_changed<FloraWind>) compares then writes the buffer IN PLACE (same size
`cast_slice_mut`), touches no material. `FloraUniform::new(params)` no longer takes the wind (API break);
`wind_differs/set_wind/global_wind/HasFlora/rewrite_wind` removed; packing is `pub(crate) wind_vectors(&FloraWind)->[Vec4;5]`.
`flora_material(color, uniform, leaf, &FloraWindBuffer)`, `ImpostorMaterial::new(.., flora, &FloraWindBuffer)`.
The buffer is created in `shading::build`/`finish` once `Assets<ShaderBuffer>` exists (add_asset_types now inits it too).
Fallback (no vertex storage buffers): `build` inserts private `FloraSupportPending` instead of `FloraMaterialSupport`;
`detect_flora_support` (first in the Update chain) once `RenderDevice` exists removes Pending, then inserts
`FloraMaterialSupport` or warns and removes `ImpostorSupport`. `rebuild_trees` and `spawn_pooled_trees` are
`run_if(flora_decided)` so no tree is built before the decision (no rebuild needed). Tree/pool systems take
`Option<Res<FloraWindBuffer>>` filtered by FloraMaterialSupport.

## State
All code edits are in the commit, NEVER COMPILED OR TESTED IN THIS WORKTREE. (Bug: my edit tool resolved relative
paths against /Users/ladvien/pixy_tree, the main checkout, not the worktree; the earlier "builds/tests/identity pass"
I saw ran against UNMODIFIED worktree sources and prove nothing. I moved the diff here with git diff/apply and
restored the main checkout's src/.) Tests rewritten in shading/mod.rs and plugin.rs
(`changing_the_wind_modifies_no_material`, `a_material_made_after_a_wind_change_sees_the_current_wind`).

## Next steps
1. `CARGO_TARGET_DIR=/tmp/ft-c cargo build --release --all-targets --all-features`; fix compile errors (unchecked: unused imports
   e.g. `FloraWind` in gpu/render.rs, `Handle`/`Asset` in shading, `Vec4` import in plugin tests, impostor/bake.rs).
2. `cargo test --release --all-features`; clippy `-D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`;
   `cargo build --release --examples`.
3. Smoke (no ERROR/panicked/Device lost) + md5 identity vs /tmp/baseline/base_{naive,naive_gust,gpu,gpulive,pool}.png:
   `forest --mode naive|gpu|gpu-live --trees 64 --spacing 10 --wind 0.6 [--gust 10] --deterministic --warmup-secs 6 --screenshot X`,
   pool: `--mode pool --trees 400 --spacing 10 --wind 0.6 --deterministic --warmup-secs 10`; `gallery --out D --gpu --filter mature_`.
4. Perf: `forest --mode pool --trees 20000 --wind 0.6 --wind-animate --offscreen --frames 120 --warmup-secs 8`, forest_base vs new, interleaved.
5. Try `cargo check --target wasm32-unknown-unknown --lib` (not tried). Docs: FloraWind doc already updated; lib.rs docs mention nothing stale (check "rewritten").

## Risks
- Unverified: in-place same-size ShaderBuffer write must not re-prepare materials (growth's write_state relies on it).
- WGSL: runtime-sized `array<vec4<f32>>` storage in vertex stage of main + prepass + impostor; ShaderBuffer::from(Vec<[f32;4]>) layout.
- Making `ImpostorMaterial` storage-bound may change its bindless/non-bindless layout.
- Merge overlap: slice A (rebuild_trees material part, shading/mod.rs `flora_material`), slice B (gpu/render.rs GrowthExtension literal).
