# Getting started

The crate features, the plugin's own behaviour, presets by name, and a tree's growing time.

## Install

```toml
[dependencies]
bevy_flora = "0.47"
```

| feature | default | what it does |
|---|---|---|
| `textures` | yes | The bundled CC0 bark and leaf scans (about 3.6 MB of JPEG compiled into the crate). Without it every scan is absent: wood is plain colour and leaves keep their procedural outline |
| `serde` | — | `Serialize` / `Deserialize` on every parameter type and on `Scatter`, so trees and forests can be authored in RON or JSON. A tree's fields may be left out (they keep their defaults); an unknown field is an error |
| `ron` | — | `serde`, plus the `.tree.ron` asset loader and `TreeParams::to_ron` / `from_ron` (see [Trees from files](growing.md#trees-from-files-proceduraltreeasset-and-treeron)) |
| `file_watcher` | — | Bevy's `file_watcher`: assets, `.tree.ron` trees included, reload when their files are saved. An app without an `assets` folder then logs a warning at startup |

The library depends on `bevy` with default features off (`std`, `bevy_asset`, `bevy_color`, `bevy_log`, `bevy_mesh`, `bevy_pbr`, and `jpeg` to decode the bundled textures), so it adds no windowing or audio to a build.

## Without the plugin

`build_tree` is a pure function from parameters and age to CPU meshes and the aged skeleton, so you can bake trees offline, feed your own renderer, or build colliders from the skeleton:

```rust
use bevy_flora::{TreePreset, build_tree};

let params = TreePreset::Pine.params();
let tree = build_tree(&params, f32::INFINITY);
let wood = tree.trunk.clone().into_bevy_mesh();  // bevy::mesh::Mesh
let leaves = tree.foliage.into_bevy_mesh();
assert!(tree.flowers.is_empty()); // a flowering tree's flowers (`FoliageParams::blossom`); a pine has none
assert!(tree.skeleton.nodes.len() > 1);
```
