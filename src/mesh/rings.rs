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
    /// Unit vector along the ring plane's first axis.
    pub right: Vec3,
    /// Unit vector along the ring plane's second axis, perpendicular to `right`.
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

/// Where a ring of wood sits in its bark scan's texture space, in tiles of the scan.
///
/// Wood is laid out in metres anchored to the *mature* tree (see
/// [`BarkParams`](crate::params::BarkParams)): `v` is the ring's distance along the wood in
/// the tile's lengths, `u` runs a whole number of tiles around the ring, so the texture is
/// continuous over the seam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RingUv {
    /// `v` of every vertex of the ring.
    pub v: f32,
    /// Tiles around the ring: `u` runs from 0 to this value. A whole number, at least 1.
    pub u_tiles: f32,
}

/// A whole number of tiles, at least one, that fit around a ring of `mature_radius` metres:
/// the bark's features keep their real size to within half a tile of rounding.
pub fn u_tiles_around(mature_radius: f32, tile_width: f32) -> f32 {
    (TAU * mature_radius / tile_width.max(1.0e-3)).round().max(1.0)
}

/// What an aged `size` (a length or radius, scaled by `progress` when the tree was aged) was
/// when fully grown. A node that has not begun to grow (`progress` practically zero) keeps its
/// aged size, which is zero too: a quotient there would only amplify rounding.
pub(crate) fn mature_size(size: f32, progress: f32) -> f32 {
    if progress < 1.0e-9 { size } else { size / progress }
}

/// Emit `segments + 1` vertices (seam duplicated) around `center`.
///
/// Vertex `i` sits at `angle = i / segments * TAU + twist` with radius
/// `radius_at(i, angle)`, normal `right·cos + forward·sin`, uv `(i / segments * u_tiles, v)`
/// and `uv1` `(radius, 1)` (the shaders fade the bark in with the radius).
/// Returns the index of the first vertex.
pub fn emit_ring(
    mesh: &mut MeshData,
    center: Vec3,
    basis: RingBasis,
    segments: u32,
    twist: f32,
    uv: RingUv,
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
        mesh.uvs.push(Vec2::new(i as f32 / segments as f32 * uv.u_tiles, uv.v));
        mesh.uv1.push(Vec2::new(r, 1.0));
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
///
/// The texture coordinates lay the cap out as a disc around `(0.5, 0.5)` whose rim sits
/// `uv_radius` away (in tiles, so the metric size of the bark on the cap is kept: the cap's
/// mature radius over the tile's size). `uv1.x` is the vertex's radius, as for rings.
#[allow(clippy::too_many_arguments)]
pub fn emit_cap(
    mesh: &mut MeshData,
    center: Vec3,
    basis: RingBasis,
    segments: u32,
    twist: f32,
    normal: Vec3,
    uv_radius: Vec2,
    radius_at: impl Fn(u32, f32) -> f32,
) {
    let center_idx = mesh.positions.len() as u32;
    mesh.positions.push(center);
    mesh.normals.push(normal);
    mesh.uvs.push(Vec2::new(0.5, 0.5));
    mesh.uv1.push(Vec2::new(radius_at(0, twist), 1.0));

    for i in 0..=segments {
        let base_angle = (i as f32 / segments as f32) * TAU;
        let angle = base_angle + twist;
        let radial = basis.right * angle.cos() + basis.forward * angle.sin();
        let r = radius_at(i, angle);
        mesh.positions.push(center + radial * r);
        mesh.normals.push(normal);
        mesh.uvs.push(Vec2::new(0.5, 0.5) + Vec2::new(base_angle.cos(), base_angle.sin()) * uv_radius);
        mesh.uv1.push(Vec2::new(r, 1.0));
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
        assert!(mesh.uv1.is_empty() || mesh.uv1.len() == mesh.positions.len(), "{what}: uv1");
        assert!(
            mesh.colors.is_empty() || mesh.colors.len() == mesh.positions.len(),
            "{what}: colors"
        );
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
            let a = emit_ring(&mut mesh, Vec3::ZERO, basis, 8, 0.3, RingUv { v: 0.0, u_tiles: 1.0 }, |_, _| 1.0);
            let b = emit_ring(&mut mesh, axis * 2.0, basis, 8, 0.5, RingUv { v: 1.0, u_tiles: 1.0 }, |_, _| 0.5);
            connect_rings(&mut mesh, a, b, 8);
            emit_cap(&mut mesh, axis * 2.0, basis, 8, 0.5, axis, Vec2::splat(0.5), |_, _| 0.5);
            emit_cap(&mut mesh, Vec3::ZERO, basis, 8, 0.3, -axis, Vec2::splat(1.0), |_, _| 1.0);
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

    #[test]
    fn tiles_around_a_ring_are_whole_and_at_least_one() {
        for radius in [0.0, 0.001, 0.05, 0.3, 1.2, 4.0] {
            for tile in [0.5, 1.0, 1.86, 2.0] {
                let tiles = u_tiles_around(radius, tile);
                assert!(tiles >= 1.0 && tiles.fract() == 0.0, "radius {radius} tile {tile}: {tiles}");
                // Bark keeps its real width to within half a tile of rounding.
                if tiles > 1.0 {
                    assert!((tiles * tile - TAU * radius).abs() <= tile / 2.0 + 1e-4, "radius {radius} tile {tile}");
                }
            }
        }
    }

    #[test]
    fn a_ring_runs_whole_tiles_of_u_and_records_its_radius() {
        let mut mesh = MeshData::default();
        let uv = RingUv { v: 2.5, u_tiles: 3.0 };
        emit_ring(&mut mesh, Vec3::ZERO, RingBasis::from_axis(Vec3::Y), 6, 0.4, uv, |i, _| 0.1 + i as f32 * 0.01);
        assert_eq!((mesh.uvs[0], mesh.uvs[6]), (Vec2::new(0.0, 2.5), Vec2::new(3.0, 2.5)));
        assert!(mesh.uvs.iter().all(|uv| uv.y == 2.5));
        // The second UV set holds each vertex's own radius.
        for (i, uv1) in mesh.uv1.iter().enumerate() {
            assert_eq!(*uv1, Vec2::new(0.1 + i as f32 * 0.01, 1.0));
        }
    }
}
