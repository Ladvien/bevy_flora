//! Laplacian smoothing and normal recomputation.

use std::collections::HashMap;

use bevy::math::Vec3;

/// Vertices closer than this fraction of the mesh's extent (at least 1 m) are
/// the same surface point. UV seams and cap rings repeat positions (to within
/// f32 rounding), and those repeats must stay together; a fixed tolerance
/// would either tear seams on large trees or merge distinct ring vertices of
/// sub-millimetre twigs, folding them inside out.
const WELD_RELATIVE: f32 = 1.0e-6;

/// Wood whose edges are shorter than this many weld tolerances is left as
/// generated: hairline twigs, whose ring vertices are within a few f32 steps
/// of each other, would otherwise be partly welded and fold inside out.
const MIN_SMOOTH_EDGE: f32 = 100.0;

/// Taubin pass-band frequency `k_PB`: each shrinking pass (`λ`) is followed by
/// an inflating one (`μ`, with `1/λ + 1/μ = k_PB`), so noise is smoothed out
/// without the volume loss of plain Laplacian smoothing (which retracts
/// branch ends and thins limbs away from their twigs).
const TAUBIN_PASS_BAND: f32 = 0.1;

/// Taubin λ|μ smoothing: each of the `iterations` moves every interior vertex
/// toward the barycenter of its edge neighbours by `factor` (0–1), then away
/// from it by the matching μ. Normals are recomputed from the faces around
/// moved vertices only; unmoved wood keeps its generated normals, so pinned
/// pieces (collar ends, branch bases) shade exactly as before.
///
/// Vertices at the same position (UV seams) move as one, so seams stay closed.
/// Vertices on an open boundary (an edge used by a single triangle, such as
/// the base of a branch tube), vertices with an edge shorter than 100 weld
/// tolerances (wood too fine for f32 to tell its ring vertices from seam
/// copies) and vertices for which `fixed(position)` holds do not
/// move, so branches stay attached, hairline twigs cannot fold, and the trunk
/// stays on the ground.
pub fn laplacian_smooth(
    positions: &mut [Vec3],
    normals: &mut [Vec3],
    indices: &[u32],
    iterations: u32,
    factor: f32,
    fixed: impl Fn(Vec3) -> bool,
) {
    if positions.is_empty() || indices.is_empty() || iterations == 0 {
        return;
    }
    let extent = positions.iter().fold(1.0f32, |m, p| m.max(p.abs().max_element()));
    let epsilon = WELD_RELATIVE * extent;
    let (group_of, mut points) = weld(positions, epsilon);
    let mut topology = Topology::build(points.len(), &group_of, indices);
    let min_edge = MIN_SMOOTH_EDGE * epsilon;
    for (g, &point) in points.iter().enumerate() {
        let fine = topology.neighbors[g]
            .iter()
            .any(|&n| points[n as usize].distance_squared(point) < min_edge * min_edge);
        topology.pinned[g] |= fine || fixed(point);
    }
    let inflate = factor / (TAUBIN_PASS_BAND * factor - 1.0);
    for _ in 0..iterations {
        smooth_pass(&mut points, &topology, factor);
        smooth_pass(&mut points, &topology, inflate);
    }
    let mut moved = vec![false; positions.len()];
    for ((position, &group), moved) in positions.iter_mut().zip(&group_of).zip(&mut moved) {
        let point = points[group as usize];
        if !topology.pinned[group as usize] && *position != point {
            *position = point;
            *moved = true;
        }
    }
    refresh_normals(positions, indices, normals, &moved, &group_of, points.len());
}

/// Merge vertices within `epsilon`. Returns each vertex's group index and each
/// group's position (that of its first vertex).
fn weld(positions: &[Vec3], epsilon: f32) -> (Vec<u32>, Vec<Vec3>) {
    let cell_of = |p: Vec3| (p / epsilon).floor().as_ivec3().to_array();
    let mut cells: HashMap<[i32; 3], Vec<u32>> = HashMap::new();
    let mut group_positions: Vec<Vec3> = Vec::new();
    let mut group_of = Vec::with_capacity(positions.len());
    for &p in positions {
        let [cx, cy, cz] = cell_of(p);
        let existing = (-1..=1)
            .flat_map(|dx| (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| [cx + dx, cy + dy, cz + dz])))
            .filter_map(|cell| cells.get(&cell))
            .flatten()
            .copied()
            .find(|&g| group_positions[g as usize].distance_squared(p) <= epsilon * epsilon);
        let group = existing.unwrap_or_else(|| {
            let g = group_positions.len() as u32;
            group_positions.push(p);
            cells.entry([cx, cy, cz]).or_default().push(g);
            g
        });
        group_of.push(group);
    }
    (group_of, group_positions)
}

/// Edge neighbours and open-boundary flags per welded vertex group.
struct Topology {
    neighbors: Vec<Vec<u32>>,
    pinned: Vec<bool>,
}

impl Topology {
    fn build(group_count: usize, group_of: &[u32], indices: &[u32]) -> Self {
        let mut edge_uses: HashMap<(u32, u32), u32> = HashMap::new();
        for &[a, b, c] in indices.as_chunks::<3>().0 {
            let (ga, gb, gc) = (group_of[a as usize], group_of[b as usize], group_of[c as usize]);
            if ga == gb || gb == gc || ga == gc {
                continue; // collapsed triangle (zero-radius tip)
            }
            for (u, v) in [(ga, gb), (gb, gc), (gc, ga)] {
                *edge_uses.entry((u.min(v), u.max(v))).or_insert(0) += 1;
            }
        }
        let mut neighbors = vec![Vec::new(); group_count];
        let mut pinned = vec![false; group_count];
        for (&(u, v), &uses) in &edge_uses {
            neighbors[u as usize].push(v);
            neighbors[v as usize].push(u);
            if uses == 1 {
                pinned[u as usize] = true;
                pinned[v as usize] = true;
            }
        }
        // HashMap order varies per process; sorted lists keep the f32 sums,
        // and so the mesh, bit-for-bit reproducible.
        for list in &mut neighbors {
            list.sort_unstable();
        }
        Self { neighbors, pinned }
    }
}

fn smooth_pass(points: &mut [Vec3], topology: &Topology, factor: f32) {
    let next: Vec<Vec3> = points
        .iter()
        .zip(&topology.neighbors)
        .zip(&topology.pinned)
        .map(|((&p, neighbors), &pinned)| {
            if pinned || neighbors.is_empty() {
                return p;
            }
            let sum = neighbors
                .iter()
                .fold(Vec3::ZERO, |acc, &n| acc + points[n as usize]);
            p.lerp(sum / neighbors.len() as f32, factor)
        })
        .collect();
    points.copy_from_slice(&next);
}

/// Generated normals closer than this (cosine) belong to one smooth surface;
/// further apart they are a hard edge (a cap rim) and stay separate.
const SHARED_NORMAL_COS: f32 = 0.5;

/// Area-weighted normals from the faces around every vertex that shares a
/// face with a moved vertex; all other normals are left as they are. Welded
/// copies of a vertex (UV seam twins) pool their faces when their generated
/// normals agree, so a seam does not show; hard edges stay hard.
fn refresh_normals(
    positions: &[Vec3],
    indices: &[u32],
    normals: &mut [Vec3],
    moved: &[bool],
    group_of: &[u32],
    group_count: usize,
) {
    if positions.len() != normals.len() {
        return;
    }
    let triangles = indices.as_chunks::<3>().0;
    let mut stale = vec![false; positions.len()];
    for &[a, b, c] in triangles {
        if moved[a as usize] || moved[b as usize] || moved[c as usize] {
            for v in [a, b, c] {
                stale[v as usize] = true;
            }
        }
    }
    let mut sums = vec![Vec3::ZERO; positions.len()];
    for &[a, b, c] in triangles {
        let (a, b, c) = (a as usize, b as usize, c as usize);
        let face = (positions[b] - positions[a]).cross(positions[c] - positions[a]);
        sums[a] += face;
        sums[b] += face;
        sums[c] += face;
    }
    let mut members: Vec<Vec<u32>> = vec![Vec::new(); group_count];
    for (v, &g) in group_of.iter().enumerate() {
        members[g as usize].push(v as u32);
    }
    let generated = normals.to_vec();
    let agrees = |u: usize, v: usize| generated[u].dot(generated[v]) > SHARED_NORMAL_COS;
    // A refreshed vertex takes its agreeing twins along, so they keep shading
    // alike even when only one of them touches a moved face.
    for v in 0..positions.len() {
        if stale[v] {
            for &u in &members[group_of[v] as usize] {
                if agrees(u as usize, v) {
                    stale[u as usize] = true;
                }
            }
        }
    }
    for v in (0..positions.len()).filter(|&v| stale[v]) {
        let sum = members[group_of[v] as usize]
            .iter()
            .map(|&u| u as usize)
            .filter(|&u| agrees(u, v))
            .fold(Vec3::ZERO, |acc, u| acc + sums[u]);
        if sum.length_squared() > 0.0001 {
            normals[v] = sum.normalize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laplacian_smooth_lowers_peak() {
        let mut positions = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.5, 1.0, 0.5),
        ];
        let indices = [0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4];
        let before = positions[4].y;
        let mut normals = vec![Vec3::Y; positions.len()];
        laplacian_smooth(&mut positions, &mut normals, &indices, 1, 0.5, |_| false);
        assert!(positions[4].y < before);
    }

    #[test]
    fn zero_iterations_is_identity() {
        let mut positions = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        let original = positions.clone();
        let mut normals = vec![Vec3::Z; 3];
        laplacian_smooth(&mut positions, &mut normals, &[0, 1, 2], 0, 0.5, |_| false);
        assert_eq!(positions, original);
    }

    #[test]
    fn refreshed_normals_point_up() {
        let positions = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
        ];
        let mut normals = vec![Vec3::X; 3];
        refresh_normals(&positions, &[0, 1, 2], &mut normals, &[true, false, false], &[0, 1, 2], 3);
        for n in &normals {
            assert!(n.y > 0.5);
        }
    }

    /// Three rings of a 4-sided tube; each ring repeats its first vertex as a
    /// UV seam duplicate (5 vertices per ring), like `emit_ring` does.
    fn seamed_tube() -> (Vec<Vec3>, Vec<u32>) {
        let mut positions = Vec::new();
        for y in 0..3 {
            for k in 0..=4 {
                let a = (k % 4) as f32 * std::f32::consts::FRAC_PI_2;
                positions.push(Vec3::new(a.cos(), y as f32, a.sin()));
            }
        }
        let mut indices = Vec::new();
        for ring in 0..2u32 {
            for k in 0..4u32 {
                let (a, b) = (ring * 5 + k, (ring + 1) * 5 + k);
                indices.extend([a, b, a + 1, a + 1, b, b + 1]);
            }
        }
        (positions, indices)
    }

    #[test]
    fn seam_duplicates_move_together() {
        let (mut positions, indices) = seamed_tube();
        let before = positions.clone();
        let mut normals = vec![Vec3::Y; positions.len()];
        laplacian_smooth(&mut positions, &mut normals, &indices, 2, 0.5, |_| false);
        for ring in 0..3 {
            let (first, seam) = (positions[ring * 5], positions[ring * 5 + 4]);
            assert!((first - seam).length() < 1e-6, "ring {ring} seam torn: {first:?} vs {seam:?}");
        }
        // The free middle ring really smoothed (shrank toward the axis).
        assert!(positions[5].x < before[5].x - 0.01);
    }

    #[test]
    fn open_boundary_stays_put() {
        let (mut positions, indices) = seamed_tube();
        let before = positions.clone();
        let mut normals = vec![Vec3::Y; positions.len()];
        laplacian_smooth(&mut positions, &mut normals, &indices, 3, 0.5, |_| false);
        for v in (0..5).chain(10..15) {
            assert_eq!(positions[v], before[v], "boundary vertex {v} moved");
        }
    }

    #[test]
    fn fixed_vertices_stay_put() {
        let (mut positions, indices) = seamed_tube();
        let before = positions.clone();
        let mut normals = vec![Vec3::Y; positions.len()];
        laplacian_smooth(&mut positions, &mut normals, &indices, 3, 0.5, |p| p.y == 1.0);
        for v in 5..10 {
            assert_eq!(positions[v], before[v], "fixed vertex {v} moved");
        }
    }
}
