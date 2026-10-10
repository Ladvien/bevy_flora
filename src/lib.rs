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
pub mod growth;
pub mod mesh;
pub mod params;
pub mod pipeline;
pub mod plugin;
pub mod presets;
pub mod rng;
pub mod skeleton;

pub use crown::CrownShape;
pub use foliage::LeafPoint;
pub use growth::GrowthParams;
pub use mesh::MeshData;
pub use params::{
    BranchParams, FoliageParams, FoliagePlacement, LeafOrientation, LeafStyle, MeshParams,
    SplitParams, TreeParams, TrunkParams, TrunkTermination,
};
pub use pipeline::{TreeMeshes, build_tree};
pub use plugin::{FloraPlugin, Growing, ProceduralTree, TreeAge, TreeGenerated, TreeRender};
pub use presets::TreePreset;
pub use rng::SeededRng;
pub use skeleton::{Node, NodeKind, Skeleton, generate_skeleton};

// Compile the README's Rust examples as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
