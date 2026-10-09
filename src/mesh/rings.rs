//! Shared ring / quad-strip / cap emitters.
//!
//! Winding convention (single source of truth for every mesher): front faces
//! are counter-clockwise under the right-hand rule, i.e. for every triangle
//! `(v0, v1, v2)`: `cross(v1 - v0, v2 - v0) · normal > 0`. Bevy/wgpu cull
//! clockwise faces as back faces.

use bevy::math::{Vec2, Vec3};
use std::f32::consts::TAU;

use super::MeshData;

/// Orthonormal pair spanning a ring's plane. `(right, forward, axis)` must be
/// right-handed (`right × forward = axis`) for [`connect_rings`] to face outward.
#[derive(Clone, Copy, Debug)]
pub struct RingBasis {
    pub right: Vec3,
    pub forward: Vec3,
}

impl RingBasis {
    /// Right-handed basis perpendicular to `axis`.
    pub fn from_axis(axis: Vec3) -> Self {
        let right = perpendicular(axis);
        Self {
            right,
            forward: axis.cross(right),
        }
    }

    /// Axis of this basis (`right × forward`).
    pub fn axis(self) -> Vec3 {
        self.right.cross(self.forward)
    }
}

/// A unit vector perpendicular to `dir`.
pub fn perpendicular(dir: Vec3) -> Vec3 {
    let up = if dir.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    dir.cross(up).normalize_or_zero()
}

/// Emit `segments + 1` vertices (seam duplicated) around `center`.
///
/// Vertex `i` sits at `angle = i / segments * TAU + twist` with radius
/// `radius_at(i, angle)`, normal `right·cos + forward·sin`, uv `(i / segments, v)`.
/// Returns the index of the first vertex.
pub fn emit_ring(
    mesh: &mut MeshData,
    center: Vec3,
    basis: RingBasis,
    segments: u32,
    twist: f32,
    v: f32,
    radius_at: impl Fn(u32, f32) -> f32,
) -> u32 {
    let first = mesh.positions.len() as u32;
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * TAU + twist;
        let (sin, cos) = angle.sin_cos();
        let radial = basis.right * cos + basis.forward * sin;
        let r = radius_at(i, angle);
        mesh.positions.push(center + radial * r);
        mesh.normals.push(radial);
        mesh.uvs.push(Vec2::new(i as f32 / segments as f32, v));
    }
    first
}

/// Quad strip between ring `a` (lower) and ring `b` (upper along the axis).
pub fn connect_rings(mesh: &mut MeshData, ring_a: u32, ring_b: u32, segments: u32) {
    for i in 0..segments {
        let a0 = ring_a + i;
        let a1 = ring_a + i + 1;
        let b0 = ring_b + i;
        let b1 = ring_b + i + 1;
        mesh.indices.extend_from_slice(&[a0, a1, b0, a1, b1, b0]);
    }
}

/// Fan cap: a center vertex plus a ring of `segments + 1` vertices, all with
/// `normal`. Triangle order is chosen so faces point along `normal`.
pub fn emit_cap(
    mesh: &mut MeshData,
    center: Vec3,
    basis: RingBasis,
    segments: u32,
    twist: f32,
    normal: Vec3,
    radius_at: impl Fn(u32, f32) -> f32,
) {
    let center_idx = mesh.positions.len() as u32;
    mesh.positions.push(center);
    mesh.normals.push(normal);
    mesh.uvs.push(Vec2::new(0.5, 0.5));

    for i in 0..=segments {
        let base_angle = (i as f32 / segments as f32) * TAU;
        let angle = base_angle + twist;
        let radial = basis.right * angle.cos() + basis.forward * angle.sin();
        mesh.positions.push(center + radial * radius_at(i, angle));
        mesh.normals.push(normal);
        mesh.uvs.push(Vec2::new(
            0.5 + base_angle.cos() * 0.5,
            0.5 + base_angle.sin() * 0.5,
        ));
    }

    let ccw = basis.axis().dot(normal) > 0.0;
    let ring = center_idx + 1;
    for i in 0..segments {
        let (p, q) = if ccw { (ring + i, ring + i + 1) } else { (ring + i + 1, ring + i) };
        mesh.indices.extend_from_slice(&[center_idx, p, q]);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Every triangle's geometric normal agrees with its stored vertex normals.
    pub(crate) fn assert_faces_outward(mesh: &MeshData, what: &str) {
        for (t, &[a, b, c]) in mesh.indices.as_chunks::<3>().0.iter().enumerate() {
            let (a, b, c) = (a as usize, b as usize, c as usize);
            let (v0, v1, v2) = (mesh.positions[a], mesh.positions[b], mesh.positions[c]);
            let geometric = (v1 - v0).cross(v2 - v0);
            if geometric.length_squared() < 1e-12 {
                continue; // degenerate (e.g. zero-radius tip)
            }
            let stored = mesh.normals[a] + mesh.normals[b] + mesh.normals[c];
            assert!(
                geometric.dot(stored) > 0.0,
                "{what}: triangle {t} is inside-out (geometric {geometric:?}, stored {stored:?})"
            );
        }
    }

    pub(crate) fn assert_indices_valid(mesh: &MeshData, what: &str) {
        assert_eq!(mesh.indices.len() % 3, 0, "{what}: index count not a multiple of 3");
        assert_eq!(mesh.normals.len(), mesh.positions.len(), "{what}: normals");
        assert_eq!(mesh.uvs.len(), mesh.positions.len(), "{what}: uvs");
        let max = mesh.indices.iter().copied().max().unwrap_or(0) as usize;
        assert!(
            mesh.indices.is_empty() || max < mesh.positions.len(),
            "{what}: index {max} out of range"
        );
        for p in &mesh.positions {
            assert!(p.is_finite(), "{what}: non-finite position {p:?}");
        }
        for n in &mesh.normals {
            assert!(n.is_finite(), "{what}: non-finite normal {n:?}");
        }
    }

    #[test]
    fn tube_faces_outward_for_any_axis() {
        for axis in [Vec3::Y, Vec3::X, Vec3::NEG_Y, Vec3::new(0.3, -0.5, 0.8).normalize()] {
            let mut mesh = MeshData::default();
            let basis = RingBasis::from_axis(axis);
            let a = emit_ring(&mut mesh, Vec3::ZERO, basis, 8, 0.3, 0.0, |_, _| 1.0);
            let b = emit_ring(&mut mesh, axis * 2.0, basis, 8, 0.5, 1.0, |_, _| 0.5);
            connect_rings(&mut mesh, a, b, 8);
            emit_cap(&mut mesh, axis * 2.0, basis, 8, 0.5, axis, |_, _| 0.5);
            emit_cap(&mut mesh, Vec3::ZERO, basis, 8, 0.3, -axis, |_, _| 1.0);
            assert_indices_valid(&mesh, "tube");
            assert_faces_outward(&mesh, "tube");
        }
    }

    #[test]
    fn trunk_basis_is_right_handed() {
        let basis = RingBasis {
            right: Vec3::X,
            forward: Vec3::NEG_Z,
        };
        assert!(basis.axis().abs_diff_eq(Vec3::Y, 1e-6));
    }
}
