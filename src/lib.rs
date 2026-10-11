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
//! 15 ms a frame (1080p, Apple M5).
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
//! fn gust(mut wind: ResMut<FloraWind>, time: Res<Time>) {
//!     wind.strength = 0.6;
//!     wind.launch_gust(GustFront { speed: 20.0, ..default() }, &time);
//! }
//! ```
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

pub use crown::CrownShape;
pub use foliage::LeafPoint;
pub use gpu::{GpuGrowth, GpuGrowthSupport, GrowthExtension, GrowthMaterial};
pub use growth::GrowthParams;
pub use impostor::{IMPOSTOR_VIEWS, ImpostorAtlas, ImpostorMaterial};
pub use lod::{LodDistances, LodSettings, build_lods};
pub use mesh::MeshData;
pub use params::{
    BranchParams, FoliageParams, FoliagePlacement, LeafOrientation, LeafShading, LeafStyle,
    MeshParams, SplitParams, TreeParams, TrunkParams, TrunkTermination, WindResponse,
};
pub use pipeline::{TreeMeshes, build_tree};
pub use plugin::{FloraPlugin, Growing, ProceduralTree, TreeAge, TreeGenerated, TreeRender};
pub use pool::{PoolMaterials, PoolVariant, PooledTree, TreePool, build_pool};
pub use presets::TreePreset;
pub use rng::SeededRng;
pub use shading::{FloraExtension, FloraKey, FloraMaterial, FloraUniform, FloraWind, FloraWindBuffer, GustFront};
pub use skeleton::{Node, NodeKind, Skeleton, generate_skeleton};

// Compile the README's Rust examples as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
