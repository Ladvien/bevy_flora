//! `bevy_flora`: parametric procedural trees that grow over time, for Bevy.
//!
//! Pipeline: [`TreeParams`] → [`generate_skeleton`] → [`Skeleton::aged`] →
//! wood mesh ([`mesh::branch::skeleton_mesh`]) + leaves
//! ([`foliage::collect_leaf_points`] / [`foliage::foliage_mesh`]) → [`MeshData`]
//! → Bevy `Mesh`. [`build_tree`] runs the whole thing; [`FloraPlugin`] drives it
//! from the [`ProceduralTree`] / [`TreeAge`] / [`Growing`] components.
//!
//! # Growing a tree in a Bevy app
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::{FloraPlugin, Growing, ProceduralTree, TreeAge, TreePreset};
//!
//! fn main() {
//!     App::new()
//!         .add_plugins((DefaultPlugins, FloraPlugin))
//!         .add_systems(Startup, setup)
//!         .run();
//! }
//!
//! fn setup(mut commands: Commands) {
//!     commands.spawn((
//!         ProceduralTree(TreePreset::Oak.params()),
//!         TreeAge(0.0),
//!         Growing::default(),
//!     ));
//! }
//! ```
//!
//! # Growing a forest: [`GpuGrowth`]
//!
//! Rebuilding a growing tree's meshes costs the CPU 0.1-0.4 ms per tree per
//! frame. Add [`GpuGrowth`] and the tree is built once, at maturity, and grown in
//! the vertex shader instead (a few microseconds of CPU per tree per frame, 0.004
//! ms in the 0.4.0 measurements):
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::{FloraPlugin, GpuGrowth, Growing, ProceduralTree, TreeAge, TreePreset};
//!
//! fn setup(mut commands: Commands) {
//!     for i in 0..200 {
//!         commands.spawn((
//!             ProceduralTree(TreePreset::Oak.params()),
//!             TreeAge(0.0),
//!             Growing::default(),
//!             GpuGrowth,
//!             Transform::from_xyz((i % 20) as f32 * 8.0, 0.0, (i / 20) as f32 * 8.0),
//!         ));
//!     }
//! }
//! ```
//!
//! It needs a device with storage buffers in the vertex stage (everything but
//! WebGL2; see [`GpuGrowthSupport`]); elsewhere the trees grow on the CPU.
//!
//! # Drawing a forest: [`TreePool`]
//!
//! Every [`ProceduralTree`] has meshes of its own, which a GPU draws a few thousand
//! of. For a forest build a [`TreePool`] (a few seed variants of one tree, each at
//! three levels of detail from [`build_lods`]) and place [`PooledTree`]s from it:
//! trees that share a mesh are drawn in one instanced call, the distance to the
//! camera picks the level, and beyond the last one a tree is a baked camera-facing
//! quad ([`ImpostorAtlas`]), shaded by the scene's own lights. 100 000 oaks draw at
//! 7.4 ms a frame (1080p, Apple M4 Pro).
//! Pooled trees are static; add [`FloraPlugin`] after `DefaultPlugins` to get the
//! impostors. See [`pool`] for an example.
//!
//! # Leaf shading and wind
//!
//! The plugin draws trees with [`FloraMaterial`]: a darker, glossier upper side of
//! every leaf, a paler lower side, veins and light through a leaf lit from behind
//! ([`LeafShading`]), and wind. The [`FloraWind`] resource sets the weather (a
//! [`GustFront`] sweeps a band of stronger wind across the world,
//! [`FloraWind::launch_gust`]) and [`TreeParams::wind`] ([`WindResponse`]) how each
//! species answers it.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::{FloraWind, GustFront};
//!
//! // Call this once, when the gust begins (a system run on an event or a timer), not
//! // every frame: launching restarts the front at its origin.
//! fn start_storm(mut wind: ResMut<FloraWind>, time: Res<Time>) {
//!     wind.strength = 0.6;
//!     wind.launch_gust(GustFront { speed: 20.0, ..default() }, &time);
//! }
//! ```
//!
//! # Bark
//!
//! Wood wears a bundled CC0 bark scan ([`BarkScan`], chosen per species by the presets in
//! [`TreeParams::bark`]). The scan is laid out in metres on the mature tree, so a growing tree's
//! bark never slides, and [`BarkParams`] fades it in as a branch thickens: smooth wood below
//! `smooth_below` metres of radius, the whole scan above `fissured_above`. The scan's average
//! colour is divided out of the base colour, so a textured tree keeps its preset colour. The
//! images are compiled in by the default `textures` feature (about 3.6 MB with the leaf scans);
//! without it every scan is absent and wood is plain colour.
//!
//! # Leaf scans and grown veins
//!
//! A [`FoliageParams::scan`] ([`LeafScan`]) cuts leaf cards to the real outline of a bundled leaf
//! scan (one of four leaves of an atlas, by a hash of the leaf's own random draws) and the leaf
//! shader wears the atlas's colour and normal map on them. Leaves without a scan keep the
//! procedural outline and get veins grown by space colonisation ([`venation`]), drawn from one
//! small image every such leaf shares. Both need the `textures` feature only for the scans: the
//! veins are grown at run time.
//!
//! # Looks: natural, painterly, toon
//!
//! [`TreeParams::style`] ([`RenderStyle`]) changes how a tree is lit, not its shape. `Painterly`
//! gives every leaf the normal that points out of the centre of its own clump (the leaves of a
//! primary branch, the leader or a split) rather than out of the crown's, so a crown reads as
//! rounded masses with a lit and a shaded side each. `Toon` keeps those normals and cuts the
//! light of every directional light into three bands, with an ink rim toward the silhouette. The
//! pools, the impostors and `GpuGrowth` draw them all.
//!
//! # Seasons
//!
//! The [`FloraSeason`] resource turns deciduous trees ([`FoliageParams::autumn`], set on the
//! oak, willow, birch, bonsai, maple, poplar, baobab and Japanese maple presets) to their autumn
//! colour and sheds their leaves, one leaf at a time; evergreens ignore it. It shares the wind's
//! buffer, so a season that changes every frame costs no material change and nothing per tree.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::FloraSeason;
//!
//! // A year every two minutes.
//! fn seasons(time: Res<Time>, mut season: ResMut<FloraSeason>) {
//!     *season = FloraSeason::of_year(time.elapsed_secs() / 120.0);
//! }
//! ```
//!
//! # Gameplay shapes
//!
//! A [`TreeShape`] on a tree (opt-in: insert `TreeShape::default()`) is kept up to date by the
//! plugin: capsules up the trunk and along every limb thicker than its `limb_radius`, and a box
//! around the crown, in the tree's local space, for colliders, picking or line of sight.
//! [`TreeMature`] is sent once when a [`Growing`] tree reaches maturity. The [`prelude`] has the
//! types a game usually needs.
//!
//! # Building meshes without Bevy's ECS
//!
//! ```rust
//! use bevy_flora::{TreeMeshes, TreePreset, build_tree};
//!
//! let params = TreePreset::Oak.params();
//! let meshes: TreeMeshes = build_tree(&params, f32::INFINITY);
//! assert!(!meshes.trunk.is_empty());
//! ```

pub mod crown;
pub mod foliage;
mod gpu;
pub mod growth;
pub mod impostor;
pub mod lod;
pub mod mesh;
pub mod params;
pub mod pipeline;
pub mod plugin;
pub mod pool;
pub mod presets;
pub mod rng;
pub mod shading;
pub mod skeleton;
pub mod shape;
mod textures;
pub mod venation;

pub use crown::CrownShape;
pub use foliage::LeafPoint;
pub use gpu::{GpuGrowth, GpuGrowthSupport, GrowthExtension, GrowthMaterial};
pub use growth::GrowthParams;
pub use impostor::{IMPOSTOR_VIEWS, ImpostorAtlas, ImpostorMaterial};
pub use lod::{LodDistances, LodSettings, build_lods};
pub use mesh::MeshData;
pub use params::{
    BarkParams, BarkScan, BranchParams, FoliageParams, FoliagePlacement, LeafOrientation, LeafScan, LeafShading,
    LeafStyle, MeshParams, RenderStyle, SplitParams, TreeParams, TrunkParams, TrunkTermination, WindResponse,
};
pub use pipeline::{TreeMeshes, build_tree};
pub use plugin::{FloraPlugin, Growing, ProceduralTree, TreeAge, TreeGenerated, TreeMature, TreeRender};
pub use pool::{PoolMaterials, PoolVariant, PooledTree, TreePool, build_pool};
pub use presets::TreePreset;
pub use rng::SeededRng;
pub use shading::{
    FloraExtension, FloraKey, FloraMaterial, FloraMaterialSupport, FloraSeason, FloraUniform, FloraWind,
    FloraWindBuffer, GustFront,
    SharedFlora,
};
pub use shape::{TreeShape, WoodCapsule};
pub use skeleton::{Node, NodeKind, Skeleton, generate_skeleton};

/// What a game usually needs, in one `use bevy_flora::prelude::*;`: the plugin, the tree, pool and
/// growth components, the presets and parameters, the wind, the season and the gameplay shape.
pub mod prelude {
    pub use crate::{
        BarkParams, BarkScan, FloraPlugin, FloraSeason, FloraWind, FoliageParams, GpuGrowth, Growing, GustFront,
        LeafScan,
        LodDistances, LodSettings, PooledTree, ProceduralTree, RenderStyle, TreeAge, TreeGenerated, TreeMature,
        TreeParams, TreePool, TreePreset, TreeShape, WindResponse, WoodCapsule, build_pool,
    };
}

// Compile the README's Rust examples as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
