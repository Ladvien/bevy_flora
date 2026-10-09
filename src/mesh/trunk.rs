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

/// Mesh for the (aged) trunk `node`.
///
/// Ring `k` evaluates the full-height profile at `t = k / (rings - 1) *
/// node.progress`, so radius, wobble and twist match the branch origins
/// computed on the full trunk; radii are scaled by `node.radius_progress`.
/// Returns an empty mesh when `node.progress <= 0`.
pub fn trunk_mesh(trunk: &TrunkParams, node: &Node, seed: i32) -> MeshData {
    let mut mesh = MeshData::default();
    if node.progress <= 0.0 {
        return mesh;
    }

    let segments = trunk.radial_segments.max(3);
    let rings = trunk.height_segments.max(1) + 1;
    let base_radius = trunk.radius * trunk.flare;
    let root_height = trunk.root_flare_height * trunk.height;
    let has_root_flares = trunk.root_flare_count > 0 && trunk.root_flare_spread > 0.0;
    let rp = node.radius_progress;

    // Radial bulge from root ridges at `angle`, at blend strength `root_blend`.
    let bulge = |angle: f32, root_blend: f32| -> f32 {
        if root_blend <= 0.0 {
            return 0.0;
        }
        let raw = ((angle * trunk.root_flare_count as f32).cos() * 0.5 + 0.5).powi(2);
        raw * trunk.root_flare_spread * base_radius * root_blend
    };
    let root_blend_at = |y: f32| -> f32 {
        if has_root_flares && y < root_height {
            1.0 - y / root_height
        } else {
            0.0
        }
    };
    let center_at = |t: f32| -> Vec3 {
        let (wx, wz) = trunk_wobble(trunk, seed, t);
        Vec3::new(wx, t * trunk.height, wz)
    };

    let mut prev_ring = None;
    for k in 0..rings {
        let t = (k as f32 / (rings - 1) as f32) * node.progress;
        let radius = trunk_radius_at(trunk, t);
        let root_blend = root_blend_at(t * trunk.height);
        let twist = (t * trunk.twist).to_radians();
        let ring = emit_ring(&mut mesh, center_at(t), TRUNK_BASIS, segments, twist, t, |_, angle| {
            (radius + bulge(angle - twist, root_blend)) * rp
        });
        if let Some(prev) = prev_ring {
            connect_rings(&mut mesh, prev, ring, segments);
        }
        prev_ring = Some(ring);
    }

    // Bottom cap matches ring 0 (full root bulge).
    let bottom_blend = root_blend_at(0.0);
    emit_cap(&mut mesh, Vec3::ZERO, TRUNK_BASIS, segments, 0.0, Vec3::NEG_Y, |_, angle| {
        (base_radius + bulge(angle, bottom_blend)) * rp
    });

    // Top cap: flat-capped trunks always; otherwise only while still growing
    // (a pointed tip or leader covers the top once mature).
    let top_cap = match trunk.termination {
        TrunkTermination::FlatCap => trunk.taper >= 0.1,
        TrunkTermination::PointedTip | TrunkTermination::LeaderBranch => node.progress < 1.0,
    };
    if top_cap {
        let t_top = node.progress;
        let radius = trunk_radius_at(trunk, t_top);
        let root_blend = root_blend_at(t_top * trunk.height);
        let twist = (t_top * trunk.twist).to_radians();
        emit_cap(&mut mesh, center_at(t_top), TRUNK_BASIS, segments, twist, Vec3::Y, |_, angle| {
            (radius + bulge(angle - twist, root_blend)) * rp
        });
    }

    mesh
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
}
