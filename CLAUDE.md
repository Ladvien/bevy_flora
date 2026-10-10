# CLAUDE.md

Guidance for Claude Code in this repository.

## Project

`bevy_flora`: a pure-Rust Bevy 0.20 crate that generates parametric, stylized procedural trees which can grow over time. See `ARCHITECTURE.md`.

## Commands

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo run --example showcase   # orbit: right-drag/wheel; [ ] presets; N reseed; Space grow; R restart; M mature; G rebuild
cargo run --example gallery -- --out target/gallery/latest [--filter <substr>]... [--no-shadows]   # 4-view PNG + stats.tsv per scenario
```

## Layout

- `src/params.rs`, `crown.rs`, `presets.rs`: `TreeParams` and the 15 presets
- `src/skeleton.rs`, `growth.rs`: node tree and age model
- `src/mesh/`: ring helpers (winding lives in `rings.rs`), trunk, branches, smoothing
- `src/foliage.rs`, `pipeline.rs`, `plugin.rs`: leaves, `build_tree`, Bevy components/systems
- `examples/showcase.rs`, `examples/gallery.rs` (visual QA renderer), `examples/capture_growth.rs` (records `docs/growth.gif` for the README)
- Publishing: `Cargo.toml` `include` keeps `qa/`, `slop/`, `docs/` out of the package; README media use absolute GitHub URLs
- `qa/visual_findings.md`: visual QA defect log (open / fixed / proposed / iterations)

## Conventions

- Prefer `TreeParams` fields over magic numbers.
- Core modules depend only on `bevy::math` and `bevy::color`; only `mesh/mod.rs` and `plugin.rs` touch render/ECS types.
- Docs: https://docs.rs/bevy/0.20.0, https://bevy.org/news/bevy-0-20/
