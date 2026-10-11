//! The mature tree, tagged for growing in the vertex shader.
//!
//! The wood and leaf meshes are the ones the CPU path builds for the mature
//! tree (smoothing included), with two differences: the wood always carries the
//! trunk's top cap and, for a pointed trunk, its tip cone (the shader hides
//! whichever the age does not have), and every vertex gets a [`Tag`] recording
//! the node it belongs to, which formula places it and where it sits relative to
//! its anchor. The offsets are measured on the final (smoothed) positions, so
//! smoothing rides along as a deformation.

use std::f32::consts::TAU;

use bevy::math::Vec3;
use bevy::mesh::Mesh;

use super::frame::{Class, Tag, placement, twisted};
use super::state::GrowthState;
use crate::foliage::{foliage_mesh_with, leaf_points_for_node};
use crate::mesh::MeshData;
use crate::mesh::branch::{WoodPiece, WoodSpan, skeleton_mesh_with, tube_rings};
use crate::mesh::trunk::{TrunkPieces, root_bulge_unit, trunk_ring_count};
use crate::params::{FoliagePlacement, TreeParams, TrunkTermination};
use crate::pipeline::aged_skeleton;
use crate::skeleton::Skeleton;

/// Offsets smaller than this scale are not measured: the vertex sits on its anchor.
const MIN_SCALE: f32 = 1e-6;

/// The tagged mature meshes and the state they were measured against.
#[derive(Clone, Debug)]
pub(crate) struct GrowthData {
    /// Wood: trunk, collars, tubes, bases.
    pub wood: MeshData,
    /// One tag per wood vertex.
    pub wood_tags: Vec<Tag>,
    /// Which node and piece each run of wood vertices belongs to.
    #[cfg_attr(not(test), allow(dead_code))]
    pub wood_spans: Vec<WoodSpan>,
    /// Leaves; empty without foliage.
    pub leaves: MeshData,
    /// One tag per leaf vertex.
    pub leaf_tags: Vec<Tag>,
    /// The mature state the offsets were measured against.
    pub state: GrowthState,
}

/// Measure `position` against `tag`'s anchor and scale under `state`.
fn measure(tag: &mut Tag, position: Vec3, state: &GrowthState) {
    let p = placement(tag, state);
    tag.offset = if p.scale > MIN_SCALE { twisted((position - p.anchor) / p.scale, -p.twist) } else { Vec3::ZERO };
}

/// Build the tagged mature meshes of the tree `full` is the full skeleton of.
pub(crate) fn build_growth_data(full: &Skeleton, params: &TreeParams) -> GrowthData {
    let mature = aged_skeleton(full, params, f32::INFINITY);
    assert_eq!(mature.nodes.len(), full.nodes.len(), "the mature skeleton keeps every node");
    let mut state = GrowthState::default();
    state.update(full, params, f32::INFINITY);

    let pieces = TrunkPieces {
        cone: params.trunk.termination == TrunkTermination::PointedTip,
        top_cap: true,
    };
    let mut wood_spans = Vec::new();
    let wood = skeleton_mesh_with(&mature, params, pieces, Some(&mut wood_spans));
    let mut wood_tags = vec![Tag::default(); wood.positions.len()];
    for span in &wood_spans {
        tag_wood_span(span, &wood, &mature, params, &state, &mut wood_tags);
    }

    let (leaves, leaf_tags) = tag_leaves(&mature, params, &state);
    GrowthData {
        wood,
        wood_tags,
        wood_spans,
        leaves,
        leaf_tags,
        state,
    }
}

fn tag_wood_span(
    span: &WoodSpan,
    wood: &MeshData,
    mature: &Skeleton,
    params: &TreeParams,
    state: &GrowthState,
    tags: &mut [Tag],
) {
    let node = &mature.nodes[span.node];
    let first = span.first as usize;
    let count = span.count as usize;
    let sides = span.segments as usize;
    let ring_len = sides + 1;
    let tag = |class: Class, f: f32, axis: Vec3| Tag {
        node: node.id,
        class,
        axis,
        f,
        ..Tag::default()
    };

    match span.piece {
        WoodPiece::Trunk => return tag_trunk(span, wood, params, state, tags),
        WoodPiece::Tube => {
            let rings = tube_rings(node, params.branches.twist) as usize;
            assert_eq!(count, rings * ring_len + 1 + ring_len);
            let axis = node.axis(rings as u32);
            let on_axis = |k: usize| (axis[k].0 - node.start) / node.length;
            for k in 0..rings {
                let f = k as f32 / (rings - 1) as f32;
                for i in 0..ring_len {
                    tags[first + k * ring_len + i] = tag(Class::Tube, f, on_axis(k));
                }
            }
            for j in 0..1 + ring_len {
                tags[first + rings * ring_len + j] = tag(Class::Tube, 1.0, on_axis(rings - 1));
            }
        }
        WoodPiece::Collar => {
            assert_eq!(count, 3 * ring_len);
            for k in 0..3 {
                for i in 0..ring_len {
                    tags[first + k * ring_len + i] = tag(Class::Collar, k as f32 / 2.0, Vec3::ZERO);
                }
            }
        }
        WoodPiece::Root { collar } => {
            assert_eq!(count, 2 * ring_len);
            let (far, near) = if collar {
                (Class::RootFarCollar, Class::BaseCollar)
            } else {
                (Class::RootFar, Class::Base)
            };
            for i in 0..ring_len {
                tags[first + i] = tag(far, 0.0, Vec3::ZERO);
                tags[first + ring_len + i] = tag(near, 0.0, Vec3::ZERO);
            }
        }
        WoodPiece::Dome { collar } => {
            let class = if collar { Class::BaseCollar } else { Class::Base };
            for slot in &mut tags[first..first + count] {
                *slot = tag(class, 0.0, Vec3::ZERO);
            }
        }
    }
    for (slot, &position) in tags[first..first + count].iter_mut().zip(&wood.positions[first..first + count]) {
        measure(slot, position, state);
    }
}

fn tag_trunk(span: &WoodSpan, wood: &MeshData, params: &TreeParams, state: &GrowthState, tags: &mut [Tag]) {
    let trunk = &params.trunk;
    let sides = trunk.radial_segments.max(3) as usize;
    let ring_len = sides + 1;
    let rings = trunk_ring_count(trunk) as usize;
    let cone = trunk.termination == TrunkTermination::PointedTip;
    let unit = |i: usize| root_bulge_unit(trunk, i as f32 / sides as f32 * TAU);
    let tag = |class: Class, f: usize, extra: f32| Tag {
        class,
        f: f as f32,
        extra,
        ..Tag::default()
    };

    let mut at = 0;
    for k in 0..rings {
        for i in 0..ring_len {
            let slot = &mut tags[at + i];
            *slot = tag(Class::Trunk, k, unit(i));
            measure(slot, wood.positions[at + i], state);
        }
        at += ring_len;
    }
    if cone {
        let top = rings - 1;
        let apex = state.rings[top].center + Vec3::Y * state.cone_lift;
        for i in 0..ring_len {
            let top_ring_offset = tags[top * ring_len + i].offset;
            tags[at + i] = Tag {
                axis: top_ring_offset,
                offset: wood.positions[at + i] - apex,
                ..tag(Class::TrunkCone, top, unit(i))
            };
        }
        at += ring_len;
    }
    // Bottom cap: the centre, then the rim, laid out like ring 0.
    for (class, f) in [(Class::Trunk, 0), (Class::TrunkTopCap, rings - 1)] {
        tags[at] = tag(class, f, 0.0);
        measure(&mut tags[at], wood.positions[at], state);
        for i in 0..ring_len {
            let slot = &mut tags[at + 1 + i];
            *slot = tag(class, f, unit(i));
            measure(slot, wood.positions[at + 1 + i], state);
        }
        at += 1 + ring_len;
    }
    assert_eq!(span.count as usize, at, "trunk mesh layout");
}

fn tag_leaves(mature: &Skeleton, params: &TreeParams, state: &GrowthState) -> (MeshData, Vec<Tag>) {
    let foliage = &params.foliage;
    if !foliage.enabled {
        return (MeshData::default(), Vec::new());
    }
    let mut points = Vec::new();
    // (node index, first point, one past the last point)
    let mut ranges = Vec::new();
    for i in 0..mature.nodes.len() {
        let before = points.len();
        leaf_points_for_node(mature, i, foliage, &params.growth, &params.trunk, params.seed, &mut points);
        if points.len() > before {
            ranges.push((i, before, points.len()));
        }
    }
    let mut firsts = Vec::new();
    let leaves = foliage_mesh_with(&points, foliage, Some(&mut firsts));
    let mut tags = vec![Tag::default(); leaves.positions.len()];
    for (i, from, to) in ranges {
        let node = &mature.nodes[i];
        for p in from..to {
            // A tip cluster hangs off the node's end; other leaves off the point they sit at.
            let anchor = if foliage.placement == FoliagePlacement::TipClusters {
                mature.end(i)
            } else {
                points[p].position
            };
            let vertices = firsts[p] as usize..firsts.get(p + 1).map_or(leaves.positions.len(), |&n| n as usize);
            for v in vertices {
                tags[v] = Tag {
                    node: node.id,
                    class: Class::Leaf,
                    ordinal: (p - from) as u32,
                    axis: (anchor - node.start) / node.length,
                    ..Tag::default()
                };
                measure(&mut tags[v], leaves.positions[v], state);
            }
        }
    }
    (leaves, tags)
}

/// Pack `tags` into the three vertex attributes and add them to `mesh`.
fn with_tags(data: MeshData, tags: &[Tag]) -> Mesh {
    let ids: Vec<[u32; 2]> = tags.iter().map(|t| [t.node, t.class as u32 | (t.ordinal << 8)]).collect();
    let axis: Vec<[f32; 4]> = tags.iter().map(|t| [t.axis.x, t.axis.y, t.axis.z, t.f]).collect();
    let offset: Vec<[f32; 4]> = tags.iter().map(|t| [t.offset.x, t.offset.y, t.offset.z, t.extra]).collect();
    data.into_bevy_mesh()
        .with_inserted_attribute(super::ATTRIBUTE_GROWTH_ID, ids)
        .with_inserted_attribute(super::ATTRIBUTE_GROWTH_AXIS, axis)
        .with_inserted_attribute(super::ATTRIBUTE_GROWTH_OFFSET, offset)
}

impl GrowthData {
    /// The wood and (when there are leaves) leaf meshes, with the growth attributes.
    pub fn into_meshes(self) -> (Mesh, Option<Mesh>) {
        let leaves = (!self.leaves.is_empty()).then(|| with_tags(self.leaves, &self.leaf_tags));
        (with_tags(self.wood, &self.wood_tags), leaves)
    }
}
