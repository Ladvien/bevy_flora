# bevy_flora

> ⚠️ **Vibe Coded.** This crate was written by an AI agent working from a human's direction. It is covered by tests and by a rendered visual QA loop (`qa/visual_findings.md` in the repository), but it has had no line-by-line human audit. Read it before you trust it.

[![crates.io](https://img.shields.io/crates/v/bevy_flora.svg)](https://crates.io/crates/bevy_flora) [![docs.rs](https://img.shields.io/docsrs/bevy_flora)](https://docs.rs/bevy_flora) [![CI](https://github.com/Ladvien/bevy_flora/actions/workflows/ci.yml/badge.svg)](https://github.com/Ladvien/bevy_flora/actions/workflows/ci.yml) [![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/Ladvien/bevy_flora/blob/main/LICENSE-MIT)

**Parametric, stylized procedural trees for Bevy that grow over time.** Pick one of 15 presets or tune every parameter, spawn a component, and watch a seedling grow into a mature tree. The same seed always gives the same tree.

![A maple growing from a bare trunk into a full crown while the camera circles it](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/growth.gif)

*`cargo run --release --example capture_growth`: the maple preset from age 0 to maturity, one frame per age step, in a light wind.*

![A grove of eight trees of six species growing in turn while the camera circles them and a gust of wind sweeps through](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/grove.gif)

*`cargo run --release --example capture_growth -- --grove`: a grove of oak, two maples, two birches, a pine, a cypress, a willow and a Japanese maple, each at its own age, with a gust front crossing it in the last seconds. Every tree is a `ProceduralTree` with its own seed.*

## Compatibility

| `bevy_flora` | `bevy` | Rust |
|---|---|---|
| 0.8 | 0.20 | 1.97.1+ (edition 2024) |

## Install

```toml
[dependencies]
bevy_flora = "0.8"
```

| feature | default | what it does |
|---|---|---|
| `textures` | yes | The bundled CC0 bark and leaf scans (about 3.7 MB of JPEG compiled into the crate). Without it every scan is absent: wood is plain colour and leaves keep their procedural outline |
| `serde` | — | `Serialize` / `Deserialize` on every parameter type, so trees can be authored in RON or JSON |

The library depends on `bevy` with default features off (`std`, `bevy_asset`, `bevy_color`, `bevy_log`, `bevy_mesh`, `bevy_pbr`, and `jpeg` to decode the bundled textures), so it adds no windowing or audio to a build.

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

Shadows follow the growth (the prepass shares the vertex shader), and the grown frames match the CPU path: over all 319 gallery scenarios the mean difference between the two renders is 0.07 of 255 (39 PNGs are byte-identical, the worst is `extreme_lagthick_6` at 1.25). What differs, and why, is listed on `GpuGrowth` in the docs: smoothing is computed once on the mature mesh and deformed, side counts stay at the mature count, `AllBranches` leaves keep their mature positions, and the `Mesh` assets (picking, physics, culling) are the mature tree. It needs storage buffers in the vertex stage, so not WebGL2; there the trees grow on the CPU.

### Drawing a forest: `TreePool`

`ProceduralTree` gives every tree its own meshes, which a GPU can draw a few thousand of. Trees with equal looks (the same colours, trunk height, wind response and leaf shading; seeds and every other parameter may differ) share their two materials, so a thousand oaks cost 22 ms a frame, not 58; give each tree its own colour or trunk height and each gets its own materials again. For a forest build a `TreePool` once, a few seed variants of one tree, and place `PooledTree`s from it. Entities that share a mesh and a material are drawn in one instanced call, and the distance to the camera picks what is drawn: the full tree up close, two coarser levels of detail (30 m and 80 m by default, cross-fading) and, beyond 200 m, one camera-facing quad per tree that shows a view of the variant baked in the running app.

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

Measured for 0.6.1 with `cargo run --release --example forest -- --offscreen` (1920x1080, one shadow-casting sun, wind on, an oak forest on an 8 m grid seen across it from 30 m up, Apple M4 Pro, release build; the median of three runs on a machine with nothing else running, the runs within 1% of each other; the larger forests are not run with a mesh per tree, which would take an estimated 15 and 75 GB of GPU memory):

| trees | one `ProceduralTree` each | pool, levels of detail only | pool, with impostors |
|---|---|---|---|
| 1 000 | 22 ms | 4.2 ms | 3.3 ms |
| 5 000 | 60 ms | 8.0 ms | 3.3 ms |
| 20 000 | not run | 18.7 ms | 3.4 ms |
| 100 000 | not run | 64.7 ms | 7.3 ms |

Bark (0.7.0) and leaf scans (0.8.0) leave these figures where they were, within what could be measured: the machine these were taken on has been loaded by other builds since, so the table above was not re-measured, and the releases were compared by interleaved runs instead (see the `CHANGELOG`). In those, 100 000 pooled oaks cost no more in 0.8.0 than in 0.7.0 (0.6.2 and 0.7.0 took 7.41 and 7.45 ms), and a thousand individual `ProceduralTree` oaks cost 4 to 10% more, because a leaf card cut to a real outline has more vertices than the old fan.

The pool's trees are static: they have the age the pool was built at, and growing trees stay `ProceduralTree` (with `GpuGrowth`), which suits the few that are close. Impostors are baked once per variant by a few frames of offscreen rendering, so a tree appears when that is done, and they need `FloraPlugin` added after `DefaultPlugins` (before it, with no renderer, or on a device without storage buffers in the vertex stage such as WebGL2, the coarsest level is drawn at every distance and one warning says so). They are baked unlit, as a colour and a surface picture, and shaded by the scene's own lights every frame: they follow the time of day, glow with the sun behind them and sway with the wind. They cast no shadows, and they show one of eight views, at one elevation. With MSAA an impostor keeps the gaps of its crown: each pixel draws the share of its samples the baked tree covers, so a far crown is as airy, and as light, as the leaves it stands for. Level 2 and the impostors replace thin twigs and most leaves, so a forest seen from above or close up should keep its near trees at level 0.

### Leaf shading and wind

Trees drawn by the plugin move in the wind, with no cost to the CPU per frame. `FloraWind` is a global resource: how hard (`strength`), which way (`direction`), how fast (`speed`), how far trees bend (`bend`), how much leaves flutter (`flutter`, `flutter_frequency`), how much the wind comes in gusts (`gust_strength`, `gust_frequency`) and sways across itself (`turbulence`). `TreeParams::wind` is how a species answers it, and the presets set their own: a willow moves more than an oak, a bonsai hardly at all. A `GustFront` is a band of stronger wind that sweeps across the world:

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{FloraWind, GustFront, TreePreset, WindResponse};

// Call this once, when the storm begins (from an event or a timer), not every frame: launching a
// gust restarts its front at `origin`.
fn start_storm(mut wind: ResMut<FloraWind>, time: Res<Time>) {
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

Changing `FloraWind` writes one small shared buffer (`FloraWindBuffer`) and touches no material, so you can animate it every frame (a storm that builds, a wind that turns): a pooled forest of 20 000 trees takes 3.3 ms a frame with the wind changing every frame, where 0.5 took 212 ms (Apple M4 Pro). Launching a gust front writes the same buffer. Wind writes no motion vectors, and branches do not sway on their own: the whole tree bends and the leaves flutter. Leaf shading and wind need storage buffers in the vertex stage; on a device without them (WebGL2) trees are drawn with plain materials and one warning says so.

### Bark

Trunks and branches wear a bundled CC0 bark scan (a colour map and a normal map, 512 px wide, about 3.2 MB for all 14). Each preset picks the scan of its species (`TreeParams::bark`); `BarkParams` sets how it follows the wood's growth. The scan is laid out in metres on the *mature* tree, so a growing tree's bark never slides; wood thinner than `smooth_below` is smooth, and by `fissured_above` it shows the whole scan, so a young branch gains relief as it thickens. The scan's average colour is divided out of the material's base colour, so a textured tree keeps its preset colour, and far trees, impostors and the plain `StandardMaterial` fallback look as they always did.

```rust,no_run
use bevy_flora::{BarkParams, BarkScan, TreePreset};

fn gnarled_oak() -> bevy_flora::TreeParams {
    let mut params = TreePreset::Oak.params();
    // Fissured sooner than the preset: smooth under 1 cm of radius, the whole scan from 8 cm.
    params.bark = BarkParams { scan: Some(BarkScan::Brown), smooth_below: 0.01, fissured_above: 0.08 };
    params
}

fn plain_pine() -> bevy_flora::TreeParams {
    let mut params = TreePreset::Pine.params();
    params.bark.scan = None; // flat colour, as before 0.7
    params
}
```

The textures are compiled in by the default `textures` feature (about 3.7 MB of JPEG for the bark and the leaf scans); with `default-features = false` every scan is absent, wood is plain colour and leaves keep their procedural outline. Their sources and licences (all CC0 1.0, no attribution required) are in [`src/textures/CREDITS.md`](src/textures/CREDITS.md).

### Leaves: real outlines and grown veins

Seven presets (oak, willow, birch, bonsai, maple, poplar and Japanese maple) cut their leaf cards to the real outline of a scanned leaf and wear it. `FoliageParams::scan` (`LeafScan`) picks one of seven bundled CC0 atlases of four photographed leaves each; every leaf is one of the four (chosen by a hash of its own random draws, which never change as the tree ages, so a leaf keeps its shape while the tree grows and on `GpuGrowth` too), the card is as wide as that leaf is, and its length is the leaf's `size`. The atlas's colour and normal maps are applied in the fragment shader and its average colour is divided out of the leaf colour, so a crown keeps its preset colour. Distant levels of detail cut the cards to a coarser outline.

Leaves with no scan (the other presets and any `TreeParams` of your own, since `scan` defaults to `None`) keep the procedural outline, and their veins are no longer a formula: a vein network is grown from the petiole toward auxin sources scattered over the card by space colonisation (Runions et al., SIGGRAPH 2005), widths by Murray's law, drawn into one small image that every such leaf shares (`bevy_flora::venation`; `LeafShading::veins` sets how strongly the shader draws them). Needle clusters and cluster spheres are not leaf-shaped and have none.

```rust,no_run
use bevy_flora::{LeafScan, TreePreset};

fn maple_with_oak_leaves() -> bevy_flora::TreeParams {
    let mut params = TreePreset::Maple.params();
    params.foliage.scan = Some(LeafScan::Oak);
    params
}

fn procedural_oak() -> bevy_flora::TreeParams {
    let mut params = TreePreset::Oak.params();
    params.foliage.scan = None; // the procedural leaf, with grown veins
    params
}
```

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
- **Leaves**: five styles (crossed planes, single quad, cluster sphere, star burst, needle cluster), three placements (terminal branches, all branches, tip clusters) and four orientations. Leaf cards are leaf-shaped (cut into the geometry, so there is no alpha-mask `discard`) and grow out of their twigs, crowns shade as one soft volume through crown-bent normals (`normal_bend`), and each leaf gets its own brightness (`color_variation`), hue (`hue_variation`) and share of the sky (`occlusion`), all baked into the mesh. The plugin draws trees with `FloraMaterial`, a `StandardMaterial` with a leaf shader: a darker, glossier upper side, a paler matte lower side, veins, and light through a leaf lit from behind (`LeafShading`); facing the sun, the gloss gives way to that glow, so a backlit crown stays green instead of mirroring the sun white. The meshes also draw with a plain `StandardMaterial`, without those.
- **Wind.** `FloraWind` bends every tree and flutters its leaves in the vertex shader, shadows and impostors included, with a travelling `GustFront`, and each species answers in its own way (`TreeParams::wind`).
- **Bark.** Trunks and branches wear one of 14 bundled CC0 bark scans (colour and normal map; `TreeParams::bark`, `BarkScan`), laid out in metres on the mature tree so a growing tree's bark never slides, smooth on young wood and fissured as a branch thickens (`BarkParams`), and averaging the preset colour, so far trees, impostors and the plain-material fallback keep their colour. Compiled in by the default `textures` feature.
- **Leaf scans and grown veins.** Seven presets cut their leaf cards to the real outline of a bundled CC0 leaf scan and wear its colour and normal map (`FoliageParams::scan`, `LeafScan`); every other leaf-shaped card has veins grown by space colonisation (`venation`), drawn by the leaf shader from one small shared image.
- **Forest scale.** `TreePool` builds a few variants of a tree once, each at three levels of detail made from the skeleton (thin wood dropped, fewer sides and rings, fewer and larger leaves) and a baked camera-facing impostor for the far field; `PooledTree` places them. 100 000 oak trees draw at 7.3 ms a frame, leaf shading and wind included (see above). A forest of individual `ProceduralTree`s is fine for a few thousand trees, because trees with equal looks share their materials (1 000 oaks: 22 ms a frame), but each keeps its own meshes, so use a pool beyond that.

## Examples

| example | what it shows |
|---|---|
| `showcase` | An interactive growing tree. Right-drag / wheel orbits, `[` `]` cycle presets, `N` reseeds, `Space` toggles growth, `R` restarts, `M` jumps to mature, `G` rebuilds, `U` switches between growing in the vertex shader (`GpuGrowth`) and rebuilding on the CPU. |
| `gallery` | Renders 319 scenarios (every preset at several ages and seeds, forced splits, smoothing, pipe radii (mature and young) and a wobbly twisted trunk on every preset, every leaf style × placement × orientation, every crown shape, parameter extremes) offscreen to four-view PNGs plus a `stats.tsv` with mesh sanity flags. This is the visual QA harness. `--filter <substr>` narrows it; `--gpu` grows every tree in the vertex shader. |
| `compare_gallery` | Compares two renders directory against directory (CPU `gallery` against `gallery --gpu`, or two `capture_growth` runs): mean difference and share of close pixels per PNG, non-zero exit past the limits. |
| `inspect` | Photographs one tree under controlled conditions, to see how it reads: a camera that backs off through every level of detail and into the impostor (`--distance a,b --frames n`), optionally zooming so the tree keeps its size on screen and a level switch shows as a jump (`--constant-size`), any bearing, height and field of view, one level forced at every distance (`--level`), the empty scene (`--empty`) to measure the tree's pixels against, with or without MSAA. The clock is fixed, so runs repeat. |
| `capture_growth` | Records the clips above as numbered PNG frames: one tree growing (`--preset <name>`, default maple) or, with `--grove`, a grove of mixed species with a gust front crossing it (`--gpu` grows the trees in the vertex shader). The clock steps a fixed 1/24 s per frame, so the wind sways smoothly and two runs write the same frames; the `ffmpeg` commands that make the GIFs are in the example's docs. |
| `forest` | The forest-scale benchmark: up to 100 000 trees on a grid under a shadow-casting sun, drawn from a `TreePool` (`--mode pool`, the default), as a `ProceduralTree` each (`--mode naive`), or as `GpuGrowth` trees, fully grown or growing for the whole run (`--mode gpu`, `gpu-live`; `--mode cpu-live` is the CPU-rebuilt equivalent), timed per frame or saved as a screenshot. `--offscreen` times the real frame cost without a window and the result line gives the mean, p95, median, minimum and CPU time per frame; `--no-impostors`, `--lod` and `--shadows` change the setup; `--sun <x,y,z>`, `--wind <strength>`, `--wind-dir <x,z>`, `--gust <speed>` and `--wind-animate` (the strength changes every frame) move the light and the weather; `--deterministic` fixes the clock at 1/60 s, so two builds' `--screenshot`s compare byte for byte. |

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
