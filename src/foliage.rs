//! Leaf placement on the (aged) skeleton and leaf geometry.

use bevy::math::{Vec2, Vec3, Vec4};
use std::f32::consts::{PI, TAU};

use crate::growth::GrowthParams;
use crate::mesh::MeshData;
use crate::params::{FoliageParams, FoliagePlacement, LeafOrientation, LeafStyle, TrunkParams};
use crate::rng::SeededRng;
use crate::skeleton::{Node, NodeKind, Skeleton, trunk_depth};

/// Separate random stream for leaf tints, so adding them does not change any
/// existing draw (leaf positions, sizes and rotations stay as in 0.1).
const LEAF_TINT_SALT: i32 = 0x7135_1EAF;

/// One leaf: where it sits and how it is oriented.
#[derive(Clone, Debug, PartialEq)]
pub struct LeafPoint {
    /// World-space (tree-local) position of the leaf, in metres.
    pub position: Vec3,
    /// Primary orientation.
    pub direction: Vec3,
    /// Leaf size in metres (base-to-tip length for leaf-shaped cards).
    pub size: f32,
    /// Rotation around `direction`, radians.
    pub rotation: f32,
    /// Per-leaf colour draw in -1..1, scaled by [`FoliageParams::color_variation`].
    pub tint: f32,
}

/// Leaf points for every node of `skeleton`.
///
/// Each node draws from its own RNG stream (keyed by the node's id in the
/// full skeleton), so aging the tree never re-rolls other nodes' leaves.
pub fn collect_leaf_points(
    skeleton: &Skeleton,
    foliage: &FoliageParams,
    growth: &GrowthParams,
    trunk: &TrunkParams,
    seed: i32,
) -> Vec<LeafPoint> {
    let mut points = Vec::new();
    if !foliage.enabled {
        return points;
    }
    for i in 0..skeleton.nodes.len() {
        leaf_points_for_node(skeleton, i, foliage, growth, trunk, seed, &mut points);
    }
    points
}

/// Append the leaf points of node `i` to `points`.
#[allow(clippy::too_many_arguments)]
pub fn leaf_points_for_node(
    skeleton: &Skeleton,
    i: usize,
    foliage: &FoliageParams,
    growth: &GrowthParams,
    trunk: &TrunkParams,
    seed: i32,
    points: &mut Vec<LeafPoint>,
) {
    let node = &skeleton.nodes[i];
    if node.kind == NodeKind::Trunk {
        return;
    }

    let leaf_scale = if growth.leaf_threshold >= 1.0 {
        if node.progress >= 1.0 { 1.0 } else { 0.0 }
    } else {
        ((node.progress - growth.leaf_threshold) / (1.0 - growth.leaf_threshold)).clamp(0.0, 1.0)
    };
    if leaf_scale <= 0.0 {
        return;
    }

    // Threshold on the mature radius so the decision does not change with age.
    let full_tip_radius = if node.radius_progress > 0.0 {
        node.tip_radius / node.radius_progress
    } else {
        node.tip_radius
    };
    if trunk.radius > 0.0
        && full_tip_radius / trunk.radius > foliage.radius_threshold
        && foliage.placement != FoliagePlacement::AllBranches
    {
        return;
    }

    let mut rng = SeededRng::from_parts(seed, node.id);
    let first = points.len();
    let ctx = NodeCtx {
        wood: node,
        end: skeleton.end(i),
        direction: node.direction,
        length: node.length,
        height_ratio: node.height_ratio,
        trunk,
        seed,
        leaf_scale,
    };

    match foliage.placement {
        FoliagePlacement::TerminalBranches => {
            if skeleton.is_terminal(i) {
                add_terminal_leaves(points, &ctx, foliage, &mut rng);
            }
        }
        FoliagePlacement::AllBranches => add_distributed_leaves(points, &ctx, foliage, &mut rng),
        FoliagePlacement::TipClusters => {
            if skeleton.is_terminal(i) {
                add_cluster_leaves(points, &ctx, foliage, &mut rng);
            }
        }
    }

    // Tints come from their own stream, drawn in leaf order, so a node's
    // leaves keep their tints as the tree ages.
    let mut tint_rng = SeededRng::from_parts(seed ^ LEAF_TINT_SALT, node.id);
    for p in &mut points[first..] {
        p.tint = tint_rng.range(-1.0, 1.0);
    }

    if leaf_scale < 1.0 {
        for p in &mut points[first..] {
            p.size *= leaf_scale;
        }
    }
}

struct NodeCtx<'a> {
    /// The node itself; leaves along it follow its (bent) axis.
    wood: &'a Node,
    /// Leaves around a tip must not end up inside the trunk.
    trunk: &'a TrunkParams,
    seed: i32,
    end: Vec3,
    direction: Vec3,
    length: f32,
    height_ratio: f32,
    /// Growth scale of this node's leaves (0..1); tip clusters shrink with it.
    leaf_scale: f32,
}

fn add_terminal_leaves(
    points: &mut Vec<LeafPoint>,
    node: &NodeCtx,
    foliage: &FoliageParams,
    rng: &mut SeededRng,
) {
    let height_mod = height_modifier(node.height_ratio, foliage);
    let count = ((foliage.density * height_mod) as i32).max(1);
    for _ in 0..count {
        let size = foliage.leaf_size
            * (1.0 + rng.range(-foliage.leaf_size_variation, foliage.leaf_size_variation));
        let direction = leaf_direction(node, foliage, rng);
        let offset = rng.range(0.0, node.length * 0.2);
        let along = if node.length > 0.0 { 1.0 - offset / node.length } else { 1.0 };
        points.push(LeafPoint {
            position: node.wood.point_at(along),
            direction,
            size,
            rotation: rng.range(0.0, TAU) * foliage.rotation_variation,
            tint: 0.0,
        });
    }
}

fn add_distributed_leaves(
    points: &mut Vec<LeafPoint>,
    node: &NodeCtx,
    foliage: &FoliageParams,
    rng: &mut SeededRng,
) {
    let height_mod = height_modifier(node.height_ratio, foliage);
    let count = ((foliage.density * node.length * height_mod) as i32).max(1);
    for i in 0..count {
        let t = (i as f32 + rng.range(0.0, 1.0)) / count as f32;
        let biased_t = t * t;
        let position = node.wood.point_at(0.3 + biased_t * 0.7);
        let size = foliage.leaf_size
            * (1.0 + rng.range(-foliage.leaf_size_variation, foliage.leaf_size_variation));
        let direction = leaf_direction(node, foliage, rng);
        points.push(LeafPoint {
            position,
            direction,
            size,
            rotation: rng.range(0.0, TAU) * foliage.rotation_variation,
            tint: 0.0,
        });
    }
}

fn add_cluster_leaves(
    points: &mut Vec<LeafPoint>,
    node: &NodeCtx,
    foliage: &FoliageParams,
    rng: &mut SeededRng,
) {
    let height_mod = height_modifier(node.height_ratio, foliage);
    let count = ((foliage.cluster_size as f32 * height_mod) as i32).max(1);
    // Leaves fill a ball within 3/4 of a leaf of the tip, so each one
    // overlaps the twig end instead of floating on a shell around it. The
    // ball shrinks with the leaves while the node is young.
    let cluster_radius = foliage.leaf_size * 0.75 * node.leaf_scale;
    for _ in 0..count {
        let theta = rng.range(0.0, TAU);
        let phi = rng.range(0.0, PI);
        let r = cluster_radius * rng.range(0.0, 1.0);
        let mut offset = Vec3::new(
            r * phi.sin() * theta.cos(),
            r * phi.sin() * theta.sin(),
            r * phi.cos(),
        );
        // Twigs near the trunk would put part of the sphere inside it; the
        // mirrored point is outside (the tip is, and the trunk is convex).
        if trunk_depth(node.trunk, node.seed, node.end + offset) > 0.0 {
            offset = -offset;
        }
        let size = foliage.leaf_size
            * (1.0 + rng.range(-foliage.leaf_size_variation, foliage.leaf_size_variation));
        points.push(LeafPoint {
            position: node.end + offset,
            direction: offset.normalize_or_zero(),
            size,
            rotation: rng.range(0.0, TAU) * foliage.rotation_variation,
            tint: 0.0,
        });
    }
}

fn height_modifier(height_ratio: f32, foliage: &FoliageParams) -> f32 {
    if !foliage.use_crown_density {
        return 1.0;
    }
    let base = 1.0 - foliage.height_falloff;
    base + height_ratio * foliage.height_falloff
}

fn leaf_direction(node: &NodeCtx, foliage: &FoliageParams, rng: &mut SeededRng) -> Vec3 {
    let base_dir = match foliage.orientation {
        LeafOrientation::RadialOutward => {
            let horizontal = Vec3::new(node.end.x, 0.0, node.end.z);
            if horizontal.length_squared() > 0.001 {
                horizontal.normalize()
            } else {
                Vec3::X
            }
        }
        LeafOrientation::FollowBranch => node.direction,
        LeafOrientation::RandomUpward => Vec3::new(
            rng.range(-1.0, 1.0),
            rng.range(0.2, 1.0),
            rng.range(-1.0, 1.0),
        )
        .normalize_or_zero(),
        LeafOrientation::HorizontalSpread => {
            let angle = rng.range(0.0, TAU);
            Vec3::new(angle.cos(), 0.0, angle.sin())
        }
    };
    Vec3::new(base_dir.x, base_dir.y - foliage.droop, base_dir.z).normalize_or_zero()
}

/// Geometry for all `points` in `foliage.style`.
///
/// Each leaf gets a vertex colour (its tint scaled by
/// [`FoliageParams::color_variation`]), and every normal is bent toward
/// pointing out of the crown by [`FoliageParams::normal_bend`].
pub fn foliage_mesh(points: &[LeafPoint], foliage: &FoliageParams) -> MeshData {
    let mut mesh = MeshData::default();
    if points.is_empty() {
        return mesh;
    }
    let variation = foliage.color_variation.clamp(0.0, 0.5);
    for point in points {
        match foliage.style {
            LeafStyle::CrossedPlanes => crossed_planes(&mut mesh, point),
            LeafStyle::SingleQuad => single_quad(&mut mesh, point),
            LeafStyle::ClusterSphere => cluster_sphere(&mut mesh, point),
            LeafStyle::StarBurst => star_burst(&mut mesh, point),
            LeafStyle::NeedleCluster => needle_cluster(&mut mesh, point),
        }
        let factor = 1.0 + point.tint * variation;
        mesh.colors
            .resize(mesh.positions.len(), Vec4::new(factor, factor, factor, 1.0));
    }

    let bend = foliage.normal_bend.clamp(0.0, 1.0);
    if bend > 0.0 {
        let center = points.iter().map(|p| p.position).sum::<Vec3>() / points.len() as f32;
        for (normal, &position) in mesh.normals.iter_mut().zip(&mesh.positions) {
            let out = (position - center).normalize_or(Vec3::Y);
            *normal = normal.lerp(out, bend).normalize_or(out);
        }
    }
    mesh
}

/// Two leaf cards at 90° sharing `up`. Their bases meet at the leaf point and
/// the tips reach one `size` along `up`, so the leaf grows out of its stem
/// instead of being threaded on it.
fn crossed_planes(mesh: &mut MeshData, point: &LeafPoint) {
    let half = point.size * 0.5;
    let (right, up, forward) = build_basis(point.direction, point.rotation);
    let center = point.position + up * half;
    add_double_sided_leaf(mesh, center, right * half, up * half);
    add_double_sided_leaf(mesh, center, forward * half, up * half);
}

/// One leaf card, base at the leaf point and tip one `size` along `up`.
fn single_quad(mesh: &mut MeshData, point: &LeafPoint) {
    let half = point.size * 0.5;
    let (right, up, _) = build_basis(point.direction, point.rotation);
    add_double_sided_leaf(mesh, point.position + up * half, right * half, up * half);
}

/// Octahedron.
fn cluster_sphere(mesh: &mut MeshData, point: &LeafPoint) {
    let size = point.size * 0.4;
    let c = point.position;
    let top = c + Vec3::Y * size;
    let bottom = c - Vec3::Y * size;
    let front = c + Vec3::NEG_Z * size;
    let back = c + Vec3::Z * size;
    let left = c + Vec3::NEG_X * size;
    let right = c + Vec3::X * size;

    add_triangle_outward(mesh, c, top, front, right);
    add_triangle_outward(mesh, c, top, right, back);
    add_triangle_outward(mesh, c, top, back, left);
    add_triangle_outward(mesh, c, top, left, front);
    add_triangle_outward(mesh, c, bottom, right, front);
    add_triangle_outward(mesh, c, bottom, back, right);
    add_triangle_outward(mesh, c, bottom, left, back);
    add_triangle_outward(mesh, c, bottom, front, left);
}

/// Three quads at 60°.
fn star_burst(mesh: &mut MeshData, point: &LeafPoint) {
    let half = point.size * 0.5;
    let (_, up, _) = build_basis(point.direction, point.rotation);
    for i in 0..3 {
        let angle = (i as f32 / 3.0) * TAU + point.rotation;
        let right = Vec3::new(angle.cos(), 0.0, angle.sin());
        add_double_sided_leaf(mesh, point.position, right * half, up * half);
    }
}

/// Six thin radiating quads.
fn needle_cluster(mesh: &mut MeshData, point: &LeafPoint) {
    let length = point.size;
    let width = point.size * 0.1;
    let (_, _, forward) = build_basis(point.direction, point.rotation);
    for i in 0..6 {
        let angle = (i as f32 / 6.0) * TAU + point.rotation;
        let needle_dir = Vec3::new(angle.cos(), 0.3, angle.sin()).normalize();
        let needle_right = forward.cross(needle_dir).normalize_or_zero();
        add_double_sided_quad(
            mesh,
            point.position + needle_dir * length * 0.5,
            needle_right * width,
            needle_dir * length * 0.5,
        );
    }
}

/// Orthonormal `(right, up, forward)` with `forward = direction`, rotated by
/// `rotation` around `direction`.
fn build_basis(direction: Vec3, rotation: f32) -> (Vec3, Vec3, Vec3) {
    let world_up = if direction.y.abs() > 0.9 { Vec3::NEG_Z } else { Vec3::Y };
    let right = direction.cross(world_up).normalize_or_zero();
    let up = right.cross(direction).normalize_or_zero();
    let (sin_r, cos_r) = rotation.sin_cos();
    (right * cos_r + up * sin_r, up * cos_r - right * sin_r, direction)
}

/// Quad spanned by `half_right`/`half_up` around `center`, emitted twice with
/// opposite windings; each side's normal is derived from its winding.
fn add_double_sided_quad(mesh: &mut MeshData, center: Vec3, half_right: Vec3, half_up: Vec3) {
    let v0 = center - half_right - half_up;
    let v1 = center + half_right - half_up;
    let v2 = center + half_right + half_up;
    let v3 = center - half_right + half_up;
    let normal = half_right.cross(half_up).normalize_or_zero();
    let uvs = [
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 1.0),
    ];

    let front = mesh.positions.len() as u32;
    mesh.positions.extend_from_slice(&[v0, v1, v2, v3]);
    mesh.normals.extend_from_slice(&[normal; 4]);
    mesh.uvs.extend_from_slice(&uvs);
    mesh.indices
        .extend_from_slice(&[front, front + 1, front + 2, front, front + 2, front + 3]);

    let back = mesh.positions.len() as u32;
    mesh.positions.extend_from_slice(&[v0, v1, v2, v3]);
    mesh.normals.extend_from_slice(&[-normal; 4]);
    mesh.uvs.extend_from_slice(&uvs);
    mesh.indices
        .extend_from_slice(&[back, back + 2, back + 1, back, back + 3, back + 2]);
}

/// Pointed oval, base to tip, about half the card's area. Units are
/// half-extents: x along `half_right`, y along `half_up`, base at y = -1, tip
/// at y = 1. Counter-clockwise seen from the front (`half_right × half_up`).
const LEAF_OUTLINE: [Vec2; 8] = [
    Vec2::new(0.0, -1.0),
    Vec2::new(0.55, -0.6),
    Vec2::new(0.8, -0.05),
    Vec2::new(0.6, 0.5),
    Vec2::new(0.0, 1.0),
    Vec2::new(-0.6, 0.5),
    Vec2::new(-0.8, -0.05),
    Vec2::new(-0.55, -0.6),
];

/// Leaf-shaped card spanned by `half_right`/`half_up` around `center`: a fan
/// over [`LEAF_OUTLINE`], emitted twice with opposite windings; each side's
/// normal is derived from its winding.
fn add_double_sided_leaf(mesh: &mut MeshData, center: Vec3, half_right: Vec3, half_up: Vec3) {
    let normal = half_right.cross(half_up).normalize_or_zero();
    let n = LEAF_OUTLINE.len() as u32;
    for (side_normal, front) in [(normal, true), (-normal, false)] {
        let base = mesh.positions.len() as u32;
        mesh.positions.push(center);
        mesh.uvs.push(Vec2::splat(0.5));
        for p in LEAF_OUTLINE {
            mesh.positions.push(center + half_right * p.x + half_up * p.y);
            mesh.uvs.push((p + Vec2::ONE) * 0.5);
        }
        mesh.normals
            .extend(std::iter::repeat_n(side_normal, n as usize + 1));
        for i in 0..n {
            let a = base + 1 + i;
            let b = base + 1 + (i + 1) % n;
            if front {
                mesh.indices.extend_from_slice(&[base, a, b]);
            } else {
                mesh.indices.extend_from_slice(&[base, b, a]);
            }
        }
    }
}

/// Triangle wound so its face points away from `center`.
fn add_triangle_outward(mesh: &mut MeshData, center: Vec3, a: Vec3, b: Vec3, c: Vec3) {
    let geometric = (b - a).cross(c - a);
    let outward = (a + b + c) / 3.0 - center;
    let (v1, v2) = if geometric.dot(outward) >= 0.0 { (b, c) } else { (c, b) };
    let normal = (v1 - a).cross(v2 - a).normalize_or_zero();
    let base = mesh.positions.len() as u32;
    mesh.positions.extend_from_slice(&[a, v1, v2]);
    mesh.normals.extend_from_slice(&[normal; 3]);
    mesh.uvs.extend_from_slice(&[
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(0.5, 1.0),
    ]);
    mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::rings::tests::{assert_faces_outward, assert_indices_valid};
    use crate::params::TreeParams;
    use crate::presets::TreePreset;
    use crate::skeleton::generate_skeleton;

    fn point() -> LeafPoint {
        LeafPoint {
            position: Vec3::ZERO,
            direction: Vec3::Y,
            size: 1.0,
            rotation: 0.0,
            tint: 0.0,
        }
    }

    /// Foliage settings with flat per-card normals and uniform colour.
    fn flat(style: LeafStyle) -> FoliageParams {
        FoliageParams {
            style,
            normal_bend: 0.0,
            color_variation: 0.0,
            ..FoliageParams::default()
        }
    }

    #[test]
    fn build_basis_produces_orthonormal() {
        for dir in [Vec3::Y, Vec3::X, Vec3::new(0.3, 0.9, 0.1).normalize()] {
            let (right, up, forward) = build_basis(dir, 0.7);
            assert!((right.length() - 1.0).abs() < 1e-5);
            assert!((up.length() - 1.0).abs() < 1e-5);
            assert!(right.dot(up).abs() < 1e-5);
            assert!(right.dot(forward).abs() < 1e-5);
            assert!(up.dot(forward).abs() < 1e-5);
        }
    }

    #[test]
    fn leaf_vertex_counts() {
        let p = point();
        let count = |style| foliage_mesh(std::slice::from_ref(&p), &flat(style)).positions.len();
        assert_eq!(count(LeafStyle::CrossedPlanes), 36);
        assert_eq!(count(LeafStyle::SingleQuad), 18);
        assert_eq!(count(LeafStyle::ClusterSphere), 24);
        assert_eq!(count(LeafStyle::StarBurst), 54);
        assert_eq!(count(LeafStyle::NeedleCluster), 48);
    }

    #[test]
    fn leaf_meshes_face_outward() {
        // Flat normals: bent normals deliberately disagree with the winding.
        let p = LeafPoint {
            position: Vec3::new(1.0, 2.0, -0.5),
            direction: Vec3::new(0.4, -0.3, 0.7).normalize(),
            size: 0.8,
            rotation: 1.1,
            tint: 0.0,
        };
        for style in [
            LeafStyle::CrossedPlanes,
            LeafStyle::SingleQuad,
            LeafStyle::ClusterSphere,
            LeafStyle::StarBurst,
            LeafStyle::NeedleCluster,
        ] {
            let mesh = foliage_mesh(std::slice::from_ref(&p), &flat(style));
            assert_indices_valid(&mesh, &format!("{style:?}"));
            assert_faces_outward(&mesh, &format!("{style:?}"));
        }
    }

    #[test]
    fn leaf_cards_grow_out_of_their_attachment_point() {
        // D-045: cards centred on the leaf point were threaded on their twig
        // and read as green rings around thick limbs.
        let p = LeafPoint {
            position: Vec3::new(1.0, 2.0, -0.5),
            direction: Vec3::new(0.4, -0.3, 0.7).normalize(),
            size: 0.8,
            rotation: 1.1,
            tint: 0.0,
        };
        let (_, up, _) = build_basis(p.direction, p.rotation);
        for style in [LeafStyle::CrossedPlanes, LeafStyle::SingleQuad] {
            let mesh = foliage_mesh(std::slice::from_ref(&p), &flat(style));
            let (min, max) = mesh
                .positions
                .iter()
                .map(|v| (*v - p.position).dot(up))
                .fold((f32::MAX, f32::MIN), |(lo, hi), a| (lo.min(a), hi.max(a)));
            assert!(min > -1e-5, "{style:?}: card reaches {min} m behind the leaf point");
            assert!((max - p.size).abs() < 1e-5, "{style:?}: tip {max} m out, leaf size {}", p.size);
        }
    }

    fn oak_leaf_points(placement: FoliagePlacement, age: f32) -> (TreeParams, Vec<LeafPoint>) {
        let mut params = TreePreset::Oak.params();
        params.foliage.placement = placement;
        let skeleton = generate_skeleton(&params).aged(age, &params.growth);
        let points = collect_leaf_points(&skeleton, &params.foliage, &params.growth, &params.trunk, params.seed);
        (params, points)
    }

    #[test]
    fn bent_normals_point_out_of_the_crown() {
        let (params, points) = oak_leaf_points(FoliagePlacement::AllBranches, f32::INFINITY);
        assert!(!points.is_empty());
        let center = points.iter().map(|p| p.position).sum::<Vec3>() / points.len() as f32;
        for normal_bend in [params.foliage.normal_bend, 1.0] {
            let foliage = FoliageParams { normal_bend, ..params.foliage.clone() };
            let mesh = foliage_mesh(&points, &foliage);
            assert_indices_valid(&mesh, "oak");
            for (position, normal) in mesh.positions.iter().zip(&mesh.normals) {
                assert!((normal.length() - 1.0).abs() < 1e-4, "bend {normal_bend}: non-unit normal {normal:?}");
                assert!(
                    normal.dot(*position - center) > 0.0,
                    "bend {normal_bend}: normal {normal:?} at {position:?} points into the crown"
                );
            }
        }
    }

    #[test]
    fn leaf_colors_vary_per_leaf_within_bounds() {
        let (params, points) = oak_leaf_points(FoliagePlacement::AllBranches, f32::INFINITY);
        assert_eq!(params.foliage.style, LeafStyle::CrossedPlanes);
        let leaf_vertices = 36;

        let varied = FoliageParams { color_variation: 0.2, ..params.foliage.clone() };
        let mesh = foliage_mesh(&points, &varied);
        assert_eq!(mesh.colors.len(), mesh.positions.len());
        assert_eq!(mesh.colors.len(), points.len() * leaf_vertices);
        let mut distinct = std::collections::BTreeSet::new();
        for leaf in mesh.colors.chunks(leaf_vertices) {
            assert!(leaf.iter().all(|c| *c == leaf[0]), "one leaf, several colours");
            let c = leaf[0];
            assert!((0.8 - 1e-5..=1.2 + 1e-5).contains(&c.x), "factor {} out of bounds", c.x);
            assert!(c.x == c.y && c.y == c.z && c.w == 1.0, "tint must be grey and opaque: {c:?}");
            distinct.insert(c.x.to_bits());
        }
        assert!(distinct.len() > 1, "all leaves share one colour");

        let plain = FoliageParams { color_variation: 0.0, ..params.foliage.clone() };
        let mesh = foliage_mesh(&points, &plain);
        assert!(mesh.colors.iter().all(|c| *c == Vec4::ONE));
    }

    #[test]
    fn leaf_tints_never_reroll_as_the_tree_ages() {
        let tints = |age: f32| -> std::collections::BTreeSet<u32> {
            let (_, points) = oak_leaf_points(FoliagePlacement::TerminalBranches, age);
            points.iter().map(|p| p.tint.to_bits()).collect()
        };
        let young = tints(10.0);
        let mature = tints(f32::INFINITY);
        assert!(young.len() > 1, "no tints drawn at age 10");
        assert!(young.is_subset(&mature), "a leaf's tint changed while the tree aged");
    }

    #[test]
    fn collect_leaf_points_disabled() {
        let params = TreePreset::Oak.params();
        let skeleton = generate_skeleton(&params);
        let foliage = FoliageParams {
            enabled: false,
            ..params.foliage.clone()
        };
        let points = collect_leaf_points(&skeleton, &foliage, &params.growth, &params.trunk, 42);
        assert!(points.is_empty());
    }

    #[test]
    fn leaves_scale_with_age() {
        let params = TreePreset::Oak.params();
        let full = generate_skeleton(&params);
        let leaves_at = |age: f32| {
            let aged = full.aged(age, &params.growth);
            collect_leaf_points(&aged, &params.foliage, &params.growth, &params.trunk, params.seed)
        };
        let young = leaves_at(2.0);
        let mid = leaves_at(params.growth.maturity_age * 0.3);
        let mature = leaves_at(f32::INFINITY);
        assert!(!mature.is_empty());
        assert!(young.len() < mature.len(), "{} vs {}", young.len(), mature.len());
        let max_size = |pts: &[LeafPoint]| pts.iter().map(|p| p.size).fold(0.0, f32::max);
        assert!(max_size(&mid) < max_size(&mature));
    }

    #[test]
    fn leaf_points_stable_under_aging() {
        let mut params = TreePreset::Oak.params();
        params.foliage.placement = FoliagePlacement::AllBranches;
        params.foliage.orientation = LeafOrientation::FollowBranch;
        let full = generate_skeleton(&params);
        let (idx, primary) = full
            .nodes
            .iter()
            .enumerate()
            .find(|(_, n)| n.kind == NodeKind::Primary)
            .unwrap();
        let id = primary.id;
        let relative_leaves = |age: f32| {
            let aged = full.aged(age, &params.growth);
            let i = aged.nodes.iter().position(|n| n.id == id).expect("primary alive");
            let mut pts = Vec::new();
            leaf_points_for_node(&aged, i, &params.foliage, &params.growth, &params.trunk, params.seed, &mut pts);
            let end = aged.end(i);
            pts.into_iter()
                .map(|p| (p.position - end, p.direction, p.size, p.rotation))
                .collect::<Vec<_>>()
        };
        let a = relative_leaves(primary.birth + primary.duration);
        let b = relative_leaves(f32::INFINITY);
        assert!(!a.is_empty(), "node {idx} should have leaves");
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert!(x.0.distance(y.0) < 1e-4);
            assert_eq!(x.1, y.1);
            assert_eq!(x.2, y.2);
            assert_eq!(x.3, y.3);
        }
    }

    #[test]
    fn leaves_sit_on_bent_wood() {
        let mut params = TreePreset::Oak.params();
        params.branches.gravity_strength = 0.8;
        params.branches.stiffness = 0.1;
        let skeleton = generate_skeleton(&params);
        let axes: Vec<Vec<Vec3>> = skeleton
            .nodes
            .iter()
            .skip(1)
            .map(|n| n.axis(crate::skeleton::BENT_AXIS_POINTS).into_iter().map(|(c, _)| c).collect())
            .collect();
        let off_wood = |p: Vec3| {
            axes.iter()
                .flat_map(|axis| axis.windows(2))
                .map(|w| {
                    let ab = w[1] - w[0];
                    let t = ((p - w[0]).dot(ab) / ab.length_squared().max(f32::MIN_POSITIVE)).clamp(0.0, 1.0);
                    p.distance(w[0] + ab * t)
                })
                .fold(f32::MAX, f32::min)
        };
        for placement in [FoliagePlacement::AllBranches, FoliagePlacement::TerminalBranches] {
            params.foliage.placement = placement;
            let points = collect_leaf_points(&skeleton, &params.foliage, &params.growth, &params.trunk, params.seed);
            assert!(!points.is_empty());
            for p in &points {
                let d = off_wood(p.position);
                assert!(d < 1e-4, "{placement:?}: leaf at {:?} is {d} off the bent wood", p.position);
            }
        }
    }

    #[test]
    fn no_leaf_inside_the_trunk() {
        // Cypress twigs hug the trunk, so its tip clusters reach toward it.
        for preset in [TreePreset::Cypress, TreePreset::Poplar, TreePreset::Oak] {
            for placement in [FoliagePlacement::TipClusters, FoliagePlacement::TerminalBranches, FoliagePlacement::AllBranches] {
                let mut params = preset.params();
                params.foliage.placement = placement;
                let skeleton = generate_skeleton(&params);
                let points = collect_leaf_points(&skeleton, &params.foliage, &params.growth, &params.trunk, params.seed);
                for p in &points {
                    let depth = trunk_depth(&params.trunk, params.seed, p.position);
                    assert!(depth <= 0.0, "{} {placement:?}: leaf {depth} m inside the trunk", preset.name());
                }
            }
        }
    }

    #[test]
    fn young_tip_clusters_stay_on_their_tips() {
        // D-037: the cluster ball must shrink with its leaves while the tree
        // is young, or a 1 cm leaf floats tens of cm off its twig.
        for preset in [TreePreset::DragonTree, TreePreset::Oak, TreePreset::Cypress] {
            let mut params = preset.params();
            params.foliage.placement = FoliagePlacement::TipClusters;
            let mature = generate_skeleton(&params);
            for age in [4.0, 6.0, 9.0, 12.0] {
                let skeleton = mature.aged(age, &params.growth);
                let tips: Vec<Vec3> = (0..skeleton.nodes.len())
                    .filter(|&i| skeleton.nodes[i].kind != NodeKind::Trunk && skeleton.is_terminal(i))
                    .map(|i| skeleton.end(i))
                    .collect();
                let points = collect_leaf_points(&skeleton, &params.foliage, &params.growth, &params.trunk, params.seed);
                let max_ratio = 0.75 / (1.0 - params.foliage.leaf_size_variation) + 1e-3;
                for p in &points {
                    let d = tips.iter().map(|t| t.distance(p.position)).fold(f32::MAX, f32::min);
                    assert!(
                        d <= p.size * max_ratio + 1e-5,
                        "{} age {age}: leaf of size {} sits {d} m from the nearest tip",
                        preset.name(),
                        p.size
                    );
                }
            }
        }
    }

    #[test]
    fn leaves_never_disappear_as_a_tree_ages() {
        // D-039: tip placements must not drop a branch's leaves when its
        // first (still leafless) twigs are born; baobab went from 80 leaves
        // at age 6 to none at age 8.
        for preset in TreePreset::ALL {
            for placement in [FoliagePlacement::TerminalBranches, FoliagePlacement::TipClusters] {
                let mut params = preset.params();
                params.foliage.placement = placement;
                let mature = generate_skeleton(&params);
                let mut previous = 0;
                for step in 0..=40 {
                    let age = step as f32 * 0.5;
                    let skeleton = mature.aged(age, &params.growth);
                    let count = collect_leaf_points(&skeleton, &params.foliage, &params.growth, &params.trunk, params.seed).len();
                    assert!(
                        count >= previous,
                        "{} {placement:?}: {previous} leaves before age {age}, {count} at it",
                        preset.name()
                    );
                    previous = count;
                }
            }
        }
    }
}
