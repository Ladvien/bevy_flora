//! One call from parameters + age to CPU meshes.

use crate::foliage::{FoliageMeshes, collect_leaf_points, foliage_meshes};
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
    /// A flowering tree's flowers ([`FoliageParams::blossom`](crate::FoliageParams::blossom)),
    /// one at each leaf's stem; empty for a tree without flowers. Drawn with the leaves'
    /// material; [`FloraPlugin`](crate::FloraPlugin) draws them apart from the leaves so that it
    /// can leave them out while [`FloraSeason::blossom`](crate::FloraSeason::blossom) is 0.
    pub flowers: MeshData,
    /// The aged skeleton the meshes were built from.
    pub skeleton: Skeleton,
    /// Age after which the output no longer changes.
    pub mature_at: f32,
}

/// Build the tree described by `params` as it looks at `age`
/// (`f32::INFINITY` for the mature tree).
pub fn build_tree(params: &TreeParams, age: f32) -> TreeMeshes {
    build_aged(&generate_skeleton(params), params, age)
}

/// The skeleton `full` (from [`generate_skeleton`]) as it looks at `age`: the
/// aged nodes, with pipe-model radii when `params.mesh.pipe_radius_enabled`.
pub(crate) fn aged_skeleton(full: &Skeleton, params: &TreeParams, age: f32) -> Skeleton {
    let mut skeleton = full.aged(age, &params.growth);
    if params.mesh.pipe_radius_enabled {
        apply_pipe_radius(
            &mut skeleton,
            params.mesh.pipe_radius_exponent,
            params.mesh.pipe_radius_min,
        );
    }
    skeleton
}

/// [`build_tree`] from the full-grown skeleton `full` that
/// [`generate_skeleton`] made from `params`, so a growing tree can reuse it
/// from frame to frame instead of regenerating it at every age.
pub(crate) fn build_aged(full: &Skeleton, params: &TreeParams, age: f32) -> TreeMeshes {
    let mature_at = full.mature_at(&params.growth);
    let skeleton = aged_skeleton(full, params, age);

    let trunk = skeleton_mesh(&skeleton, params);

    let crown = if params.foliage.enabled {
        let points = collect_leaf_points(
            &skeleton,
            &params.foliage,
            &params.growth,
            &params.trunk,
            params.seed,
        );
        foliage_meshes(&points, &params.foliage, params.style)
    } else {
        FoliageMeshes::default()
    };

    TreeMeshes {
        trunk,
        foliage: crown.leaves,
        flowers: crown.flowers,
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
                assert_indices_valid(&out.flowers, &what);
                assert!(out.mature_at.is_finite() && out.mature_at > 0.0);
                // Bonsai's density (0.8) yields zero primaries over its branch zone, as in the
                // original generator, so it is a bare trunk with no leaves.
                if age.is_infinite() && params.foliage.enabled && out.skeleton.nodes.len() > 1 {
                    assert!(!out.foliage.is_empty(), "{what}: mature tree has no leaves");
                }
                assert_eq!(out.flowers.is_empty(), params.foliage.blossom.is_none() || out.foliage.is_empty(), "{what}");
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
