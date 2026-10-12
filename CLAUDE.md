# CLAUDE.md

Guidance for Claude Code in this repository.

## Project

`bevy_flora`: a pure-Rust Bevy 0.20 crate that generates parametric, stylized procedural trees which can grow over time. See `ARCHITECTURE.md`.

## Commands

```bash
cargo build --release
cargo build --release --features serde
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo build --examples
cargo package --locked
cargo run --example showcase   # orbit: right-drag/wheel; [ ] presets; N reseed; Space grow; R restart; M mature; G rebuild
cargo run --release --example gallery -- --out target/gallery/latest [--gpu] [--warmup-secs <s>] [--filter <substr>]... [--no-shadows]   # 4-view PNG + stats.tsv per scenario
cargo run --release --example compare_gallery -- target/gallery/cpu target/gallery/gpu   # compares two gallery renders
cargo run --release --example forest -- --offscreen [--mode pool|naive|gpu|gpu-live|cpu-live] [--deterministic]   # forest benchmark, real frame times
```

## Layout

- `src/params.rs`, `crown.rs`, `presets.rs`: `TreeParams` and the 15 presets
- `src/skeleton.rs`, `growth.rs`: node tree and age model
- `src/mesh/`: ring helpers (winding lives in `rings.rs`), trunk, branches, smoothing
- `src/foliage.rs`, `pipeline.rs`, `plugin.rs`: leaves (scanned outlines included), `build_tree`, Bevy components/systems
- `src/venation.rs`: leaf veins by space colonisation (pure CPU), rasterised into the image `FloraTextures` shares
- `src/shading/` (`FloraWind`, `FloraMaterial`, leaf look and wind in `common.wesl`/`vertex.wesl`/`prepass.wesl`/`fragment.wesl`; `share.rs`: `SharedFlora`, materials shared by trees with equal looks; `textures.rs`: `FloraTextures`, the bark images and their mean colours)
- `src/gpu/` (`GpuGrowth`: growth in the vertex shader, `growth*.wesl`)
- `src/lod.rs` (levels of detail from the skeleton), `src/pool.rs` (`TreePool`, `PooledTree`), `src/impostor/` (baked far-field quads, `impostor.wesl`): forest scale
- `examples/showcase.rs`, `examples/gallery.rs` (visual QA renderer), `examples/compare_gallery.rs` (diffs two gallery renders), `examples/inspect.rs` (one tree photographed through every level of detail, at any bearing, height, zoom or forced level: readability checks), `examples/capture_growth.rs` (records `docs/growth.gif`, and `docs/grove.gif` with `--grove`, for the README), `examples/forest.rs` (forest benchmark: `--offscreen` for real frame times, never a windowed run; `--mode`, `--sun`, `--wind`, `--wind-dir`, `--wind-animate`, `--gust`, `--deterministic`, `--eye`/`--at`/`--fov`)
- `src/textures/` (bundled CC0 bark scans and leaf atlases, `CREDITS.md`, the generated outline table `leaves.rs`; compiled in by the default `textures` feature) and `tools/prepare_textures.py` (downloads and processes them and generates `leaves.rs`; repo only, not packaged)
- Publishing: `Cargo.toml` `include` keeps `qa/`, `slop/`, `docs/`, `tools/` out of the package; README media use absolute GitHub URLs
- Releasing: follow `AGENTS.md`. Every release, patch releases included, re-records `docs/growth.gif` and `docs/grove.gif` with `capture_growth`; the clips are not updated automatically
- `qa/visual_findings.md`: visual QA defect log (open / fixed / proposed / iterations)

## Conventions

- Prefer `TreeParams` fields over magic numbers.
- Core modules depend only on `bevy::math` and `bevy::color`; only `mesh/mod.rs`, `plugin.rs`, `shading/`, `gpu/`, `pool.rs` and `impostor/` touch render/ECS types (`lod.rs` is pure CPU).
- Docs: https://docs.rs/bevy/0.20.0, https://bevy.org/news/bevy-0-20/
