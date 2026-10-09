//! One call from parameters + age to CPU meshes.

use crate::foliage::{collect_leaf_points, foliage_mesh};
use crate::mesh::MeshData;
use crate::mesh::branch::{apply_pipe_radius, skeleton_mesh};
use crate::params::TreeParams;
use crate::skeleton::{Skeleton, generate_skeleton};

/// Output of [`build_tree`].
#[derive(Clone, Debug)]
pub struct TreeMeshes {
    /// Trunk, collars and branches.
    pub trunk: MeshData,
    /// Leaves; empty when foliage is disabled or nothing has leafed yet.
    pub foliage: MeshData,
    /// The aged skeleton the meshes were built from.
    pub skeleton: Skeleton,
    /// Age after which the output no longer changes.
    pub mature_at: f32,
}

/// Build the tree described by `params` as it looks at `age`
/// (`f32::INFINITY` for the mature tree).
pub fn build_tree(params: &TreeParams, age: f32) -> TreeMeshes {
    let full = generate_skeleton(params);
    let mature_at = full.mature_at(&params.growth);
    let mut skeleton = full.aged(age, &params.growth);

    if params.mesh.pipe_radius_enabled {
        apply_pipe_radius(
            &mut skeleton,
            params.mesh.pipe_radius_exponent,
            params.mesh.pipe_radius_min,
        );
    }

    let trunk = skeleton_mesh(&skeleton, params);

    let foliage = if params.foliage.enabled {
        let points = collect_leaf_points(
            &skeleton,
            &params.foliage,
            &params.growth,
            &params.trunk,
            params.seed,
        );
        if points.is_empty() {
            MeshData::default()
        } else {
            foliage_mesh(&points, params.foliage.style)
        }
    } else {
        MeshData::default()
    };

    TreeMeshes {
        trunk,
        foliage,
        skeleton,
        mature_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::rings::tests::assert_indices_valid;
    use crate::presets::TreePreset;

    #[test]
    fn build_tree_all_presets_mature() {
        for preset in TreePreset::ALL {
            let params = preset.params();
            for age in [f32::INFINITY, 3.0] {
                let out = build_tree(&params, age);
                let what = format!("{} @ {age}", preset.name());
                assert!(!out.trunk.is_empty(), "{what}: empty trunk");
                assert_indices_valid(&out.trunk, &what);
                assert_indices_valid(&out.foliage, &what);
                assert!(out.mature_at.is_finite() && out.mature_at > 0.0);
                // Bonsai's density (0.8) yields zero primaries over its branch zone, as in the
                // original generator, so it is a bare trunk with no leaves.
                if age.is_infinite() && params.foliage.enabled && out.skeleton.nodes.len() > 1 {
                    assert!(!out.foliage.is_empty(), "{what}: mature tree has no leaves");
                }
            }
        }
    }

    #[test]
    fn pipe_radius_path_builds() {
        let mut params = TreePreset::Oak.params();
        params.mesh.pipe_radius_enabled = true;
        params.mesh.smooth_iterations = 2;
        params.mesh.adaptive_resolution = true;
        let out = build_tree(&params, f32::INFINITY);
        assert_indices_valid(&out.trunk, "pipe+smooth+adaptive");
    }
}
