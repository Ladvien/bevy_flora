# bevy_flora

> ⚠️ **Vibe Coded.** This crate was written by an AI agent working from a human's direction. It is covered by tests and by a rendered visual QA loop (`qa/visual_findings.md` in the repository), but it has had no line-by-line human audit. Read it before you trust it.

[![crates.io](https://img.shields.io/crates/v/bevy_flora.svg)](https://crates.io/crates/bevy_flora) [![docs.rs](https://img.shields.io/docsrs/bevy_flora)](https://docs.rs/bevy_flora) [![CI](https://github.com/Ladvien/bevy_flora/actions/workflows/ci.yml/badge.svg)](https://github.com/Ladvien/bevy_flora/actions/workflows/ci.yml) [![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/Ladvien/bevy_flora/blob/main/LICENSE-MIT)

**Parametric, stylized procedural trees for Bevy that grow over time.** Pick one of 16 presets (15 trees and a shrub) or tune every parameter, spawn a component, and watch a seedling grow into a mature tree. The same seed always gives the same tree.

![A maple growing from a bare trunk into a full crown while the camera circles it](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/growth.gif)

*`cargo run --release --example capture_growth`: the maple preset from age 0 to maturity, one frame per age step, in a light wind.*

![A grove of eight trees of six species growing in turn while the camera circles them and a gust of wind sweeps through](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/grove.gif)

*`cargo run --release --example capture_growth -- --grove`: a grove of oak, two maples, two birches, a pine, a cypress, a willow and a Japanese maple, each at its own age, with a gust front crossing it in the last seconds. Every tree is a `ProceduralTree` with its own seed.*

![The same grove fully grown through a year: the leaves turn red, orange and gold and fall until the broadleaf trees stand bare beside the green pine and cypress, snow settles on every tree and melts, and the leaves come back green](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/seasons.gif)

*`cargo run --release --example capture_growth -- --seasons`: the grove from late summer through autumn, leaf fall, snow and winter into spring, driven by `FloraSeason::of_year`. The deciduous presets carry an autumn colour; the pine and the cypress are evergreen.*

## Compatibility

| `bevy_flora` | `bevy` | Rust |
|---|---|---|
| 0.19 | 0.20 | 1.97.1+ (edition 2024) |

## Install

```toml
[dependencies]
bevy_flora = "0.19"
```

| feature | default | what it does |
|---|---|---|
| `textures` | yes | The bundled CC0 bark and leaf scans (about 3.6 MB of JPEG compiled into the crate). Without it every scan is absent: wood is plain colour and leaves keep their procedural outline |
| `serde` | — | `Serialize` / `Deserialize` on every parameter type and on `Scatter`, so trees and forests can be authored in RON or JSON. A tree's fields may be left out (they keep their defaults); an unknown field is an error |
| `ron` | — | `serde`, plus the `.tree.ron` asset loader and `TreeParams::to_ron` / `from_ron` (see [Trees from files](#trees-from-files-proceduraltreeasset-and-treeron)) |
| `file_watcher` | — | Bevy's `file_watcher`: assets, `.tree.ron` trees included, reload when their files are saved. An app without an `assets` folder then logs a warning at startup |

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

### Trees from files: `ProceduralTreeAsset` and `.tree.ron`

`TreeParams` is an asset. Spawn `ProceduralTreeAsset(handle)` in place of `ProceduralTree(params)` and the plugin gives the entity its `ProceduralTree` once the asset has loaded, and replaces it whenever the asset changes, so every tree that shares the handle follows an edit, made in code (`Assets<TreeParams>::get_mut`), in an inspector (the asset is registered for reflection) or in a file. With the `ron` feature the plugin loads `.tree.ron` files, and with Bevy's `file_watcher` (this crate's `file_watcher` feature turns it on) saving one rebuilds its trees while the game runs, a few tenths of a second after the save: tune a tree without recompiling. `TreeParams::to_ron` writes any tree as a file to start from (`cargo run --example tree_file --features ron -- --export oak`). A file may leave fields out, which keep their defaults; a misspelt field is an error that gives the line and the fields that exist; and a file that fails to load keeps the last good tree.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FloraPlugin))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    // assets/trees/maple.tree.ron, written with `TreePreset::Maple.params().to_ron()` and edited.
    let maple: Handle<TreeParams> = assets.load("trees/maple.tree.ron");
    for x in [-8.0, 0.0, 8.0] {
        commands.spawn((ProceduralTreeAsset(maple.clone()), Transform::from_xyz(x, 0.0, 0.0)));
    }
}
```

![The maple preset as exported to a tree file, and the same file with three values changed: a fuller, lighter crown](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/tree_file.jpg)

*`cargo run --release --example tree_file --features ron` (add `file_watcher` to rebuild on save): left, `examples/assets/trees/maple.tree.ron` as exported; right, the same file with the branch density raised from 3 to 7, the leaf density from 4.5 to 7 and the leaf colour lightened.*

Pools are built from a value (`build_pool(assets.get(&handle).unwrap(), ..)`), so a pool does not follow later edits to the asset.

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

Shadows follow the growth (the prepass shares the vertex shader), and the grown frames match the CPU path: over all 357 gallery scenarios the mean difference between the two renders is 0.07 of 255 (48 PNGs are byte-identical, the worst is a toon maple at 1.31). What differs, and why, is listed on `GpuGrowth` in the docs: smoothing is computed once on the mature mesh and deformed, side counts stay at the mature count, `AllBranches` leaves keep their mature positions, and the `Mesh` assets (picking, physics, culling) are the mature tree. It needs storage buffers in the vertex stage, so not WebGL2; there the trees grow on the CPU.

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

Bark (0.7.0) and leaf scans (0.8.0) leave these figures where they were, within what could be measured: the machine these were taken on has been loaded by other builds since, so the table above was not re-measured, and the releases were compared by interleaved runs instead (see the `CHANGELOG`). In those, 100 000 pooled oaks cost no more in 0.8.0 than in 0.7.0 (0.6.2 and 0.7.0 took 7.41 and 7.45 ms), and a thousand individual `ProceduralTree` oaks cost 4 to 10% more, because a leaf card cut to a real outline has more vertices than the old fan. The looks of 0.9.0 do not move them: the natural look draws the same pixels as 0.8.0 with the same shaders, and in interleaved runs 100 000 pooled oaks took 12 to 16 ms in all three looks on a machine under load, a thousand individual ones 31 to 48. Seasons (0.11.0) add under 1%: 100 000 pooled oaks took 7.33 ms in 0.10.0 and 7.40 in 0.11.0, a thousand individual oaks 23.4 and 23.6 ms (medians of three interleaved runs on a quiet machine), and half-bare trees are cheaper (22.1 ms).

The pool's trees are static: they have the age the pool was built at, and growing trees stay `ProceduralTree` (with `GpuGrowth`), which suits the few that are close. Impostors are baked once per variant by a few frames of offscreen rendering, so a tree appears when that is done, and they need `FloraPlugin` added after `DefaultPlugins` (before it, with no renderer, or on a device without storage buffers in the vertex stage such as WebGL2, the coarsest level is drawn at every distance and one warning says so). They are baked unlit, as a colour and a surface picture, and shaded by the scene's own lights every frame: they follow the time of day, glow with the sun behind them and sway with the wind. They cast no shadows, and they show one of eight views, at one elevation. With MSAA an impostor keeps the gaps of its crown: each pixel draws the share of its samples the baked tree covers, so a far crown is as airy, and as light, as the leaves it stands for. Level 2 and the impostors replace thin twigs and most leaves, so a forest seen from above or close up should keep its near trees at level 0.

### Planting a forest: `Scatter`

A grid of trees reads as an orchard. `Scatter` spreads them the way a forest stands: trunks never closer than `spacing` and otherwise irregular (Poisson-disc sampling, Bridson 2007), species in groves about `grove_size` wide whose borders bend instead of salt and pepper, and every tree its own yaw, size (`scale`, 0.85 to 1.15 by default) and pool variant. `points_where` takes a density for each spot, 0 for a road, a lake or a village and a fraction for open woodland, and only ever removes trees, so carving a clearing never moves the trees around it. It is pure CPU and deterministic per seed: 90 000 trees in groves take 0.2 s (Apple M4 Pro), once.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

fn plant_forest(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut pools: ResMut<Assets<TreePool>>) {
    let lod = LodSettings::default();
    let species: Vec<Handle<TreePool>> = [TreePreset::Oak, TreePreset::Birch, TreePreset::Pine]
        .iter()
        .map(|preset| pools.add(build_pool(&preset.params(), 8, f32::INFINITY, &lod, &mut meshes)))
        .collect();
    let forest = Scatter {
        species: vec![3.0, 2.0, 1.0], // three oaks to two birches to one pine
        grove_size: 60.0,
        ..Scatter::new(Rect::new(-500.0, -500.0, 500.0, 500.0), 7.0)
    };
    // A road along the x axis, kept clear.
    for tree in forest.points_where(|at| if at.y.abs() < 6.0 { 0.0 } else { 1.0 }) {
        commands.spawn((
            PooledTree { pool: species[tree.species].clone(), variant: tree.variant },
            tree.transform(0.0), // on terrain: `tree.transform_on(&ground, &params)`, below
        ));
    }
}
```

`forest --scatter oak:3,birch:2,maple:2,pine:1 --grove 60 --clearing 25` plants one, here in autumn so the groves show:

![A scattered forest in autumn seen from above one edge: groves of scarlet maples, russet oaks, pale birches and green pines with bending borders, around a round clearing](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/scatter.jpg)

*`cargo run --release --example forest -- --trees 3000 --spacing 7 --scatter oak:3,birch:2,maple:2,pine:1 --grove 60 --clearing 25 --season 1,0 --eye 0,45,-200 --at 0,0,-20 --wind 0 --deterministic --screenshot scatter.png`*

Undergrowth is a second, closer scatter that keeps clear of the trunks, drawn from the `Shrub` preset (a mound of arching stems about 1.4 m high and 1.9 m across: 5 900 wood and 17 600 leaf triangles up close, 2 100 in all at its coarsest level):

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

// Shrubs at least 2.5 m apart and 1.5 m from every trunk. For a big forest, look the trunks up in a
// grid instead of all of them (the `forest` example does).
fn plant_undergrowth(commands: &mut Commands, trees: &[ScatterPoint], area: Rect, shrubs: &Handle<TreePool>) {
    let layer = Scatter { seed: 2, ..Scatter::new(area, 2.5) };
    for shrub in layer.points_where(|at| if trees.iter().any(|t| t.position.distance(at) < 1.5) { 0.0 } else { 1.0 }) {
        commands.spawn((PooledTree { pool: shrubs.clone(), variant: shrub.variant }, shrub.transform(0.0)));
    }
}
```

![Eye level in a birch grove with oaks beyond: shrubs stand between the trunks in thickets and gaps](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/undergrowth.jpg)

*`cargo run --release --example forest -- --trees 1500 --spacing 7 --scatter oak:3,birch:2,maple:2 --grove 30 --clearing 14 --undergrowth shrub:2.5 --eye -14,1.8,-30 --at 6,1,0 --fov 55 --sun 20,25,-30 --wind 0 --deterministic --screenshot undergrowth.png`: the example's undergrowth also thins out in patches about 30 m across.*

A shrub stands about a tenth as tall as a tree, so its pool can hand over to coarser levels much nearer: build it with half the default distances, `LodSettings { distances: LodDistances { lod1: 15.0, lod2: 40.0, impostor: 100.0 }, ..default() }`, as the example does. In the forest above (1 372 trees and 4 863 shrubs, `--offscreen --deterministic`, the default camera), the trees alone take 2.7 ms a frame; with the shrubs at the trees' distances 7.9 ms, at half of them 3.3 ms (Apple M4 Pro).

### On terrain: `FloraGround`

A tree stood at the ground's height under its centre floats on a slope: the downhill side of its trunk hangs in the air over the ground, its flat base showing. `FloraGround` is the height of your ground at every point (x, z), your terrain's height function or a lookup into its heightmap, and `footing(at, radius)` is the lowest ground under a trunk's base, where a tree stands so that its trunk meets the ground all round. `TreeParams::base_radius()` is that base, flare and root ridges included, and `ScatterPoint::transform_on(&ground, &params)` does it for a scattered tree at its own scale. `slope(at)` and `normal(at)` keep trees off cliffs and screes.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

fn height(at: Vec2) -> f32 {
    6.0 * (at.x * 0.03).sin() * (at.y * 0.02).cos() // your terrain, or a heightmap lookup
}

fn plant_on_hills(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut pools: ResMut<Assets<TreePool>>) {
    let ground = FloraGround::new(height);
    let oak = TreePreset::Oak.params();
    let pool = pools.add(build_pool(&oak, 8, f32::INFINITY, &LodSettings::default(), &mut meshes));
    let forest = Scatter::new(Rect::new(-200.0, -200.0, 200.0, 200.0), 8.0);
    // Nothing on ground steeper than 30°.
    for tree in forest.points_where(|at| if ground.slope(at) > 30f32.to_radians() { 0.0 } else { 1.0 }) {
        commands.spawn((PooledTree { pool: pool.clone(), variant: tree.variant }, tree.transform_on(&ground, &oak)));
    }
    // Felled trees are cut above the uphill side of their trunk and land on the hills.
    commands.insert_resource(ground);
}
```

![Two oak trunks on a 25° slope, side by side: on the left, stood at the height under its centre, its flat base juts out over the downhill ground; on the right, stood at its footing, the trunk meets the slope all round](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/footing.jpg)

*`cargo run --release --example inspect -- --out target/footing --preset oak --mode tree --slope 25 --distance 4 --eye 0.6 --look 0.04` (right), with `--on-centre` (left).*

As a resource it is also the ground felling uses: the saw goes in `cut_height` above the trunk's uphill side, so a stump on a slope is taller on its downhill side, and a felled tree comes to rest where its trunk or limbs meet the terrain, turning further than on flat ground when it falls down the slope and less when it falls against it. Without it the ground is flat at each tree's own height, as before. The crate does not draw the ground: `FloraGround` is only what trees stand and fall on, so it must match the terrain you draw. Branches and leaves are not bent to the slope: on steep ground a spruce's lowest whorls and a willow's strands on the uphill side run into the hillside.

![A wooded valley of rolling hills from a clearing: oaks, maples, birches and pines in groves climb the slopes, shrubs between them](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/terrain.jpg)

*`cargo run --release --example forest -- --trees 3000 --spacing 7 --scatter oak:3,birch:2,pine:2,maple:1 --grove 50 --clearing 30 --terrain 14 --undergrowth shrub:3 --eye 0,2,0 --at -80,8,0 --fov 50 --wind 0.3 --deterministic --screenshot terrain.png`: `--terrain 14` draws hills about 14 m high and plants every tree and shrub at its footing, none on ground steeper than 30°.*

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

Trunks and branches wear a bundled CC0 bark scan (a colour map and a normal map, 512 px wide, about 3.1 MB for all 14). Each preset picks the scan of its species (`TreeParams::bark`); `BarkParams` sets how it follows the wood's growth. The scan is laid out in metres on the *mature* tree, so a growing tree's bark never slides; wood thinner than `smooth_below` is smooth, and by `fissured_above` it shows the whole scan, so a young branch gains relief as it thickens. The scan's average colour is divided out of the material's base colour, so a textured tree keeps its preset colour, and far trees, impostors and the plain `StandardMaterial` fallback look as they always did.

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

The textures are compiled in by the default `textures` feature (about 3.6 MB of JPEG for the bark and the leaf scans); with `default-features = false` every scan is absent, wood is plain colour and leaves keep their procedural outline. Their sources and licences (all CC0 1.0, no attribution required) are in [`src/textures/CREDITS.md`](src/textures/CREDITS.md).

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

### Looks: natural, painterly, toon

`TreeParams::style` (`RenderStyle`) changes how a tree is lit, not its shape; every preset is `Natural`. `Painterly` gives each leaf the normal that points out of the centre of its own clump (the leaves of one primary branch, the leader or one half of a split) instead of out of the crown's centre, so a crown reads as a cluster of rounded masses, each with a lit side and a shaded side; the lighting is the same physically based lighting as `Natural`. `Toon` keeps those normals, cuts the light of each directional light into three bands (0.15, 0.55 and 1 at `N·L` of 0 and 0.5) times its shadow and the surface colour, keeps the light that comes through backlit leaves, and darkens every surface toward its silhouette with an ink rim. Point and spot lights do not light a toon. Pools, impostors, `GpuGrowth` and growing trees draw them all; without the renderer's plugins (the plain-material fallback) a toon is lit as a painterly tree.

```rust,no_run
use bevy_flora::{RenderStyle, TreePreset};

fn painted_maple() -> bevy_flora::TreeParams {
    let mut params = TreePreset::Maple.params();
    params.style = RenderStyle::Painterly;
    params
}

fn toon_oak() -> bevy_flora::TreeParams {
    let mut params = TreePreset::Oak.params();
    params.style = RenderStyle::Toon;
    params
}
```

### Seasons

`FloraSeason` is a global resource like `FloraWind`: how far the leaves have turned (`autumn`, 0 summer green to 1 every leaf in its autumn colour), how many have fallen (`leaf_fall`, 0 to 1, bare) and how much snow lies on the trees (`snow`, 0 to 1). The deciduous presets (oak, willow, birch, bonsai, maple, poplar, baobab and Japanese maple) carry an autumn colour, `FoliageParams::autumn`; the evergreens have `None` and keep their needles. Leaves turn one at a time, each at its own moment, and fall in the order they turned. A fallen leaf is dropped in the vertex shader, so it leaves the shadows with it, and a leaf keeps its moment as the tree grows, on `GpuGrowth`, and on a pool's levels of detail; an impostor turns in patches of a few texels and thins its crown's coverage. Snow settles on every tree, first on what faces straight up and, as more falls, on steeper faces: the tops of branches, and on leaf and needle cards the side the camera sees when that side faces up (a tier of needles is white from above and green from below), as much as the leaf sees of the sky, with a light dusting on every exposed needle so a snowy conifer reads as snowy from below too. Impostors take it by their baked normals. The season shares the wind's buffer, so changing it every frame writes 96 bytes in place and touches no material: a year costs nothing per tree.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

// A year every two minutes: green until 0.55 of the way through, turning to 0.75, falling from
// 0.7 to 0.9, snowing from 0.88 to 0.92, melting from 0.98 and in leaf again by 0.05.
fn seasons(time: Res<Time>, mut season: ResMut<FloraSeason>) {
    *season = FloraSeason::of_year(time.elapsed_secs() / 120.0);
}

// Or set it outright: `FloraSeason::AUTUMN` is every leaf turned, `WINTER` bare trees and `SNOW`
// bare trees and every tree under snow.
fn first_frost(mut season: ResMut<FloraSeason>) {
    *season = FloraSeason::AUTUMN;
}

// A preset's autumn colour, or one for a tree of your own.
fn golden_oak() -> TreeParams {
    let mut params = TreePreset::Oak.params();
    params.foliage.autumn = Some(Color::srgb(0.85, 0.6, 0.1));
    params
}
```

A scattered winter forest under `FloraSeason::SNOW`: the spruces and pines keep their needles with snow on the tiers that face up, the oaks and birches stand bare with snow along their branches (the white ground is the example's, not the crate's):

![A winter forest: dark spruces and pines with snow on the upper sides of their branch tiers, bare oaks and pale birches, on white ground](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/snow.jpg)

*`cargo run --release --example forest -- --trees 3000 --spacing 7 --scatter spruce:3,pine:2,birch:2,oak:1 --grove 50 --clearing 25 --season 1,1,1 --eye 25,9,-75 --at 0,6,-20 --fov 50 --wind 0 --deterministic --screenshot snow.png`*

### Trees that react: `TreeShakes`

A game hits trees. `TreeShakes` is a resource like `FloraWind`: `shake(TreeShake { at, radius, strength, direction, duration }, &time)` swings every tree whose trunk stands within `radius` metres of `at` (world x and z), the nearest the most, first away from `at` (or along `direction`), then back and forth, dying away over `duration` seconds, while its leaves rustle. About a metre of radius singles out the tree that was hit; tens of metres reach a whole grove. Each tree swings at its own pace, taller trees slower, and the whole tree bends as it does in the wind, on every way a tree is drawn: `ProceduralTree`, `GpuGrowth`, pooled trees and their impostors. Up to eight shakes run at once (`TreeShakes::MAX`; a new one takes the place of the one that ends soonest, so a knock always shows). They live in the wind's shared buffer, so a shake writes a few hundred bytes and touches no material, and while none is under way the shaders compare one number a vertex. A shake is drawn, not simulated: colliders (`TreeShape`) stay where they are.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

// An axe on the tree standing at `trunk`, swung along `swing`: a metre of radius singles it out.
fn axe_hit(shakes: &mut TreeShakes, time: &Time, trunk: Vec3, swing: Vec3) {
    let hit = TreeShake { at: trunk.xz(), radius: 1.0, strength: 2.0, direction: swing.xz(), ..default() };
    shakes.shake(hit, time);
}

// A blast: every tree within 30 m swings away from it.
fn blast(shakes: &mut TreeShakes, time: &Time, at: Vec3) {
    shakes.shake(TreeShake { at: at.xz(), radius: 30.0, strength: 3.0, duration: 3.5, ..default() }, time);
}

#[derive(Component)]
struct Player;

// A character pushing through a thicket: a small knock where it is, a few times a second.
fn brush(mut shakes: ResMut<TreeShakes>, time: Res<Time>, player: Single<&Transform, With<Player>>, mut wait: Local<f32>) {
    *wait -= time.delta_secs();
    if *wait <= 0.0 {
        *wait = 0.3;
        let knock = TreeShake { at: player.translation.xz(), radius: 2.0, strength: 0.6, duration: 1.5, ..default() };
        shakes.shake(knock, &time);
    }
}
```

![The grown grove from a still camera: an axe hit swings the oak alone, another swings a birch, then a blast swings every tree outward and they settle back](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/shake.gif)

*`cargo run --release --example capture_growth -- --shake`: an axe on the oak, one on a birch, then a blast at the grove's middle, in a light wind.*

### Felling: `FellTree`

Insert `FellTree { direction, cut_height, speed }` on a tree and it comes down. The plugin cuts its wood at `cut_height`: what stays becomes a `TreeStump` entity, and what falls keeps the tree's entity, both closed by a cut face that shows the end grain (pale sapwood, rings, the pith and a few checks). The tree tips over toward `direction` about the front edge of the cut, as a felled trunk turns on its hinge, slowly at first and then faster as a rod of its height would (the oak below lands about 2.7 seconds after the cut), until its trunk or one of its big limbs reaches the ground; it lands there with a small bounce, thin branches and leaves on the near side giving way into the ground. `TreeLanded` is sent the frame it hits, with the point it hit, for the crash, the dust and the camera shake, and the landing shakes the crowns of the trees it fell through and rustles its own leaves. `Felled` says how far it has tipped and when it lies still. A `PooledTree` becomes the `ProceduralTree` of its variant before it is cut (the same tree as the pool's finest level), and a `GpuGrowth` tree is moved to the CPU path first, so any tree in a forest can be felled. On a hillside, a `FloraGround` resource (see "On terrain" above) is the ground it is cut above and lands on.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

// The player's axe finished the job: the tree falls away from them.
fn fell(commands: &mut Commands, tree: Entity, player: Vec3, trunk: Vec3) {
    let away = (trunk - player).xz();
    commands.entity(tree).insert(FellTree { direction: away, ..default() });
}

// When it lands: a crash and dust where it hit; a minute later, swap it for logs.
fn crash(mut landed: MessageReader<TreeLanded>) {
    for landing in landed.read() {
        info!("tree {:?} hit the ground at {}", landing.tree, landing.at);
    }
}
```

Once it has landed the tree no longer sways (its leaves still flutter), it is not rebuilt if its `ProceduralTree` or `TreeAge` changes, and it stays until you despawn it or buck it (below); its `TreeShape`, if it has one, turns with it and is not cut. There is no physics: the fall is drawn, as a shake is.

![An oak among a few trees is cut, leans, falls to the left with gathering speed, lands on its limbs with a small bounce and lies there by its stump](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/fell.gif)

*`cargo run --release --example capture_growth -- --fell`*

Insert `BuckTree { log_length, min_radius }` on a felled tree, with its `FellTree` or later, and once it lies still the plugin bucks it: its trunk is cut into logs `log_length` long (2 m by default), which lie on the ground in a row from the stump, the way the tree fell, on the `FloraGround` if there is one, each with its bark and end grain at both ends; the tree's entity and its crown are gone. The logs stop where the trunk gets thinner than `min_radius` (8 cm). Each is a `TreeLog` entity: `index` counts up the trunk from the butt, and `capsule` is a `WoodCapsule` in its own space, for its collider. `TreeBucked { tree, stump, logs }` is sent the same frame.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

// Fell the tree away from the player, and buck it into 2 m logs once it lies still.
fn chop_down(commands: &mut Commands, tree: Entity, away: Vec2) {
    commands.entity(tree).insert((FellTree { direction: away, ..default() }, BuckTree::default()));
}

// A collider for every new log, for the player to pick up.
fn log_colliders(logs: Query<&TreeLog, Added<TreeLog>>) {
    for log in &logs {
        let (capsule, place) = log.capsule.to_primitive(); // in the log's own space
        info!("log {}: {:.2} m across, its middle at {}", log.index, 2.0 * capsule.radius, place.translation);
    }
}
```

![An oak bucked into three logs lying in a row from its stump, the pale end grain showing in the gaps between them](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/logs.jpg)

*`cargo run --release --example inspect -- --out target/logs --preset oak --mode tree --fell 0 --buck 1.5 --distance 11 --eye 3.2 --look 0 --yaw 25 --fov 40 --sun 25,22,-15 --size 1600x900`, cropped.*

### Gameplay: colliders, growth events, the prelude

A game wants a tree's shape in a handful of primitives, not twenty thousand triangles. Insert `TreeShape::default()` next to a `ProceduralTree` (or a `PooledTree`) and the plugin keeps it up to date: three capsules up the trunk that taper with it (the lowest has the trunk's radius at hip height, so a walking character meets what it sees), one capsule for every stretch of wood at least `limb_radius` thick (6 cm by default; `TreeShape::with_limb_radius`), and a box around the leaves, all in the tree's local space. A growing tree's shape grows with it on both the CPU and the `GpuGrowth` path, and it is written only when it changes, so `Changed<TreeShape>` is the cue to rebuild colliders. `WoodCapsule::to_primitive` gives Bevy's `Capsule3d` and where it sits, which is what most physics engines' capsule colliders take. Building the shape costs about as much as ageing the skeleton (6 to 9 microseconds a tree on an Apple M4 Pro), which is why it is opt-in. `TreeMature` is sent once when a `Growing` tree reaches maturity, and `use bevy_flora::prelude::*;` brings in the plugin, the components, the presets and parameters, the wind and the shapes.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::prelude::*;

fn plant(mut commands: Commands) {
    commands.spawn((
        ProceduralTree(TreePreset::Oak.params()),
        TreeAge(0.0),
        Growing::default(),
        TreeShape::default(), // kept up to date while the oak grows
    ));
}

// Hand the capsules to your physics engine whenever the shape changes.
fn colliders(trees: Query<(Entity, &TreeShape), Changed<TreeShape>>) {
    for (tree, shape) in &trees {
        for capsule in shape.capsules() {
            let (capsule, place) = capsule.to_primitive(); // Capsule3d along the wood
            info!("{tree}: a capsule of radius {:.2} m at {:?}", capsule.radius, place.translation);
        }
    }
}

fn on_mature(mut mature: MessageReader<TreeMature>, mut commands: Commands) {
    for message in mature.read() {
        commands.entity(message.entity).remove::<Growing>();
    }
}
```

`examples/showcase.rs` draws the shape over the tree (`C`), and `inspect --colliders` photographs it:

![An oak with its gameplay shape drawn over it: three tapering capsules up the trunk, capsules along the thick limbs, and a box around the crown](https://raw.githubusercontent.com/Ladvien/bevy_flora/main/docs/shape.jpg)

*`cargo run --release --example inspect -- --out target/shape --preset oak --mode tree --colliders --distance 13 --eye 2 --look 0.45 --yaw 30`*

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

- **16 presets**: oak, pine, willow, birch, palm, cypress, bonsai, maple, spruce, poplar, baobab, dragon tree, Japanese maple, dead tree, redwood, and a shrub for undergrowth. Each is a plain `TreeParams` value you can edit.
- **Growth over time.** Every branch has a birth age and a growth duration, derived from where it sits on the tree. A tree can be shown at any age, branches thicken after they lengthen, and leaves grow to full size without re-rolling or disappearing as the tree ages.
- **Deterministic.** Each branch draws randomness from its own seeded stream, so the same `seed` gives the same tree on every run, and aging never re-rolls branches or leaves that already exist.
- **Branch shape**: crown envelopes, phyllotaxis, apical dominance, Y-splits, recursive sub-branches, twist, and a gravity bend that sub-branches and leaves follow. Floor avoidance keeps even strongly drooping wood above the ground.
- **Closed wood.** Branches meet the trunk through flared collars and root segments, forks taper into their halves, and trunk tops end in a flat cap, a cone or a leader branch. Optional pipe-model radii, radius-adaptive ring counts and volume-preserving (Taubin) smoothing that keeps seams, branch ends and twig seats in place.
- **Leaves**: five styles (crossed planes, single quad, cluster sphere, star burst, needle cluster), three placements (terminal branches, all branches, tip clusters) and four orientations. Leaf cards are leaf-shaped (cut into the geometry, so there is no alpha-mask `discard`) and grow out of their twigs, crowns shade as one soft volume through crown-bent normals (`normal_bend`), and each leaf gets its own brightness (`color_variation`), hue (`hue_variation`) and share of the sky (`occlusion`), all baked into the mesh. The plugin draws trees with `FloraMaterial`, a `StandardMaterial` with a leaf shader: a darker, glossier upper side, a paler matte lower side, veins, and light through a leaf lit from behind (`LeafShading`); facing the sun, the gloss gives way to that glow, so a backlit crown stays green instead of mirroring the sun white. The meshes also draw with a plain `StandardMaterial`, without those.
- **Wind.** `FloraWind` bends every tree and flutters its leaves in the vertex shader, shadows and impostors included, with a travelling `GustFront`, and each species answers in its own way (`TreeParams::wind`).
- **Bark.** Trunks and branches wear one of 14 bundled CC0 bark scans (colour and normal map; `TreeParams::bark`, `BarkScan`), laid out in metres on the mature tree so a growing tree's bark never slides, smooth on young wood and fissured as a branch thickens (`BarkParams`), and averaging the preset colour, so far trees, impostors and the plain-material fallback keep their colour. Compiled in by the default `textures` feature.
- **Leaf scans and grown veins.** Seven presets cut their leaf cards to the real outline of a bundled CC0 leaf scan and wear its colour and normal map (`FoliageParams::scan`, `LeafScan`); every other leaf-shaped card has veins grown by space colonisation (`venation`), drawn by the leaf shader from one small shared image.
- **Looks.** `TreeParams::style` draws a tree as `Natural`, `Painterly` (leaf normals point out of each clump's own centre: rounded masses) or `Toon` (banded light and an ink rim); impostors and pools follow.
- **Seasons.** `FloraSeason` turns deciduous trees to their autumn colour (`FoliageParams::autumn`), sheds their leaves one by one and lays snow on every tree, shadows, pools, impostors and `GpuGrowth` included, at no cost per tree; `FloraSeason::of_year` drives a whole year from one number.
- **Gameplay shapes.** `TreeShape`: capsules for the trunk and limbs and a box around the crown, kept up to date as a tree grows (CPU or GPU) and filled on pooled trees, ready to become colliders in any physics engine; `TreeMature` announces the moment a growing tree is done.
- **Trees that react.** `TreeShakes` swings the trees near a hit, a passing character or a blast and rustles their leaves, on every way a tree is drawn, for one small buffer write and nothing per tree while nothing shakes.
- **Felling.** `FellTree` cuts a tree, pooled ones included: it falls about the hinge of the cut, lands on its limbs with a bounce (`TreeLanded`), and leaves a `TreeStump`; both cut faces show end grain. `BuckTree` then cuts its trunk into `TreeLog`s lying on the ground, each with a capsule for its collider.
- **Trees from files.** `TreeParams` is an asset: `ProceduralTreeAsset` draws a tree from a handle, and every tree sharing it follows edits; with the `ron` feature trees load from `.tree.ron` files and rebuild when the file is saved.
- **Forest scale.** `TreePool` builds a few variants of a tree once, each at three levels of detail made from the skeleton (thin wood dropped, fewer sides and rings, fewer and larger leaves) and a baked camera-facing impostor for the far field; `PooledTree` places them. 100 000 oak trees draw at 7.7 ms a frame, leaf shading, wind and seasons included (see above). A forest of individual `ProceduralTree`s is fine for a few thousand trees, because trees with equal looks share their materials (1 000 oaks: 24 ms a frame), but each keeps its own meshes, so use a pool beyond that.
- **Planting forests.** `Scatter` says where the trees stand: trunks at least a spacing apart and otherwise irregular (Poisson disc), species in groves with bending borders, each tree its own yaw, size and variant, and a density per spot for roads and clearings; deterministic per seed. A second scatter of the `Shrub` preset, kept clear of the trunks, makes the undergrowth.
- **Terrain.** `FloraGround` is your ground's height: trees stand at their footing, where the trunk meets a slope all round (`ScatterPoint::transform_on`), `slope` keeps them off cliffs, and felled trees are cut above the uphill side and land on the hills.

## Examples

| example | what it shows |
|---|---|
| `showcase` | An interactive growing tree. Right-drag / wheel orbits, `[` `]` cycle presets, `N` reseeds, `Space` toggles growth, `R` restarts, `M` jumps to mature, `G` rebuilds, `U` switches between growing in the vertex shader (`GpuGrowth`) and rebuilding on the CPU, `C` draws the gameplay shape, `Y` runs the year (`FloraSeason::of_year`, 40 s a year), `H` hits the tree (`TreeShakes`), `F` fells it (`FellTree`) and then `B` bucks it into logs (`BuckTree`); any other key brings a new one. |
| `gallery` | Renders 399 scenarios (every preset at several ages and seeds, forced splits, smoothing, pipe radii (mature and young) and a wobbly twisted trunk on every preset, every leaf style × placement × orientation, every crown shape, every preset painterly and toon, parameter extremes, four deciduous presets and the pine turning, turned, half bare, bare and under snow) offscreen to four-view PNGs plus a `stats.tsv` with mesh sanity flags. This is the visual QA harness. `--filter <substr>` narrows it; `--gpu` grows every tree in the vertex shader. |
| `compare_gallery` | Compares two renders directory against directory (CPU `gallery` against `gallery --gpu`, or two `capture_growth` runs): mean difference and share of close pixels per PNG, non-zero exit past the limits. |
| `inspect` | Photographs one tree under controlled conditions, to see how it reads: a camera that backs off through every level of detail and into the impostor (`--distance a,b --frames n`), optionally zooming so the tree keeps its size on screen and a level switch shows as a jump (`--constant-size`), any bearing, height and field of view, one level forced at every distance (`--level`), the empty scene (`--empty`) to measure the tree's pixels against, with or without MSAA, in any look (`--style natural|painterly|toon`, which `forest` and `capture_growth` take too) and season (`--season <autumn>,<leaf fall>[,<snow>]`, which `forest` takes too), felled (`--fell <bearing>`: the photographs wait until it lies still) and bucked (`--buck <m>`: into logs `m` metres long), or on a slope (`--slope <deg>`, the ground falling toward +x: the tree at its footing, or with `--on-centre` where a plain placement would put it). The clock is fixed, so runs repeat. |
| `capture_growth` | Records the clips above as numbered PNG frames: one tree growing (`--preset <name>`, default maple), with `--grove` a grove of mixed species with a gust front crossing it, with `--seasons` the same grove fully grown through a year, with `--shake` the grove knocked by `TreeShakes`, or with `--fell` an oak felled among a few trees (`--gpu` grows the trees in the vertex shader). The clock steps a fixed 1/24 s per frame, so the wind sways smoothly and two runs write the same frames; the `ffmpeg` commands that make the GIFs are in the example's docs. |
| `forest` | The forest-scale benchmark: up to 100 000 trees under a shadow-casting sun, on a grid or, with `--scatter oak:3,birch:1,...`, where `Scatter` puts them (`--grove <m>`, `--clearing <m>`; `--undergrowth shrub:2.5` adds a patchy layer of shrubs 2.5 m apart and clear of the trunks), on flat ground or on rolling hills (`--terrain <relief>`: every tree at its footing on a `FloraGround`), drawn from a `TreePool` per species (`--mode pool`, the default), as a `ProceduralTree` each (`--mode naive`), or as `GpuGrowth` trees, fully grown or growing for the whole run (`--mode gpu`, `gpu-live`; `--mode cpu-live` is the CPU-rebuilt equivalent), timed per frame or saved as a screenshot. `--offscreen` times the real frame cost without a window and the result line gives the mean, p95, median, minimum and CPU time per frame; `--no-impostors`, `--lod` and `--shadows` change the setup; `--sun <x,y,z>`, `--wind <strength>`, `--wind-dir <x,z>`, `--gust <speed>` and `--wind-animate` (the strength changes every frame) move the light and the weather, `--shake-animate` knocks the trees every few frames (every shake slot busy), `--season <autumn>,<leaf fall>[,<snow>]` sets the season; `--deterministic` fixes the clock at 1/60 s, so two builds' `--screenshot`s compare byte for byte. |
| `tree_file` | (`--features ron`) A tree from `examples/assets/trees/maple.tree.ron` (`--tree <path>` for another file); with `file_watcher` too it rebuilds when the file is saved, and a load error shows on screen while the last good tree stays. `--export <preset>` prints a preset as a tree file, `--screenshot <path>` saves the tree and exits. |

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
