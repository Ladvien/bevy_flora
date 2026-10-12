//! Cutting a mesh across a horizontal plane: what stays on one side, and the faces that close
//! the cut (for a felled tree and its stump).

use std::collections::HashMap;

use bevy::math::{Vec2, Vec3, Vec4};

use super::MeshData;

/// One side of a mesh cut at a height: the triangles on that side (those crossing the plane
/// clipped to it) and the cut faces that close every hole the plane opened.
#[derive(Clone, Debug, Default)]
pub struct CutMesh {
    /// The part of the mesh on the kept side.
    pub kept: MeshData,
    /// The cut faces, one fan per closed outline: flat, facing away from the kept side, with
    /// texture coordinates that put each outline's centre at (0.5, 0.5) and its farthest point
    /// on a circle of radius 0.5 (an end-grain picture maps onto it as rings).
    pub faces: MeshData,
    /// The outlines of the cut, one closed loop of points per hole the plane opened.
    pub outlines: Vec<Vec<Vec3>>,
}

/// Cut `mesh` at height `y` and keep what lies above it (`keep_above`) or below. Vertices on
/// the plane count as kept. Every vertex attribute of `mesh` is interpolated along the edges the
/// plane crosses. For a closed mesh the result is closed: its cut faces fill the holes.
pub fn cut(mesh: &MeshData, y: f32, keep_above: bool) -> CutMesh {
    let side = if keep_above { 1.0 } else { -1.0 };
    let distance: Vec<f32> = mesh.positions.iter().map(|p| side * (p.y - y)).collect();
    let mut builder =
        Builder { mesh, kept: MeshData::default(), remap: vec![None; mesh.positions.len()], edges: HashMap::new() };
    // The cut edges of the kept surface, each as the cut face must run it: reversed.
    let mut segments: Vec<(u32, u32)> = Vec::new();
    for tri in mesh.indices.as_chunks::<3>().0 {
        let inside = [0, 1, 2].map(|k| distance[tri[k] as usize] >= 0.0);
        match inside.iter().filter(|&&i| i).count() {
            3 => {
                for &v in tri {
                    let v = builder.vertex(v);
                    builder.kept.indices.push(v);
                }
            }
            0 => {}
            1 => {
                // Rotate so the kept vertex comes first, keeping the winding.
                let r = inside.iter().position(|&i| i).expect("one vertex is inside");
                let (a, b, c) = (tri[r], tri[(r + 1) % 3], tri[(r + 2) % 3]);
                let ab = builder.edge(a, b, &distance);
                let ac = builder.edge(a, c, &distance);
                let a = builder.vertex(a);
                builder.kept.indices.extend([a, ab, ac]);
                segments.push((ac, ab));
            }
            _ => {
                // Rotate so the dropped vertex comes first.
                let r = inside.iter().position(|&i| !i).expect("one vertex is outside");
                let (a, b, c) = (tri[r], tri[(r + 1) % 3], tri[(r + 2) % 3]);
                let ab = builder.edge(a, b, &distance);
                let ca = builder.edge(c, a, &distance);
                let (b, c) = (builder.vertex(b), builder.vertex(c));
                builder.kept.indices.extend([ab, b, c, ab, c, ca]);
                segments.push((ab, ca));
            }
        }
    }
    let kept = builder.kept;
    let outlines = outermost(chain(&kept.positions, &segments));
    let faces = fill(&outlines, if keep_above { Vec3::NEG_Y } else { Vec3::Y });
    CutMesh { kept, faces, outlines }
}

/// The outlines that lie inside no other. A branch's root buried in the trunk crosses the plane
/// inside the trunk's outline; a face over it would fight the trunk's for the same pixels.
fn outermost(outlines: Vec<Vec<Vec3>>) -> Vec<Vec<Vec3>> {
    // Twice the signed area of the outline in the ground plane, made positive.
    let area = |o: &[Vec3]| {
        let twice: f32 = (0..o.len()).map(|i| o[i].x * o[(i + 1) % o.len()].z - o[(i + 1) % o.len()].x * o[i].z).sum();
        twice.abs()
    };
    let inside = |p: Vec3, o: &[Vec3]| {
        let mut crossings = false;
        for i in 0..o.len() {
            let (a, b) = (o[i], o[(i + 1) % o.len()]);
            if (a.z > p.z) != (b.z > p.z) && p.x < a.x + (p.z - a.z) / (b.z - a.z) * (b.x - a.x) {
                crossings = !crossings;
            }
        }
        crossings
    };
    let areas: Vec<f32> = outlines.iter().map(|o| area(o)).collect();
    let centres: Vec<Vec3> = outlines.iter().map(|o| o.iter().copied().sum::<Vec3>() / o.len() as f32).collect();
    (0..outlines.len())
        .filter(|&i| !(0..outlines.len()).any(|j| j != i && areas[j] > areas[i] && inside(centres[i], &outlines[j])))
        .map(|i| outlines[i].clone())
        .collect()
}

struct Builder<'a> {
    mesh: &'a MeshData,
    kept: MeshData,
    /// The kept copy of each original vertex.
    remap: Vec<Option<u32>>,
    /// The vertex made where the plane crosses each edge (lower index first).
    edges: HashMap<(u32, u32), u32>,
}

impl Builder<'_> {
    fn vertex(&mut self, v: u32) -> u32 {
        if let Some(kept) = self.remap[v as usize] {
            return kept;
        }
        let kept = self.push(v, v, 0.0);
        self.remap[v as usize] = Some(kept);
        kept
    }

    /// The vertex where the plane crosses the edge from the kept `a` to the dropped `b` (or the
    /// other way round), shared by both triangles of the edge.
    fn edge(&mut self, a: u32, b: u32, distance: &[f32]) -> u32 {
        let key = (a.min(b), a.max(b));
        if let Some(&v) = self.edges.get(&key) {
            return v;
        }
        let (da, db) = (distance[a as usize], distance[b as usize]);
        let t = if (da - db).abs() > f32::EPSILON { da / (da - db) } else { 0.0 };
        let v = self.push(a, b, t);
        self.edges.insert(key, v);
        v
    }

    /// Append the vertex `t` of the way from `a` to `b`.
    fn push(&mut self, a: u32, b: u32, t: f32) -> u32 {
        let (m, out, a, b) = (self.mesh, &mut self.kept, a as usize, b as usize);
        out.positions.push(m.positions[a].lerp(m.positions[b], t));
        out.normals.push(m.normals[a].lerp(m.normals[b], t).normalize_or(m.normals[a]));
        out.uvs.push(m.uvs[a].lerp(m.uvs[b], t));
        if !m.colors.is_empty() {
            out.colors.push(m.colors[a].lerp(m.colors[b], t));
        }
        if !m.uv1.is_empty() {
            out.uv1.push(m.uv1[a].lerp(m.uv1[b], t));
        }
        if !m.sway.is_empty() {
            out.sway.push(lerp4(m.sway[a], m.sway[b], t));
        }
        (out.positions.len() - 1) as u32
    }
}

fn lerp4(a: Vec4, b: Vec4, t: f32) -> Vec4 {
    a + (b - a) * t
}

/// Join the cut segments into closed loops. Seams duplicate vertices, so segments are joined by
/// where their ends are, not by index.
fn chain(positions: &[Vec3], segments: &[(u32, u32)]) -> Vec<Vec<Vec3>> {
    let key = |v: u32| {
        let p = positions[v as usize] * 1.0e4;
        (p.x.round() as i64, p.z.round() as i64)
    };
    let mut next: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, &(from, to)) in segments.iter().enumerate() {
        if key(from) != key(to) {
            next.entry(key(from)).or_default().push(i);
        }
    }
    let mut used = vec![false; segments.len()];
    let mut loops = Vec::new();
    for start in 0..segments.len() {
        if used[start] || key(segments[start].0) == key(segments[start].1) {
            continue;
        }
        let mut outline = Vec::new();
        let mut current = start;
        loop {
            used[current] = true;
            let (from, to) = segments[current];
            outline.push(positions[from as usize]);
            let Some(follow) = next.get(&key(to)).and_then(|options| options.iter().copied().find(|&i| !used[i]))
            else {
                break;
            };
            current = follow;
        }
        if outline.len() >= 3 {
            loops.push(outline);
        }
    }
    loops
}

/// A fan from the centre of each outline, facing `normal`.
fn fill(outlines: &[Vec<Vec3>], normal: Vec3) -> MeshData {
    let mut faces = MeshData::default();
    for outline in outlines {
        let centre = outline.iter().copied().sum::<Vec3>() / outline.len() as f32;
        let reach = outline.iter().map(|p| (*p - centre).length()).fold(1.0e-4, f32::max);
        let uv = |p: Vec3| Vec2::new(0.5, 0.5) + Vec2::new(p.x - centre.x, p.z - centre.z) / (2.0 * reach);
        let first = faces.positions.len() as u32;
        for p in std::iter::once(centre).chain(outline.iter().copied()) {
            faces.positions.push(p);
            faces.normals.push(normal);
            faces.uvs.push(uv(p));
        }
        let n = outline.len() as u32;
        for i in 0..n {
            faces.indices.extend([first, first + 1 + i, first + 1 + (i + 1) % n]);
        }
    }
    faces
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TreePreset, build_tree};

    /// Edges used by exactly one triangle, with coincident vertices treated as one.
    fn open_edges(mesh: &MeshData) -> usize {
        let key = |i: u32| (mesh.positions[i as usize] * 1.0e4).round().as_ivec3().to_array();
        let mut uses: HashMap<([i32; 3], [i32; 3]), u32> = HashMap::new();
        for tri in mesh.indices.as_chunks::<3>().0 {
            for (u, v) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                let (ku, kv) = (key(u), key(v));
                if ku != kv {
                    *uses.entry((ku.min(kv), ku.max(kv))).or_insert(0) += 1;
                }
            }
        }
        uses.values().filter(|&&n| n == 1).count()
    }

    /// The mean of the winding normals of the faces, which must agree with their stored normal.
    fn winding_normal(mesh: &MeshData) -> Vec3 {
        mesh.indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.positions[i as usize]);
                (b - a).cross(c - a)
            })
            .sum::<Vec3>()
    }

    #[test]
    fn a_cut_trunk_is_closed_on_both_sides_and_its_faces_face_out() {
        for preset in [TreePreset::Oak, TreePreset::Pine, TreePreset::Baobab, TreePreset::Birch] {
            let wood = build_tree(&preset.params(), f32::INFINITY).trunk;
            let before = open_edges(&wood);
            for (keep_above, outward) in [(true, Vec3::NEG_Y), (false, Vec3::Y)] {
                let cut = cut(&wood, 0.6, keep_above);
                assert!(!cut.outlines.is_empty(), "{preset:?}: the plane missed the trunk");
                let mut closed = cut.kept.clone();
                closed.extend(&cut.faces);
                assert!(
                    open_edges(&closed) <= before,
                    "{preset:?} (keep above: {keep_above}): the cut left {} open edges, the wood had {before}",
                    open_edges(&closed)
                );
                assert!(winding_normal(&cut.faces).dot(outward) > 0.0, "{preset:?}: the cut faces face inward");
                let wrong = cut.kept.positions.iter().filter(|p| (p.y - 0.6) * if keep_above { 1.0 } else { -1.0 } < -1e-4);
                assert_eq!(wrong.count(), 0, "{preset:?}: a vertex is on the wrong side of the cut");
                for uv in &cut.faces.uvs {
                    assert!((uv - Vec2::splat(0.5)).length() <= 0.5 + 1e-4, "{preset:?}: a cut face's uv leaves the disc");
                }
            }
        }
    }
}
