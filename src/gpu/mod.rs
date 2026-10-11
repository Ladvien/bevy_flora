//! Growing trees in the vertex shader.
//!
//! A tree with [`GpuGrowth`] is built once, at maturity, and uploaded once.
//! Every vertex carries which node it belongs to and where it sits relative to
//! that node's axis; each frame the CPU ages the skeleton ([`Skeleton::aged`])
//! and writes one small record per node into a storage buffer, and the vertex
//! shader (`growth.wesl`) moves every vertex to where the CPU path would have
//! put it. Nothing else is uploaded.
//!
//! [`Skeleton::aged`]: crate::Skeleton::aged

use bevy::ecs::prelude::*;
use bevy::mesh::{MeshVertexAttribute, VertexFormat};
use bevy::reflect::Reflect;

mod frame;
mod mesh;
mod state;

#[cfg(test)]
mod tests;

/// Per-vertex: the node (`Node::id`) in the low word, the placement class
/// (low 8 bits) and a leaf's ordinal (the rest) in the high word.
pub(crate) const ATTRIBUTE_GROWTH_ID: MeshVertexAttribute =
    MeshVertexAttribute::new("GrowthId", 0x666C_6F72_6101, VertexFormat::Uint32x2);
/// Per-vertex: the anchor on the node's axis in node lengths (`xyz`), and the
/// fraction along the piece or the trunk ring (`w`).
pub(crate) const ATTRIBUTE_GROWTH_AXIS: MeshVertexAttribute =
    MeshVertexAttribute::new("GrowthAxis", 0x666C_6F72_6102, VertexFormat::Float32x4);
/// Per-vertex: the offset from the anchor in units of the piece's scale (`xyz`),
/// and the trunk's unit root-ridge bulge (`w`).
pub(crate) const ATTRIBUTE_GROWTH_OFFSET: MeshVertexAttribute =
    MeshVertexAttribute::new("GrowthOffset", 0x666C_6F72_6103, VertexFormat::Float32x4);

/// Grow this tree in the vertex shader instead of rebuilding its meshes every
/// frame. Add it to an entity that has [`ProceduralTree`](crate::ProceduralTree)
/// and [`TreeAge`](crate::TreeAge) (and, to make the tree grow,
/// [`Growing`](crate::Growing)).
///
/// The mature tree is built once per change of [`ProceduralTree`](crate::ProceduralTree)
/// and uploaded once; each frame only a few kilobytes of per-node state are
/// written. A tree on this path costs the CPU microseconds per frame instead of
/// the 0.1-0.5 ms the CPU path takes, which makes growing a forest affordable.
///
/// Without this component a tree behaves exactly as before.
///
/// # What differs from the CPU path
///
/// The tree is the same at every age in its overall shape, but it is a
/// deformation of the mature mesh, not a rebuild:
///
/// - **Smoothing** ([`MeshParams::smooth_iterations`](crate::MeshParams::smooth_iterations))
///   is computed once, on the mature mesh, and deformed with the tree; the CPU
///   path smooths every age afresh.
/// - **Ring and side counts are the mature ones.** With
///   [`MeshParams::adaptive_resolution`](crate::MeshParams::adaptive_resolution)
///   a young thin branch keeps the sides of the mature branch.
/// - **Unborn parts have zero size**, instead of being absent; their triangles
///   collapse to a point and draw nothing.
/// - With [`FoliagePlacement::AllBranches`](crate::FoliagePlacement::AllBranches),
///   leaves keep their mature positions along a branch and the first ones appear
///   as it lengthens; the CPU path respaces them.
/// - Tip-cluster leaves that are mirrored away from the trunk
///   ([`FoliagePlacement::TipClusters`](crate::FoliagePlacement::TipClusters))
///   keep the mirroring the mature tree chose.
/// - Leaf normals and the shape of primary bases follow the mature tree.
/// - The [`Mesh`](bevy::mesh::Mesh) assets on the tree's children, and the
///   bounding box derived from them, are the **mature** tree at every age:
///   picking, physics and frustum culling see the grown size, not the current one.
/// - Growth does not feed motion vectors (temporal anti-aliasing sees a growing
///   branch as still).
///
/// # Platform support
///
/// The per-node state lives in a storage buffer, which WebGL2 does not have. On
/// such a device (and until the renderer is ready) the tree falls back to the
/// CPU path and a warning is logged once.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct GpuGrowth;
