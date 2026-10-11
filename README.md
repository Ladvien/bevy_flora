# bevy_flora

> ⚠️ **Vibe Coded.** This crate was written by an AI agent working from a human's direction. It is covered by tests and by a rendered visual QA loop (`qa/visual_findings.md` in the repository), but it has had no line-by-line human audit. Read it before you trust it.

[![crates.io](https://img.shields.io/crates/v/bevy_flora.svg)](https://crates.io/crates/bevy_flora) [![docs.rs](https://img.shields.io/docsrs/bevy_flora)](https://docs.rs/bevy_flora) [![CI](https://github.com/Ladvien/bevy_flora/actions/workflows/ci.yml/badge.svg)](https://github.com/Ladvien/bevy_flora/actions/workflows/ci.yml) [![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/Ladvien/bevy_flora/blob/main/LICENSE-MIT)

**Parametric, stylized procedural trees for Bevy that grow over time.** Pick one of 15 presets or tune every parameter, spawn a component, and watch a seedling grow into a mature tree. The same seed always gives the same tree.

![An oak growing from a bare trunk into a full crown while the camera circles it](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/growth.gif)

*`cargo run --release --example capture_growth`: the oak preset from age 0 to maturity, one frame per age step.*

## Compatibility

| `bevy_flora` | `bevy` | Rust |
|---|---|---|
| 0.5 | 0.20 | 1.97.1+ (edition 2024) |

## Install

```toml
[dependencies]
bevy_flora = "0.5"
```

| feature | default | what it does |
|---|---|---|
| `serde` | — | `Serialize` / `Deserialize` on every parameter type, so trees can be authored in RON or JSON |

The library depends on `bevy` with default features off (`std`, `bevy_asset`, `bevy_color`, `bevy_log`, `bevy_mesh`, `bevy_pbr`), so it adds no windowing or audio to a build.

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

### Growing many trees: `GpuGrowth`

A growing tree normally rebuilds both meshes every frame, 0.1 to 0.4 ms of CPU per tree. Add `GpuGrowth` and the tree is built once, at maturity, and grown in the vertex shader: each frame the plugin ages the skeleton and writes a few kilobytes of per-node state, and nothing else is uploaded.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{FloraPlugin, GpuGrowth, Growing, ProceduralTree, TreeAge, TreePreset};

fn forest(mut commands: Commands) {
    for i in 0..200 {
        commands.spawn((
            ProceduralTree(TreePreset::Oak.params()),
            TreeAge(0.0),
            Growing::default(),
            GpuGrowth,
            Transform::from_xyz((i % 20) as f32 * 8.0, 0.0, (i / 20) as f32 * 8.0),
        ));
    }
}
```

Main-world cost per frame with every tree growing (a headless `App`, release build, Apple M-series, median of three runs):

| 200 growing trees | CPU rebuild | `GpuGrowth` |
|---|---|---|
| oak | 17.6 ms | 0.81 ms |
| willow | 25.4 ms | 0.90 ms |
| oak, 2 smoothing passes | 69.8 ms | 0.70 ms |

Shadows follow the growth (the prepass shares the vertex shader), and the grown frames match the CPU path: over all 319 gallery scenarios the mean difference between the two renders is 0.08 of 255 (105 PNGs are byte-identical, the worst is a willow at 2.2). What differs, and why, is listed on `GpuGrowth` in the docs: smoothing is computed once on the mature mesh and deformed, side counts stay at the mature count, `AllBranches` leaves keep their mature positions, and the `Mesh` assets (picking, physics, culling) are the mature tree. It needs storage buffers in the vertex stage, so not WebGL2; there the trees grow on the CPU.

### Drawing a forest: `TreePool`

`ProceduralTree` gives every tree its own meshes, which a GPU can draw a few thousand of. For a forest build a `TreePool` once, a few seed variants of one tree, and place `PooledTree`s from it. Entities that share a mesh and a material are drawn in one instanced call, and the distance to the camera picks what is drawn: the full tree up close, two coarser levels of detail (30 m and 80 m by default, cross-fading) and, beyond 200 m, one camera-facing quad per tree that shows a view of the variant baked in the running app.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{FloraPlugin, LodSettings, PooledTree, TreePool, TreePreset, build_pool};

fn forest(
    mut commands: Commands,
    mut pools: ResMut<Assets<TreePool>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let pool = build_pool(
        &TreePreset::Oak.params(),
        8, // variants: seeds 1, 2, ... 8
        f32::INFINITY,
        &LodSettings::default(),
        &mut meshes,
    );
    let pool = pools.add(pool);
    for i in 0..100_000u32 {
        commands.spawn((
            PooledTree { pool: pool.clone(), variant: i },
            Transform::from_xyz((i % 316) as f32 * 8.0, 0.0, (i / 316) as f32 * 8.0),
        ));
    }
}
```

Measured with `cargo run --release --example forest -- --offscreen` (1920x1080, one shadow-casting sun, an oak forest on an 8 m grid seen across it from 30 m up, Apple M5, release build; the median of three runs, except 5 000 separate trees, which was one run, and the largest forests are not run with a mesh per tree, which would take an estimated 15 and 75 GB of GPU memory; timings on the machine drift by 40% and more between runs, so read them as that wide):

| trees | one `ProceduralTree` each | pool, levels of detail only | pool, with impostors |
|---|---|---|---|
| 1 000 | 114 ms | 7.5 ms | 6.2 ms |
| 5 000 | 1 280 ms | 19.0 ms | 7.0 ms |
| 20 000 | not run | 40.7 ms | 9.0 ms |
| 100 000 | not run | 118 ms | 15.3 ms (p95 16.2) |

The pool's trees are static: they have the age the pool was built at, and growing trees stay `ProceduralTree` (with `GpuGrowth`), which suits the few that are close. Impostors are baked once per variant by a few frames of offscreen rendering, so a tree appears when that is done, and they need `FloraPlugin` added after `DefaultPlugins` (before it, or with no renderer, the coarsest level is drawn at every distance and one warning says so). They are baked unlit, as a colour and a surface picture, and shaded by the scene's own lights every frame: they follow the time of day, glow with the sun behind them and sway with the wind. They cast no shadows, and they show one of eight views, at one elevation. Level 2 and the impostors replace thin twigs and most leaves, so a forest seen from above or close up should keep its near trees at level 0.

### Leaf shading and wind

Trees drawn by the plugin move in the wind, with no cost to the CPU per frame. `FloraWind` is a global resource: how hard (`strength`), which way (`direction`), how fast (`speed`), how far trees bend (`bend`), how much leaves flutter (`flutter`, `flutter_frequency`), how much the wind comes in gusts (`gust_strength`, `gust_frequency`) and sways across itself (`turbulence`). `TreeParams::wind` is how a species answers it, and the presets set their own: a willow moves more than an oak, a bonsai hardly at all. A `GustFront` is a band of stronger wind that sweeps across the world:

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{FloraWind, GustFront, TreePreset, WindResponse};

fn weather(mut wind: ResMut<FloraWind>, time: Res<Time>) {
    wind.strength = 0.6; // 0 is calm, 1 a strong wind
    wind.direction = Vec2::new(1.0, 0.0); // blowing toward +x
    // A band of wind, 30 m deep, crossing the world along +x at 20 m/s from x = -50.
    wind.launch_gust(
        GustFront { origin: Vec2::new(-50.0, 0.0), speed: 20.0, ..default() },
        &time,
    );
}

fn stiff_willow() -> bevy_flora::TreeParams {
    let mut params = TreePreset::Willow.params();
    params.wind = WindResponse { sway: 0.4, ..default() };
    params
}
```

Changing `FloraWind` rewrites every flora material once (two per tree, two per pool, one per impostor variant), so set it when the weather changes, not every frame; the gusts and the flutter animate by themselves. Wind writes no motion vectors, and branches do not sway on their own: the whole tree bends and the leaves flutter.

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
- **Growth over time.** Every branch has a birth age and a growth duration, derived from where it sits on the tree. A tree can be shown at any age, branches thicken after they lengthen, and leaves grow to full size without re-rolling or disappearing as the tree ages.
- **Deterministic.** Each branch draws randomness from its own seeded stream, so the same `seed` gives the same tree on every run, and aging never re-rolls branches or leaves that already exist.
- **Branch shape**: crown envelopes, phyllotaxis, apical dominance, Y-splits, recursive sub-branches, twist, and a gravity bend that sub-branches and leaves follow. Floor avoidance keeps even strongly drooping wood above the ground.
- **Closed wood.** Branches meet the trunk through flared collars and root segments, forks taper into their halves, and trunk tops end in a flat cap, a cone or a leader branch. Optional pipe-model radii, radius-adaptive ring counts and volume-preserving (Taubin) smoothing that keeps seams, branch ends and twig seats in place.
- **Leaves**: five styles (crossed planes, single quad, cluster sphere, star burst, needle cluster), three placements (terminal branches, all branches, tip clusters) and four orientations. Leaf cards are leaf-shaped (cut into the geometry, so there is no alpha-mask `discard`) and grow out of their twigs, crowns shade as one soft volume through crown-bent normals (`normal_bend`), and each leaf gets its own brightness (`color_variation`), hue (`hue_variation`) and share of the sky (`occlusion`), all baked into the mesh. The plugin draws trees with `FloraMaterial`, a `StandardMaterial` with a leaf shader: a darker, glossier upper side, a paler matte lower side, veins, and light through a leaf lit from behind (`LeafShading`). The meshes also draw with a plain `StandardMaterial`, without those.
- **Wind.** `FloraWind` bends every tree and flutters its leaves in the vertex shader, shadows and impostors included, with a travelling `GustFront`, and each species answers in its own way (`TreeParams::wind`).
- **Forest scale.** `TreePool` builds a few variants of a tree once, each at three levels of detail made from the skeleton (thin wood dropped, fewer sides and rings, fewer and larger leaves) and a baked camera-facing impostor for the far field; `PooledTree` places them. 100 000 oak trees draw at 15 ms a frame, leaf shading and wind included (see above); a forest of one `ProceduralTree` each is several times slower with the flora material than with a plain `StandardMaterial`.

## Examples

| example | what it shows |
|---|---|
| `showcase` | An interactive growing tree. Right-drag / wheel orbits, `[` `]` cycle presets, `N` reseeds, `Space` toggles growth, `R` restarts, `M` jumps to mature, `G` rebuilds, `U` switches between growing in the vertex shader (`GpuGrowth`) and rebuilding on the CPU. |
| `gallery` | Renders 319 scenarios (every preset at several ages and seeds, forced splits, smoothing, pipe radii (mature and young) and a wobbly twisted trunk on every preset, every leaf style × placement × orientation, every crown shape, parameter extremes) offscreen to four-view PNGs plus a `stats.tsv` with mesh sanity flags. This is the visual QA harness. `--filter <substr>` narrows it; `--gpu` grows every tree in the vertex shader. |
| `compare_gallery` | Compares two renders directory against directory (CPU `gallery` against `gallery --gpu`, or two `capture_growth` runs): mean difference and share of close pixels per PNG, non-zero exit past the limits. |
| `capture_growth` | Records the clip above as numbered PNG frames (`--gpu` grows the tree in the vertex shader). |
| `forest` | The forest-scale benchmark: up to 100 000 trees on a grid under a shadow-casting sun, drawn either naively (`--mode naive`, a `ProceduralTree` each) or from a `TreePool` (`--mode pool`, the default), timed per frame or saved as a screenshot. `--offscreen` times the real frame cost without a window; `--no-impostors`, `--lod` and `--shadows` change the setup, `--sun <x,y,z>`, `--wind <strength>`, `--wind-dir <x,z>` and `--gust <speed>` move the light and the weather. |

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
