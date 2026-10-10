# bevy_flora

> ⚠️ **Vibe Coded.** This crate was written by an AI agent working from a human's direction. It is covered by tests and by a rendered visual QA loop (`qa/visual_findings.md` in the repository), but it has had no line-by-line human audit. Read it before you trust it.

[![crates.io](https://img.shields.io/crates/v/bevy_flora.svg)](https://crates.io/crates/bevy_flora) [![docs.rs](https://img.shields.io/docsrs/bevy_flora)](https://docs.rs/bevy_flora) [![CI](https://github.com/Ladvien/bevy_flora/actions/workflows/ci.yml/badge.svg)](https://github.com/Ladvien/bevy_flora/actions/workflows/ci.yml) [![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/Ladvien/bevy_flora/blob/main/LICENSE-MIT)

**Parametric, stylized procedural trees for Bevy that grow over time.** Pick one of 15 presets or tune every parameter, spawn a component, and watch a seedling grow into a mature tree. The same seed always gives the same tree.

![An oak growing from a bare trunk into a full crown while the camera circles it](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/growth.gif)

*`cargo run --release --example capture_growth`: the oak preset from age 0 to maturity, one frame per age step.*

## Compatibility

| `bevy_flora` | `bevy` | Rust |
|---|---|---|
| 0.1 | 0.20 | 1.97.1+ (edition 2024) |

## Install

```toml
[dependencies]
bevy_flora = "0.1"
```

| feature | default | what it does |
|---|---|---|
| `serde` | — | `Serialize` / `Deserialize` on every parameter type, so trees can be authored in RON or JSON |

The library depends on `bevy` with default features off (`std`, `bevy_asset`, `bevy_color`, `bevy_mesh`, `bevy_pbr`), so it adds no windowing or audio to a build.

## Quickstart

Add the plugin and spawn a tree. `Growing` advances its age every frame until it is mature; leave it off and set `TreeAge` yourself (the default, `f32::INFINITY`, is the mature tree).

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{FloraPlugin, Growing, ProceduralTree, TreeAge, TreePreset};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FloraPlugin))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    let mut params = TreePreset::Maple.params();
    params.seed = 7;
    commands.spawn((ProceduralTree(params), TreeAge(0.0), Growing { rate: 1.0 }));

    commands.spawn((
        DirectionalLight { shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 5.0, 20.0).looking_at(Vec3::new(0.0, 4.0, 0.0), Vec3::Y),
    ));
}
```

The plugin keeps two child entities on each tree (wood and leaves) and rebuilds their meshes whenever `ProceduralTree` or `TreeAge` changes. It sends a `TreeGenerated` message after each rebuild.

### Without the plugin

`build_tree` is a pure function from parameters and age to CPU meshes and the aged skeleton, so you can bake trees offline, feed your own renderer, or build colliders from the skeleton:

```rust
use bevy_flora::{TreePreset, build_tree};

let params = TreePreset::Pine.params();
let tree = build_tree(&params, f32::INFINITY);
let wood = tree.trunk.clone().into_bevy_mesh();  // bevy::mesh::Mesh
let leaves = tree.foliage.into_bevy_mesh();
assert!(tree.skeleton.nodes.len() > 1);
```

## Features

- **15 presets**: oak, pine, willow, birch, palm, cypress, bonsai, maple, spruce, poplar, baobab, dragon tree, Japanese maple, dead tree and redwood. Each is a plain `TreeParams` value you can edit.
- **Growth over time.** Every branch has a birth age and a growth duration, derived from where it sits on the tree. A tree can be shown at any age, branches thicken after they lengthen, and leaves grow to full size without re-rolling as the tree ages.
- **Deterministic.** Each branch draws randomness from its own seeded stream, so the same `seed` gives the same tree on every run, and aging never re-rolls branches or leaves that already exist.
- **Branch shape**: crown envelopes, phyllotaxis, apical dominance, Y-splits, recursive sub-branches, twist, and a gravity bend that sub-branches and leaves follow. Floor avoidance keeps even strongly drooping wood above the ground.
- **Closed wood.** Branches meet the trunk through flared collars and root segments, forks taper into their halves, and trunk tops end in a flat cap, a cone or a leader branch. Optional pipe-model radii, radius-adaptive ring counts and volume-preserving (Taubin) smoothing that keeps seams, branch ends and twig seats in place.
- **Leaves**: five styles (crossed planes, single quad, cluster sphere, star burst, needle cluster), three placements (terminal branches, all branches, tip clusters) and four orientations. The leaf material is translucent, so back-lit leaves are not black.

## Examples

| example | what it shows |
|---|---|
| `showcase` | An interactive growing tree. Right-drag / wheel orbits, `[` `]` cycle presets, `N` reseeds, `Space` toggles growth, `R` restarts, `M` jumps to mature, `G` rebuilds. |
| `gallery` | Renders 163 scenarios (every preset at several ages and seeds, every leaf style × placement, parameter extremes) offscreen to four-view PNGs plus a `stats.tsv` with mesh sanity flags. This is the visual QA harness. `--filter <substr>` narrows it. |
| `capture_growth` | Records the clip above as numbered PNG frames. |

```sh
cargo run --release --example showcase
cargo run --release --example gallery -- --out target/gallery/latest --filter mature_
```

## How it works

`TreeParams` → skeleton (a node tree of trunk, leader, primaries, splits and sub-branches) → birth times → the skeleton at a given age → wood mesh and leaf points → `MeshData` → Bevy `Mesh`. `ARCHITECTURE.md` has the details and the invariants (winding, bend, junctions) the meshers rely on.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
