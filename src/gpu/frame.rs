//! Where a growing vertex sits: the formulas the vertex shader (`growth_common.wesl`)
//! mirrors, in Rust, so tests can check them against the CPU meshers.
//!
//! Every vertex of the mature mesh carries a [`Tag`]. At any age its position is
//! `anchor + scale * offset`, where the anchor and scale come from the node's
//! per-frame [`NodeState`](super::state::NodeState) (or the trunk's ring table)
//! and the offset was measured once on the mature mesh.

use bevy::math::Vec3;

use super::state::{BASE, BASE_COLLAR, COLLAR, GrowthState, TOP_CAP, WOOD};

/// Which formula places a vertex. The numbers are shared with the shader.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub(crate) enum Class {
    /// Branch tube ring or tip cap: on the node's axis, `lerp(base, tip, f)` wide.
    #[default]
    Tube = 0,
    /// Leaf card vertex: at a point on the node's axis, scaled by the leaf scale.
    Leaf = 1,
    /// Collar ring.
    Collar = 2,
    /// Primary base: dome, or the near ring of the extension into the trunk.
    Base = 3,
    /// Like `Base`, under the collar.
    BaseCollar = 4,
    /// Far ring of a primary's extension into the trunk.
    RootFar = 5,
    /// Like `RootFar`, under the collar.
    RootFarCollar = 6,
    /// Trunk ring or bottom cap.
    Trunk = 7,
    /// Trunk top cap.
    TrunkTopCap = 8,
    /// Mature pointed trunk's tip cone.
    TrunkCone = 9,
}

/// What the mature mesh records about one vertex.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Tag {
    /// `Node::id` of the node the vertex belongs to (0 for the trunk).
    pub node: u32,
    /// Placement formula.
    pub class: Class,
    /// Leaf's position among its node's leaves.
    pub ordinal: u32,
    /// Tubes and leaves: the vertex's anchor on the node's axis, in node
    /// lengths from the start. Tip cone: the offset the vertex has when the
    /// cone is hidden (it then coincides with the top ring).
    pub axis: Vec3,
    /// Tubes and collars: fraction along the piece. Trunk: ring index.
    pub f: f32,
    /// The vertex's offset from its anchor, in units of the scale. On the
    /// trunk it is measured with the ring's twist taken out.
    pub offset: Vec3,
    /// Trunk: unit root-ridge bulge at the vertex's angle.
    pub extra: f32,
}

/// Where a vertex goes at the current state.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Placement {
    /// Point the offset is measured from.
    pub anchor: Vec3,
    /// Length the offset is multiplied by.
    pub scale: f32,
    /// False when the piece does not exist at this age.
    #[cfg_attr(not(test), allow(dead_code))] // read by `position`, the shader's mirror
    pub on: bool,
    /// Where every vertex of an absent piece goes, so its triangles vanish.
    #[cfg_attr(not(test), allow(dead_code))]
    pub collapse: Vec3,
    /// Turn of the offset about the Y axis, in radians (trunk rings).
    pub twist: f32,
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// `v` turned about the Y axis by `twist` radians, the way a trunk ring's twist
/// turns its vertices (a vertex at angle `a` around the ring goes to `a + twist`).
pub(crate) fn twisted(v: Vec3, twist: f32) -> Vec3 {
    let (sin, cos) = twist.sin_cos();
    Vec3::new(v.x * cos + v.z * sin, v.y, -v.x * sin + v.z * cos)
}

/// The anchor, scale and presence of `tag`'s vertex under `state`. Not
/// meaningful for [`Class::TrunkCone`], which [`position`] handles itself.
pub(crate) fn placement(tag: &Tag, state: &GrowthState) -> Placement {
    let node = &state.nodes[tag.node as usize];
    let on_axis = node.start + node.length * tag.axis;
    let wood = |anchor: Vec3, scale: f32, flag: u32| Placement {
        anchor,
        scale,
        on: node.flags & flag != 0,
        collapse: node.start,
        twist: 0.0,
    };
    match tag.class {
        Class::Tube => wood(on_axis, lerp(node.base_radius, node.tip_radius, tag.f), WOOD),
        Class::Leaf => {
            let shown = node.leaf_scale > 0.0 && (node.leaf_count < 0.0 || (tag.ordinal as f32) < node.leaf_count);
            Placement {
                anchor: on_axis,
                scale: if shown { node.leaf_scale } else { 0.0 },
                on: true,
                collapse: on_axis,
                twist: 0.0,
            }
        }
        Class::Collar => {
            let smooth = tag.f * tag.f * (3.0 - 2.0 * tag.f);
            wood(
                node.collar_start.lerp(node.collar_end, tag.f),
                lerp(node.collar_start_radius, node.collar_end_radius, smooth),
                COLLAR,
            )
        }
        Class::Base => wood(node.start, node.base_radius, BASE),
        Class::BaseCollar => wood(node.collar_start, node.collar_start_radius, BASE_COLLAR),
        Class::RootFar => wood(node.root, node.base_radius, BASE),
        Class::RootFarCollar => wood(node.root, node.collar_start_radius, BASE_COLLAR),
        Class::Trunk | Class::TrunkTopCap | Class::TrunkCone => {
            let ring = &state.rings[tag.f as usize];
            Placement {
                anchor: ring.center,
                scale: ring.radius + tag.extra * ring.bulge,
                on: tag.class != Class::TrunkTopCap || state.trunk_flags & TOP_CAP != 0,
                collapse: ring.center,
                twist: ring.twist,
            }
        }
    }
}

/// Position of `tag`'s vertex under `state`: what the vertex shader computes.
#[cfg(test)]
pub(crate) fn position(tag: &Tag, state: &GrowthState) -> Vec3 {
    use super::state::CONE;
    let p = placement(tag, state);
    if tag.class == Class::TrunkCone {
        return if state.trunk_flags & CONE != 0 {
            p.anchor + Vec3::Y * state.cone_lift + tag.offset
        } else {
            p.anchor + p.scale * twisted(tag.axis, p.twist)
        };
    }
    if p.on { p.anchor + p.scale * twisted(tag.offset, p.twist) } else { p.collapse }
}

/// Radius of `tag`'s wood vertex under `state`, in metres: what `grown_wood_radius` in the vertex
/// shader computes (the scale of its offset; 0 for a leaf and for the apex of a mature pointed
/// trunk). The bark fades in with it.
#[cfg(test)]
pub(crate) fn wood_radius(tag: &Tag, state: &GrowthState) -> f32 {
    use super::state::CONE;
    match tag.class {
        Class::Leaf => 0.0,
        Class::TrunkCone if state.trunk_flags & CONE != 0 => 0.0,
        _ => placement(tag, state).scale,
    }
}
