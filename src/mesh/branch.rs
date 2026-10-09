//! Branch tube mesher, trunk collars, the pipe-radius model, and the whole-skeleton wood mesh.

use bevy::math::Vec3;

use super::MeshData;
use super::rings::{RingBasis, connect_rings, emit_cap, emit_ring};
use super::smoothing::{laplacian_smooth, recalculate_normals};
use super::trunk::trunk_mesh;
use crate::params::TreeParams;
use crate::skeleton::{BENT_AXIS_POINTS, Node, NodeKind, Skeleton};

/// Wood at or below this height is the trunk's base on the ground.
const GROUND_EPSILON: f32 = 1.0e-4;

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Short tube of `radius` from `root` (a point inside the trunk) to `base`,
/// with both rings perpendicular to `axis` and untwisted, so its `base` ring
/// coincides with the base ring of a tube or collar starting at `base` along
/// `axis`. Extends that wood back into the trunk; empty when `root` is not
/// behind `base` along `axis`.
pub fn root_segment(root: Vec3, base: Vec3, axis: Vec3, segments: u32, radius: f32) -> MeshData {
    let mut mesh = MeshData::default();
    let basis = RingBasis::from_axis(axis);
    if (base - root).dot(basis.axis()) > 1e-4 {
        let segments = segments.max(3);
        let a = emit_ring(&mut mesh, root, basis, segments, 0.0, 0.0, |_, _| radius);
        let b = emit_ring(&mut mesh, base, basis, segments, 0.0, 0.0, |_, _| radius);
        connect_rings(&mut mesh, a, b, segments);
    }
    mesh
}

/// Tapered tube along `node`'s (possibly bent) axis with optional twist,
/// capped at the tip.
pub fn tube_mesh(node: &Node, radial_segments: u32, twist_deg: f32) -> MeshData {
    let mut mesh = MeshData::default();
    let segments = radial_segments.max(3);
    let rings = if twist_deg.abs() > 0.1 || node.bend > 0.0 { BENT_AXIS_POINTS } else { 2 };
    let axis = node.axis(rings);
    let (centers, dirs): (Vec<Vec3>, Vec<Vec3>) = axis.into_iter().unzip();

    let mut prev_ring = None;
    for k in 0..rings {
        let t = k as f32 / (rings - 1) as f32;
        let radius = lerp(node.base_radius, node.tip_radius, t);
        let twist = (t * twist_deg).to_radians();
        let basis = RingBasis::from_axis(dirs[k as usize]);
        let ring = emit_ring(&mut mesh, centers[k as usize], basis, segments, twist, t, |_, _| radius);
        if let Some(prev) = prev_ring {
            connect_rings(&mut mesh, prev, ring, segments);
        }
        prev_ring = Some(ring);
    }

    let end_dir = dirs[rings as usize - 1];
    let end_pos = centers[rings as usize - 1];
    emit_cap(
        &mut mesh,
        end_pos,
        RingBasis::from_axis(end_dir),
        segments,
        twist_deg.to_radians(),
        end_dir,
        |_, _| node.tip_radius,
    );

    mesh
}

/// Start of a primary's collar (its wide end) for `collar_mesh` arguments.
fn collar_start(start: Vec3, direction: Vec3, collar_length: f32) -> Vec3 {
    start - direction * collar_length * 0.1
}

/// Width of a collar's wide end relative to the branch radius.
const COLLAR_FLARE: f32 = 1.3;

/// Flared cone blending a primary branch into the trunk surface.
pub fn collar_mesh(
    start: Vec3,
    direction: Vec3,
    branch_radius: f32,
    collar_length: f32,
    radial_segments: u32,
) -> MeshData {
    let mut mesh = MeshData::default();
    let segments = radial_segments.max(3);
    let rings = 3u32;
    let basis = RingBasis::from_axis(direction);

    let collar_start = collar_start(start, direction, collar_length);
    let collar_end = start + direction * collar_length * 0.5;
    let start_radius = branch_radius * COLLAR_FLARE;
    let end_radius = branch_radius;

    let mut prev_ring = None;
    for k in 0..rings {
        let t = k as f32 / (rings - 1) as f32;
        let center = collar_start.lerp(collar_end, t);
        let smooth_t = t * t * (3.0 - 2.0 * t);
        let radius = lerp(start_radius, end_radius, smooth_t);
        let ring = emit_ring(&mut mesh, center, basis, segments, 0.0, t, |_, _| radius);
        if let Some(prev) = prev_ring {
            connect_rings(&mut mesh, prev, ring, segments);
        }
        prev_ring = Some(ring);
    }

    mesh
}

/// Parent radius from child radii under the pipe model
/// (`parent^exponent = Σ child^exponent`), at least `min_radius`.
pub fn calculate_pipe_radius(child_radii: &[f32], exponent: f32, min_radius: f32) -> f32 {
    if child_radii.is_empty() {
        return min_radius;
    }
    let accumulated: f32 = child_radii.iter().map(|r| r.powf(exponent)).sum();
    accumulated.powf(1.0 / exponent).max(min_radius)
}

/// Recompute every non-trunk node's radii bottom-up from its children.
pub fn apply_pipe_radius(skeleton: &mut Skeleton, exponent: f32, min_radius: f32) {
    let n = skeleton.nodes.len();
    if n < 2 {
        return;
    }
    // Σ child_base^exponent per node; children have larger indices, so a
    // reverse sweep sees every child before its parent.
    let mut accumulated = vec![0.0f32; n];
    let mut has_child = vec![false; n];
    for i in (1..n).rev() {
        if has_child[i] {
            let tip = accumulated[i].powf(1.0 / exponent).max(min_radius);
            let node = &mut skeleton.nodes[i];
            node.tip_radius = tip;
            if node.base_radius < tip {
                node.base_radius = tip * 1.2;
            }
        }
        let node = &skeleton.nodes[i];
        if let Some(p) = node.parent {
            accumulated[p as usize] += node.base_radius.powf(exponent);
            has_child[p as usize] = true;
        }
    }
    for node in skeleton.nodes.iter_mut().skip(1) {
        if node.tip_radius > node.base_radius {
            node.tip_radius = node.base_radius * 0.8;
        }
        node.base_radius = node.base_radius.max(min_radius);
        node.tip_radius = node.tip_radius.max(min_radius * 0.5);
    }
}

/// Fewest sides of a non-adaptive branch tube; 4 read as flat square planks.
const MIN_BRANCH_SEGMENTS: u32 = 6;

/// Radial segment count for a branch node.
fn radial_segments_for(node: &Node, params: &TreeParams) -> u32 {
    let m = &params.mesh;
    if m.adaptive_resolution {
        let segments = (node.base_radius * m.resolution_scale).round() as u32;
        segments.clamp(m.min_radial_segments, m.max_radial_segments)
    } else if node.depth > 0 {
        MIN_BRANCH_SEGMENTS
    } else {
        (params.trunk.radial_segments / 2).max(MIN_BRANCH_SEGMENTS)
    }
}

/// Where a primary's wood begins inside the trunk, so tube and collar have no
/// open, flat-cut end at the trunk surface (`start`). `anchor` is the trunk
/// axis point at the same height.
///
/// A branch leaving the trunk at 30° or more is extended backward along its
/// own axis to the trunk axis (at most twice the trunk radius), so it emerges
/// from the trunk. A steeper branch would need a long extension that shows as
/// a ridge down the trunk; its base is closed toward `anchor` instead.
fn primary_root(node: &Node) -> Vec3 {
    let radial = (node.start - node.anchor).with_y(0.0);
    let outward = node.direction.dot(radial.normalize_or_zero());
    if outward < 0.5 {
        return node.anchor;
    }
    let back = (radial.length() / outward).min(0.9 * node.start.y / node.direction.y.max(1e-3));
    node.start - node.direction * back
}

/// Wood mesh for the whole (aged) skeleton: trunk, collars and branch tubes,
/// optionally smoothed.
pub fn skeleton_mesh(skeleton: &Skeleton, params: &TreeParams) -> MeshData {
    let b = &params.branches;
    let mut mesh = trunk_mesh(&params.trunk, &skeleton.nodes[0], params.seed);
    // Primary bases extended into the trunk. Added after smoothing: the bases
    // are open boundaries then, so smoothing keeps them in place and does not
    // pull the junction toward the trunk axis.
    let mut roots = MeshData::default();

    for node in skeleton.nodes.iter().skip(1) {
        if node.length <= 1e-4 || node.base_radius <= 0.0 {
            continue;
        }
        let segments = radial_segments_for(node, params);
        if node.kind == NodeKind::Primary {
            let root = primary_root(node);
            roots.extend(&root_segment(root, node.start, node.direction, segments, node.base_radius));
            if params.mesh.collar_enabled {
                let collar_length = node.base_radius * params.mesh.collar_length;
                // Same sides as the tube it wraps.
                let collar_segments = segments;
                mesh.extend(&collar_mesh(
                    node.start,
                    node.direction,
                    node.base_radius,
                    collar_length,
                    collar_segments,
                ));
                roots.extend(&root_segment(
                    root,
                    collar_start(node.start, node.direction, collar_length),
                    node.direction,
                    collar_segments,
                    node.base_radius * COLLAR_FLARE,
                ));
            }
        }
        mesh.extend(&tube_mesh(node, segments, b.twist));
    }

    if params.mesh.smooth_iterations > 0 {
        // The trunk's bottom cap sits at y = 0; keep it on the ground.
        laplacian_smooth(
            &mut mesh.positions,
            &mesh.indices,
            params.mesh.smooth_iterations,
            params.mesh.smooth_factor,
            |p| p.y <= GROUND_EPSILON,
        );
        recalculate_normals(&mesh.positions, &mesh.indices, &mut mesh.normals);
    }
    mesh.extend(&roots);

    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::rings::tests::{assert_faces_outward, assert_indices_valid};
    use crate::presets::TreePreset;
    use crate::skeleton::generate_skeleton;

    #[test]
    fn wood_mesh_faces_outward_for_all_presets() {
        for preset in TreePreset::ALL {
            let mut params = preset.params();
            params.mesh.smooth_iterations = 0;
            let skeleton = generate_skeleton(&params);
            let mesh = skeleton_mesh(&skeleton, &params);
            assert_indices_valid(&mesh, preset.name());
            assert_faces_outward(&mesh, preset.name());
        }
    }

    #[test]
    fn collar_and_bent_tube_face_outward() {
        let params = TreePreset::Willow.params(); // gravity + twist
        let skeleton = generate_skeleton(&params);
        let node = skeleton
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Primary)
            .unwrap();
        assert!(node.bend > 0.0);
        let tube = tube_mesh(node, 6, 45.0);
        assert_indices_valid(&tube, "tube");
        assert_faces_outward(&tube, "tube");
        let collar = collar_mesh(node.start, node.direction, node.base_radius, 0.3, 5);
        assert_indices_valid(&collar, "collar");
        assert_faces_outward(&collar, "collar");
        let root = root_segment(primary_root(node), node.start, node.direction, 6, node.base_radius);
        assert_indices_valid(&root, "root");
        assert_faces_outward(&root, "root");
    }

    /// Vertices on edges used by exactly one triangle (open boundaries), with
    /// coincident vertices (seams, cap rings) treated as one.
    fn open_boundary_vertices(mesh: &MeshData) -> Vec<Vec3> {
        let key = |i: u32| (mesh.positions[i as usize] * 1.0e4).round().as_ivec3().to_array();
        let mut uses: std::collections::HashMap<([i32; 3], [i32; 3]), u32> = Default::default();
        for &[a, b, c] in mesh.indices.as_chunks::<3>().0 {
            for (u, v) in [(a, b), (b, c), (c, a)] {
                let (ku, kv) = (key(u), key(v));
                if ku != kv {
                    *uses.entry((ku.min(kv), ku.max(kv))).or_insert(0) += 1;
                }
            }
        }
        uses.into_iter()
            .filter(|&(_, n)| n == 1)
            .flat_map(|((u, v), _)| [u, v])
            .map(|k| Vec3::new(k[0] as f32, k[1] as f32, k[2] as f32) / 1.0e4)
            .collect()
    }

    #[test]
    fn primary_bases_are_buried_in_the_trunk() {
        // Tube plus root segment is open only at the root; that opening must
        // be inside the trunk, or a shallow junction shows a flat, open end.
        for preset in TreePreset::ALL {
            let params = preset.params();
            let skeleton = generate_skeleton(&params);
            for node in skeleton.nodes.iter().filter(|n| n.kind == NodeKind::Primary) {
                let mut wood = tube_mesh(node, 4, params.branches.twist);
                wood.extend(&root_segment(primary_root(node), node.start, node.direction, 4, node.base_radius));
                assert_faces_outward(&wood, preset.name());
                for p in open_boundary_vertices(&wood) {
                    let depth = crate::skeleton::trunk_depth(&params.trunk, params.seed, p);
                    assert!(depth > 0.0, "{}: primary {} has an open end {depth} m outside the trunk", preset.name(), node.id);
                }
            }
        }
    }

    #[test]
    fn smoothing_keeps_primary_junctions_closed() {
        // Each primary's root segment must meet its (smoothed) tube and collar:
        // every vertex of the root segment's outer ring appears again in the
        // wood mesh, on the tube's or collar's base ring.
        let mut params = TreePreset::Oak.params();
        params.mesh.smooth_iterations = 2;
        let skeleton = generate_skeleton(&params);
        let mesh = skeleton_mesh(&skeleton, &params);
        let copies = |p: Vec3| mesh.positions.iter().filter(|q| q.distance(p) < 1e-5).count();
        for node in skeleton.nodes.iter().filter(|n| n.kind == NodeKind::Primary && n.length > 1e-4) {
            let segments = radial_segments_for(node, &params);
            let root = root_segment(primary_root(node), node.start, node.direction, segments, node.base_radius);
            let outer_ring = &root.positions[root.positions.len() / 2..];
            for &p in outer_ring {
                assert!(copies(p) >= 2, "primary {}: junction vertex {p:?} has no matching tube vertex", node.id);
            }
        }
    }

    #[test]
    fn gravity_bent_wood_ends_where_the_skeleton_says() {
        let mut params = TreePreset::Oak.params();
        params.branches.gravity_strength = 0.8;
        params.branches.stiffness = 0.1;
        // Mid-growth (parents still lengthening) and mature.
        for age in [6.0, f32::INFINITY] {
            let out = crate::pipeline::build_tree(&params, age);
            let skeleton = &out.skeleton;

            let mut visibly_bent = 0;
            for (i, node) in skeleton.nodes.iter().enumerate().skip(1) {
                if node.length <= 1e-4 || node.base_radius <= 0.0 {
                    continue; // no wood emitted
                }
                let end = skeleton.end(i);
                if end.distance(node.start + node.direction * node.length) > node.base_radius {
                    visibly_bent += 1;
                }
                // Child branches and leaves sit on `end`; the bent wood must reach it.
                let nearest = out
                    .trunk
                    .positions
                    .iter()
                    .map(|p| p.distance(end))
                    .fold(f32::MAX, f32::min);
                assert!(
                    nearest <= node.tip_radius + 1e-3,
                    "age {age}, node {i}: wood tip is {nearest} from where its leaves and children sit"
                );
                // Every child starts on this node's wood axis.
                for c in skeleton.children(i) {
                    let axis = node.axis(crate::skeleton::BENT_AXIS_POINTS);
                    let on_axis = axis
                        .windows(2)
                        .map(|w| segment_distance(skeleton.nodes[c].start, w[0].0, w[1].0))
                        .fold(f32::MAX, f32::min);
                    assert!(on_axis < 1e-4, "age {age}: child {c} of {i} is {on_axis} off the bent axis");
                }
            }
            assert!(visibly_bent > 0, "age {age}: the gravity toggle should bend some branch");
        }
    }

    fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.length_squared().max(f32::MIN_POSITIVE)).clamp(0.0, 1.0);
        p.distance(a + ab * t)
    }

    #[test]
    fn smoothed_wood_stays_attached_and_seamless() {
        let mut params = TreePreset::Oak.params();
        params.mesh.smooth_iterations = 0;
        let skeleton = generate_skeleton(&params);
        let raw = skeleton_mesh(&skeleton, &params);
        params.mesh.smooth_iterations = 2;
        let smooth = skeleton_mesh(&skeleton, &params);
        assert_eq!(raw.positions.len(), smooth.positions.len());

        // Vertices that coincide (UV seams, cap rings) must still coincide.
        let cell = |p: Vec3| (p * 1.0e5).round().as_ivec3();
        let mut groups: std::collections::HashMap<_, Vec<usize>> = Default::default();
        for (i, p) in raw.positions.iter().enumerate() {
            groups.entry(cell(*p)).or_default().push(i);
        }
        for group in groups.values().filter(|g| g.len() > 1) {
            let first = smooth.positions[group[0]];
            for &i in group {
                let gap = (smooth.positions[i] - first).length();
                assert!(gap < 1e-3, "vertices {} and {i} torn apart by {gap}", group[0]);
            }
        }

        // The trunk base stays on the ground instead of being pulled up.
        for (a, b) in raw.positions.iter().zip(&smooth.positions) {
            if a.y <= 1e-4 {
                assert!(b.y <= 1e-3, "ground vertex {a:?} lifted to {b:?}");
            }
        }
        assert_indices_valid(&smooth, "smoothed oak");
    }

    #[test]
    fn pipe_radius_combines_children() {
        assert_eq!(calculate_pipe_radius(&[], 2.0, 0.1), 0.1);
        let r = calculate_pipe_radius(&[0.3, 0.4], 2.0, 0.01);
        assert!((r - 0.5).abs() < 1e-6);
    }

    #[test]
    fn apply_pipe_radius_keeps_tip_below_base_and_above_min() {
        let mut params = TreePreset::Oak.params();
        params.mesh.pipe_radius_enabled = true;
        let mut skeleton = generate_skeleton(&params);
        let trunk_before = skeleton.nodes[0].clone();
        apply_pipe_radius(&mut skeleton, 2.0, 0.02);
        assert_eq!(skeleton.nodes[0], trunk_before);
        for (i, n) in skeleton.nodes.iter().enumerate().skip(1) {
            assert!(n.tip_radius <= n.base_radius, "node {i}");
            assert!(n.base_radius >= 0.02, "node {i}");
            assert!(n.tip_radius >= 0.01, "node {i}");
        }
    }
}
