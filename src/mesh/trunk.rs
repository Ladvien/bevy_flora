//! Trunk mesher: tapered, wobbling, twisted cylinder with root flares and caps.

use bevy::math::Vec3;

use super::MeshData;
use super::rings::{RingBasis, connect_rings, emit_cap, emit_ring};
use crate::params::{TrunkParams, TrunkTermination};
use crate::skeleton::{Node, trunk_radius_at, trunk_wobble};

/// Basis for horizontal rings around the Y axis: `(X, -Z, Y)` is right-handed.
const TRUNK_BASIS: RingBasis = RingBasis {
    right: Vec3::X,
    forward: Vec3::NEG_Z,
};

/// Height of a pointed trunk's tip cone relative to the trunk's top radius.
const POINTED_TIP_ASPECT: f32 = 4.0;

/// The optional pieces of a trunk mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TrunkPieces {
    /// A mature pointed trunk's tip cone above its top ring.
    pub cone: bool,
    /// A cap closing the top ring: flat-capped trunks always, the others
    /// only while still growing (once mature, the pointed tip or the leader
    /// branch closes the top).
    pub top_cap: bool,
}

/// The pieces the (aged) trunk `node` has.
pub(crate) fn trunk_pieces(trunk: &TrunkParams, node: &Node) -> TrunkPieces {
    TrunkPieces {
        cone: trunk.termination == TrunkTermination::PointedTip && node.progress >= 1.0,
        top_cap: match trunk.termination {
            TrunkTermination::FlatCap => true,
            TrunkTermination::PointedTip | TrunkTermination::LeaderBranch => node.progress < 1.0,
        },
    }
}

/// Mesh for the (aged) trunk `node`.
///
/// Ring `k` evaluates the full-height profile at `t = k / (rings - 1) *
/// node.progress`, so radius, wobble and twist match the branch origins
/// computed on the full trunk; radii are scaled by `node.radius_progress`.
/// Returns an empty mesh when `node.progress <= 0`.
pub fn trunk_mesh(trunk: &TrunkParams, node: &Node, seed: i32) -> MeshData {
    trunk_mesh_with(trunk, node, seed, trunk_pieces(trunk, node))
}

/// [`trunk_mesh`] with the optional `pieces` given. Vertex order: the rings
/// (`segments + 1` vertices each, bottom to top), the tip cone's apex ring if
/// any, the bottom cap (centre, then `segments + 1` rim vertices), the top cap
/// likewise if any.
pub(crate) fn trunk_mesh_with(trunk: &TrunkParams, node: &Node, seed: i32, pieces: TrunkPieces) -> MeshData {
    let mut mesh = MeshData::default();
    if node.progress <= 0.0 {
        return mesh;
    }

    let segments = trunk.radial_segments.max(3);
    let rings = trunk_ring_count(trunk);
    let profile = Profile::new(trunk, seed);
    let rp = node.radius_progress;

    let mut prev_ring = None;
    for k in 0..rings {
        let t = (k as f32 / (rings - 1) as f32) * node.progress;
        let radius = trunk_radius_at(trunk, t);
        let root_blend = profile.root_blend_at(t * trunk.height);
        let twist = (t * trunk.twist).to_radians();
        let ring = emit_ring(&mut mesh, profile.center_at(t), TRUNK_BASIS, segments, twist, t, |_, angle| {
            (radius + profile.bulge(angle - twist, root_blend)) * rp
        });
        if let Some(prev) = prev_ring {
            connect_rings(&mut mesh, prev, ring, segments);
        }
        prev_ring = Some(ring);
    }

    // A mature pointed trunk closes in a cone above its top ring.
    if pieces.cone
        && let Some(top_ring) = prev_ring
    {
        let apex = profile.center_at(1.0) + Vec3::Y * trunk_cone_lift(trunk, node);
        let twist = trunk.twist.to_radians();
        let tip = emit_ring(&mut mesh, apex, TRUNK_BASIS, segments, twist, 1.0, |_, _| 0.0);
        connect_rings(&mut mesh, top_ring, tip, segments);
    }

    // Bottom cap matches ring 0 (full root bulge).
    let bottom_blend = profile.root_blend_at(0.0);
    emit_cap(&mut mesh, Vec3::ZERO, TRUNK_BASIS, segments, 0.0, Vec3::NEG_Y, |_, angle| {
        (profile.base_radius + profile.bulge(angle, bottom_blend)) * rp
    });

    // Top cap: flat-capped trunks always; the others only while still growing
    // (once mature, the pointed tip or the leader branch closes the top).
    if pieces.top_cap {
        let t_top = node.progress;
        let radius = trunk_radius_at(trunk, t_top);
        let root_blend = profile.root_blend_at(t_top * trunk.height);
        let twist = (t_top * trunk.twist).to_radians();
        emit_cap(&mut mesh, profile.center_at(t_top), TRUNK_BASIS, segments, twist, Vec3::Y, |_, angle| {
            (radius + profile.bulge(angle - twist, root_blend)) * rp
        });
    }

    mesh
}

/// Rings in the trunk mesh, bottom to top.
pub(crate) fn trunk_ring_count(trunk: &TrunkParams) -> u32 {
    trunk.height_segments.max(1) + 1
}

/// Height of a mature pointed trunk's tip cone above its top ring.
pub(crate) fn trunk_cone_lift(trunk: &TrunkParams, node: &Node) -> f32 {
    trunk_radius_at(trunk, 1.0) * node.radius_progress * POINTED_TIP_ASPECT
}

/// The root-ridge bulge of the trunk around the ring at `angle`, 0 to 1,
/// before it is scaled by the ring's blend, the spread and the radius.
pub(crate) fn root_bulge_unit(trunk: &TrunkParams, angle: f32) -> f32 {
    ((angle * trunk.root_flare_count as f32).cos() * 0.5 + 0.5).powi(2)
}

/// One ring of the (aged) trunk, as the shader needs it: the vertex at base
/// angle `a` sits at `center + radial * (radius + root_bulge_unit(a) * bulge)`,
/// where `radial` is the direction at angle `a + twist` around the trunk axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TrunkRing {
    /// Centre of the ring.
    pub center: Vec3,
    /// Taper radius, scaled by the trunk's `radius_progress`.
    pub radius: f32,
    /// Peak root-ridge bulge, scaled by the trunk's `radius_progress`.
    pub bulge: f32,
    /// Rotation of the ring's vertices about the trunk axis, in radians:
    /// `trunk.twist * t` for the ring at height fraction `t`, so it changes
    /// with the trunk's growth.
    pub twist: f32,
}

/// Fill `out` with the rings of the (aged) trunk `node`, bottom to top: the
/// same centres and radii [`trunk_mesh`] puts its rings at.
pub(crate) fn trunk_rings(trunk: &TrunkParams, node: &Node, seed: i32, out: &mut Vec<TrunkRing>) {
    out.clear();
    let rings = trunk_ring_count(trunk);
    let profile = Profile::new(trunk, seed);
    let rp = node.radius_progress;
    for k in 0..rings {
        let t = (k as f32 / (rings - 1) as f32) * node.progress;
        let root_blend = profile.root_blend_at(t * trunk.height);
        out.push(TrunkRing {
            center: profile.center_at(t),
            radius: trunk_radius_at(trunk, t) * rp,
            bulge: profile.bulge_peak(root_blend) * rp,
            twist: (t * trunk.twist).to_radians(),
        });
    }
}

/// The closed-form pieces of the trunk's profile.
struct Profile<'a> {
    trunk: &'a TrunkParams,
    seed: i32,
    base_radius: f32,
    root_height: f32,
    has_root_flares: bool,
}

impl<'a> Profile<'a> {
    fn new(trunk: &'a TrunkParams, seed: i32) -> Self {
        Self {
            trunk,
            seed,
            base_radius: trunk.radius * trunk.flare,
            root_height: trunk.root_flare_height * trunk.height,
            has_root_flares: trunk.root_flare_count > 0 && trunk.root_flare_spread > 0.0,
        }
    }

    /// Peak bulge at blend strength `root_blend`.
    fn bulge_peak(&self, root_blend: f32) -> f32 {
        if root_blend <= 0.0 {
            return 0.0;
        }
        self.trunk.root_flare_spread * self.base_radius * root_blend
    }

    /// Radial bulge from root ridges at `angle`, at blend strength `root_blend`.
    fn bulge(&self, angle: f32, root_blend: f32) -> f32 {
        if root_blend <= 0.0 {
            return 0.0;
        }
        root_bulge_unit(self.trunk, angle) * self.trunk.root_flare_spread * self.base_radius * root_blend
    }

    fn root_blend_at(&self, y: f32) -> f32 {
        if self.has_root_flares && y < self.root_height {
            1.0 - y / self.root_height
        } else {
            0.0
        }
    }

    fn center_at(&self, t: f32) -> Vec3 {
        let (wx, wz) = trunk_wobble(self.trunk, self.seed, t);
        Vec3::new(wx, t * self.trunk.height, wz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::rings::tests::{assert_faces_outward, assert_indices_valid};
    use crate::presets::TreePreset;
    use crate::skeleton::generate_skeleton;

    #[test]
    fn all_triangles_face_outward() {
        for preset in TreePreset::ALL {
            let params = preset.params();
            let skeleton = generate_skeleton(&params);
            let mesh = trunk_mesh(&params.trunk, &skeleton.nodes[0], params.seed);
            assert!(!mesh.is_empty());
            assert_indices_valid(&mesh, preset.name());
            assert_faces_outward(&mesh, preset.name());
        }
    }

    #[test]
    fn growing_trunk_is_capped_and_outward() {
        let params = TreePreset::Pine.params(); // PointedTip
        let skeleton = generate_skeleton(&params);
        let aged = skeleton.aged(1.0, &params.growth);
        let node = &aged.nodes[0];
        assert!(node.progress > 0.0 && node.progress < 1.0);
        let mesh = trunk_mesh(&params.trunk, node, params.seed);
        assert_indices_valid(&mesh, "growing pine");
        assert_faces_outward(&mesh, "growing pine");
        let top = mesh.positions.iter().map(|p| p.y).fold(0.0, f32::max);
        assert!((top - params.trunk.height * node.progress).abs() < 1e-4);
    }

    #[test]
    fn trunk_progress_zero_is_empty() {
        let params = TreePreset::Oak.params();
        let skeleton = generate_skeleton(&params);
        let aged = skeleton.aged(0.0, &params.growth);
        let mesh = trunk_mesh(&params.trunk, &aged.nodes[0], params.seed);
        assert!(mesh.is_empty());
        assert!(mesh.positions.is_empty());
    }

    #[test]
    fn indices_in_range() {
        let params = TreePreset::Baobab.params(); // root flares
        let skeleton = generate_skeleton(&params);
        let mesh = trunk_mesh(&params.trunk, &skeleton.nodes[0], params.seed);
        assert_indices_valid(&mesh, "baobab");
        let max = *mesh.indices.iter().max().unwrap() as usize;
        assert!(max < mesh.positions.len());
    }

    #[test]
    fn flat_and_pointed_trunks_are_closed() {
        // Without a leader on top, nothing else covers the trunk: its mesh
        // must have no open edge (seen from above it would be a hole).
        for termination in [TrunkTermination::FlatCap, TrunkTermination::PointedTip] {
            for taper in [0.3, 0.05] {
                let mut params = TreePreset::Oak.params();
                params.trunk.termination = termination;
                params.trunk.taper = taper;
                let skeleton = generate_skeleton(&params);
                let mesh = trunk_mesh(&params.trunk, &skeleton.nodes[0], params.seed);
                let key = |i: u32| (mesh.positions[i as usize] * 1.0e4).round().as_ivec3().to_array();
                let mut uses = std::collections::HashMap::new();
                for &[a, b, c] in mesh.indices.as_chunks::<3>().0 {
                    for (u, v) in [(a, b), (b, c), (c, a)] {
                        let (ku, kv) = (key(u), key(v));
                        if ku != kv {
                            *uses.entry((ku.min(kv), ku.max(kv))).or_insert(0) += 1;
                        }
                    }
                }
                let open = uses.values().filter(|&&n| n == 1).count();
                assert_eq!(open, 0, "{termination:?} taper {taper}: {open} open edges");
                assert_faces_outward(&mesh, "closed trunk");
            }
        }
    }
}
