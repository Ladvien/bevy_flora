//! Cutting a mesh across a plane: what stays on one side, and the faces that close the cut (for
//! a felled tree and its stump, and an axe's notch).

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
    cut_plane(mesh, &Plane::horizontal(y), keep_above)
}

/// Cut `mesh` across `plane` and keep what lies above it (the side its normal points to,
/// `keep_above`) or below; see [`cut`]. The cut faces face away from the kept side.
pub fn cut_plane(mesh: &MeshData, plane: &Plane, keep_above: bool) -> CutMesh {
    let (kept, runs) = split(mesh, plane, keep_above);
    let outlines = outermost(runs.into_iter().map(|run| run.points).filter(|p| p.len() >= 3).collect(), plane);
    let side = if keep_above { 1.0 } else { -1.0 };
    let faces = fill(&outlines, plane.normal * -side, plane);
    CutMesh { kept, faces, outlines }
}

/// The part of `mesh` on one side of `plane`, as [`cut_plane`] keeps it, and the outlines of the
/// cut, each run the way a face on the cut (facing away from the kept side) runs round.
fn split(mesh: &MeshData, plane: &Plane, keep_above: bool) -> (MeshData, Vec<Run>) {
    let side = if keep_above { 1.0 } else { -1.0 };
    let distance: Vec<f32> = mesh.positions.iter().map(|p| side * plane.distance(*p)).collect();
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
    let runs = chain(&builder.kept.positions, &segments);
    (builder.kept, runs)
}

/// The outlines that lie inside no other. A branch's root buried in the trunk crosses the plane
/// inside the trunk's outline; a face over it would fight the trunk's for the same pixels.
fn outermost(outlines: Vec<Vec<Vec3>>, plane: &Plane) -> Vec<Vec<Vec3>> {
    let (u, v) = plane_axes(plane);
    let flat = |p: Vec3| Vec2::new(p.dot(u), p.dot(v));
    // Twice the signed area of the outline across the plane, made positive.
    let area = |o: &[Vec3]| {
        let twice: f32 = (0..o.len())
            .map(|i| {
                let (a, b) = (flat(o[i]), flat(o[(i + 1) % o.len()]));
                a.x * b.y - b.x * a.y
            })
            .sum();
        twice.abs()
    };
    let inside = |p: Vec3, o: &[Vec3]| {
        let p = flat(p);
        let mut crossings = false;
        for i in 0..o.len() {
            let (a, b) = (flat(o[i]), flat(o[(i + 1) % o.len()]));
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
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

/// One outline of a cut: its points in order, and whether it closes (from the last point back to
/// the first) or is an open run whose ends lie on the edge of an open mesh.
struct Run {
    points: Vec<Vec3>,
    closed: bool,
}

/// Join the cut segments into outlines. Seams duplicate vertices, so segments are joined by where
/// their ends are, not by index. A cut of an open mesh (a piece already cut below) can give open
/// runs whose ends lie on the earlier cut; they are followed from their first segment, so each
/// comes out whole.
fn chain(positions: &[Vec3], segments: &[(u32, u32)]) -> Vec<Run> {
    let key = |v: u32| (positions[v as usize] * 1.0e4).round().as_i64vec3().to_array();
    let mut next: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    let mut arrives: HashMap<[i64; 3], usize> = HashMap::new();
    for (i, &(from, to)) in segments.iter().enumerate() {
        if key(from) != key(to) {
            next.entry(key(from)).or_default().push(i);
            *arrives.entry(key(to)).or_default() += 1;
        }
    }
    let mut used = vec![false; segments.len()];
    let mut runs = Vec::new();
    // The heads of open runs first (nothing arrives where they start), then what is left: loops.
    let heads = (0..segments.len()).filter(|&i| !arrives.contains_key(&key(segments[i].0)));
    let order: Vec<usize> = heads.chain(0..segments.len()).collect();
    for start in order {
        if used[start] || key(segments[start].0) == key(segments[start].1) {
            continue;
        }
        let mut points = Vec::new();
        let mut current = start;
        let closed = loop {
            used[current] = true;
            let (from, to) = segments[current];
            points.push(positions[from as usize]);
            let Some(follow) = next.get(&key(to)).and_then(|options| options.iter().copied().find(|&i| !used[i]))
            else {
                // Back where it started, or an open run's end: keep its last point.
                let closed = key(to) == key(segments[start].0);
                if !closed {
                    points.push(positions[to as usize]);
                }
                break closed;
            };
            current = follow;
        };
        if points.len() >= 3 || (!closed && points.len() >= 2) {
            runs.push(Run { points, closed });
        }
    }
    runs
}

/// The pieces of the closed `outline` on one side of `plane` (above it with `keep_above`): the
/// whole outline if it lies there, else the runs of it there, each from where it crosses the
/// plane to where it crosses back.
fn beyond(outline: &[Vec3], plane: &Plane, keep_above: bool) -> Vec<Run> {
    let side = if keep_above { 1.0 } else { -1.0 };
    let distance: Vec<f32> = outline.iter().map(|p| side * plane.distance(*p)).collect();
    let Some(outside) = distance.iter().position(|&d| d < 0.0) else {
        return vec![Run { points: outline.to_vec(), closed: true }];
    };
    // Walk once round from a point outside, starting a run where the outline crosses in and
    // ending it where it crosses out.
    let n = outline.len();
    let mut runs = Vec::new();
    let mut points: Vec<Vec3> = Vec::new();
    for k in 0..n {
        let (i, j) = ((outside + k) % n, (outside + k + 1) % n);
        let (a, b, da, db) = (outline[i], outline[j], distance[i], distance[j]);
        if (da >= 0.0) != (db >= 0.0) {
            let crossing = a.lerp(b, da / (da - db));
            points.push(crossing);
            if da >= 0.0 {
                runs.push(Run { points: std::mem::take(&mut points), closed: false });
            }
        }
        if db >= 0.0 {
            points.push(b);
        }
    }
    runs.into_iter().filter(|r| r.points.len() >= 2).collect()
}

/// Close `runs` whose ends all lie on one straight line (running along `along`) into outlines: the
/// region they bound covers every other stretch of the line between their ends, in order along
/// it, so each run's end joins the start of the run across that stretch. A simple polygon cut by a
/// line closes this way however often its outline crosses the line, and two regions on either
/// side of the line, or above and below it in two planes through it, close along the same
/// stretches. Closed runs pass through.
fn close_runs(runs: Vec<Run>, along: Vec3) -> Vec<Vec<Vec3>> {
    let mut ends: Vec<(f32, usize, bool)> = runs
        .iter()
        .enumerate()
        .filter(|(_, run)| !run.closed)
        .flat_map(|(i, run)| [(run.points[0].dot(along), i, true), (run.points[run.points.len() - 1].dot(along), i, false)])
        .collect();
    ends.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut next = vec![None; runs.len()];
    for &[a, b] in ends.as_chunks::<2>().0 {
        match (a, b) {
            ((_, end, false), (_, start, true)) | ((_, start, true), (_, end, false)) => next[end] = Some(start),
            // Two starts or two ends: the runs are not one region's; each closes on itself.
            _ => {}
        }
    }
    let mut used = vec![false; runs.len()];
    let mut outlines = Vec::new();
    for first in 0..runs.len() {
        if used[first] {
            continue;
        }
        let mut outline = Vec::new();
        let mut current = first;
        loop {
            used[current] = true;
            outline.extend_from_slice(&runs[current].points);
            match next[current] {
                Some(n) if !used[n] => current = n,
                _ => break,
            }
        }
        if outline.len() >= 3 {
            outlines.push(outline);
        }
    }
    outlines
}

/// Two axes across `plane`: the ground's x and z for a horizontal one (so a level cut's end grain
/// lies as it always has), any pair at right angles to the normal otherwise.
fn plane_axes(plane: &Plane) -> (Vec3, Vec3) {
    if plane.normal.y.abs() > 1.0 - 1.0e-6 {
        (Vec3::X, Vec3::Z)
    } else {
        plane.normal.any_orthonormal_pair()
    }
}

/// Each outline (lying in `plane`, running anticlockwise seen from the side `normal` points to)
/// filled, facing `normal`, with texture coordinates across `plane` putting its centre at
/// (0.5, 0.5) and its farthest point at radius 0.5: a fan from the centre where every triangle of
/// it faces `normal` (a trunk's cut), else the outline's own triangles by ear clipping (the floor
/// of a notch in a lobed trunk, its long face).
pub fn fill(outlines: &[Vec<Vec3>], normal: Vec3, plane: &Plane) -> MeshData {
    let (u, v) = plane_axes(plane);
    let mut faces = MeshData::default();
    for outline in outlines {
        let centre = outline.iter().copied().sum::<Vec3>() / outline.len() as f32;
        let reach = outline.iter().map(|p| (*p - centre).length()).fold(1.0e-4, f32::max);
        let uv = |p: Vec3| Vec2::new(0.5, 0.5) + Vec2::new((p - centre).dot(u), (p - centre).dot(v)) / (2.0 * reach);
        let n = outline.len();
        let fans = (0..n).all(|i| (outline[i] - centre).cross(outline[(i + 1) % n] - centre).dot(normal) >= 0.0);
        let first = faces.positions.len() as u32;
        if fans {
            for p in std::iter::once(centre).chain(outline.iter().copied()) {
                faces.positions.push(p);
                faces.normals.push(normal);
                faces.uvs.push(uv(p));
            }
            for i in 0..n as u32 {
                faces.indices.extend([first, first + 1 + i, first + 1 + (i + 1) % n as u32]);
            }
        } else {
            for &p in outline {
                faces.positions.push(p);
                faces.normals.push(normal);
                faces.uvs.push(uv(p));
            }
            for [a, b, c] in ear_clip(outline, normal) {
                faces.indices.extend([first + a as u32, first + b as u32, first + c as u32]);
            }
        }
    }
    faces
}

/// The triangles of the simple polygon `outline` (each a triple of its indices), wound to face
/// `normal` as the outline runs: ear clipping, the corner whose triangle faces `normal` and holds
/// no other corner cut off one at a time.
fn ear_clip(outline: &[Vec3], normal: Vec3) -> Vec<[usize; 3]> {
    let facing = |a: Vec3, b: Vec3, c: Vec3| (b - a).cross(c - a).dot(normal);
    let holds = |a: Vec3, b: Vec3, c: Vec3, p: Vec3| {
        facing(a, b, p) > 0.0 && facing(b, c, p) > 0.0 && facing(c, a, p) > 0.0
    };
    let mut left: Vec<usize> = (0..outline.len()).collect();
    let mut triangles = Vec::new();
    while left.len() > 3 {
        let count = left.len();
        let ear = (0..count).find(|&i| {
            let (a, b, c) = (left[(i + count - 1) % count], left[i], left[(i + 1) % count]);
            let (pa, pb, pc) = (outline[a], outline[b], outline[c]);
            facing(pa, pb, pc) > 0.0
                && !left.iter().any(|&k| k != a && k != b && k != c && holds(pa, pb, pc, outline[k]))
        });
        // A polygon that has no ear left is degenerate (collinear corners): cut the corner anyway.
        let i = ear.unwrap_or(0);
        triangles.push([left[(i + count - 1) % count], left[i], left[(i + 1) % count]]);
        left.remove(i);
    }
    if left.len() == 3 {
        triangles.push([left[0], left[1], left[2]]);
    }
    triangles
}

/// The part of the closed `outline` on the side of `plane` its normal points to (`keep_above`) or
/// the other: the polygon clipped to the half-space (Sutherland and Hodgman), or nothing.
pub fn clip_outline(outline: &[Vec3], plane: &Plane, keep_above: bool) -> Vec<Vec3> {
    let side = if keep_above { 1.0 } else { -1.0 };
    let mut kept = Vec::new();
    for i in 0..outline.len() {
        let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
        let (da, db) = (side * plane.distance(a), side * plane.distance(b));
        if da >= 0.0 {
            kept.push(a);
        }
        if (da >= 0.0) != (db >= 0.0) {
            kept.push(a.lerp(b, da / (da - db)));
        }
    }
    if kept.len() < 3 { Vec::new() } else { kept }
}

/// Where an axe's notch is cut into a trunk, in the trunk's own space. Distances along `toward`
/// are measured from `axis`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Notch {
    /// The height of its level floor.
    pub floor: f32,
    /// The height above which nothing is carved: above the top of the face where it leaves the
    /// trunk.
    pub mouth: f32,
    /// The trunk's middle at the floor, x and z.
    pub axis: Vec2,
    /// The side it is cut from, x and z, unit length.
    pub toward: Vec2,
    /// How far its back (where the face meets the floor) is from `axis` toward `toward`;
    /// negative past the middle.
    pub inner: f32,
    /// How far from `axis` toward `toward` the carving stops: past the trunk, so a limb hanging
    /// beyond it is left whole.
    pub outer: f32,
    /// The trunk's radius at the floor: the end grain's rings on the notch's faces are laid
    /// about `axis` out to it.
    pub radius: f32,
    /// How steeply the face rises from the back, out of the trunk (rise over run).
    pub slope: f32,
}

impl Notch {
    /// The plane of the face: below it, and above the floor, is the wood the notch takes out.
    pub fn face(&self) -> Plane {
        // y - slope * s = floor - slope * inner, s = (p.xz - axis) . toward.
        let n = Vec3::new(-self.slope * self.toward.x, 1.0, -self.slope * self.toward.y);
        let offset = self.floor - self.slope * (self.inner + self.axis.dot(self.toward));
        let length = n.length();
        Plane { normal: n / length, offset: offset / length }
    }

    /// The plane of the back, standing at the line where the face meets the floor; the notch
    /// lies above it (on its far side).
    pub fn back(&self) -> Plane {
        self.across(self.inner)
    }

    /// The plane standing `distance` from `axis` toward `toward`, facing that way.
    fn across(&self, distance: f32) -> Plane {
        Plane { normal: Vec3::new(self.toward.x, 0.0, self.toward.y), offset: distance + self.axis.dot(self.toward) }
    }

    /// Texture coordinates for the faces of a cut through the trunk, so the end grain's rings
    /// centre on its middle (the pith) as on a level cut's face: from where each point stands
    /// over the ground, `radius` from `axis` at radius 0.5. On the slanted face the rings stretch
    /// as a slanted saw cut's do.
    fn lay_grain(&self, faces: &mut MeshData) {
        for (p, uv) in faces.positions.iter().zip(&mut faces.uvs) {
            *uv = Vec2::splat(0.5) + (Vec2::new(p.x, p.z) - self.axis) / (2.0 * self.radius.max(1.0e-4));
        }
    }

    /// The direction of the back's line, level and square to `toward`.
    fn along(&self) -> Vec3 {
        Vec3::new(-self.toward.y, 0.0, self.toward.x)
    }
}

/// `mesh` with a notch cut out of its trunk: the trunk's wood above the floor, below the mouth,
/// short of `outer` and below the face goes, and two faces close the cut, the floor (facing up)
/// and the face (facing out and down). Only the trunk is carved, the connected piece of `mesh`
/// (its vertices welded where they meet) that crosses the floor by `axis` with the most
/// triangles; a limb, a separate piece with its root buried in the trunk, is left whole however
/// close it passes. The pieces of the trunk meet where they did, so a closed trunk stays closed.
/// Returns the carved mesh and its faces.
pub fn carve_notch(mesh: &MeshData, notch: &Notch) -> (MeshData, MeshData) {
    let (trunk, rest) = trunk_piece(mesh, notch);
    let floor = Plane::horizontal(notch.floor);
    let below = cut_plane(&trunk, &floor, false);
    let (carved, face) = wedge(&cut_plane(&trunk, &floor, true).kept, notch);
    // The wood below the floor, split along the back's plane (both sides kept): the face cut the
    // edges of the floor's ring where the back crosses them, and the wood below must meet the
    // pieces there, not span them.
    let mut kept = cut_plane(&below.kept, &notch.back(), true).kept;
    kept.extend(&cut_plane(&below.kept, &notch.back(), false).kept);
    kept.extend(&carved);
    kept.extend(&rest);
    // The floor: the trunk's cross-section past the back, short of `outer`, closed along the back.
    let runs = below
        .outlines
        .iter()
        .map(|o| clip_outline(o, &notch.across(notch.outer), false))
        .filter(|o| !o.is_empty())
        .flat_map(|o| beyond(&o, &notch.back(), true))
        .collect();
    let mut faces = fill(&close_runs(runs, notch.along()), Vec3::Y, &floor);
    faces.extend(&face);
    notch.lay_grain(&mut faces);
    (kept, faces)
}

/// What falls when a notched trunk is cut level at the notch's floor: the wood above the cut
/// (`above`, from [`cut`] at `notch.floor`, kept above) with the notch's wedge taken out of its
/// trunk (see [`carve_notch`]), and its faces: the notch's face and the cut behind the notch's
/// back (facing down), and the whole cut of anything else the saw went through. The notch's
/// floor stays on the stump.
pub fn fell_notched(above: &CutMesh, notch: &Notch) -> (MeshData, MeshData) {
    let (trunk, rest) = trunk_piece(&above.kept, notch);
    let (mut kept, mut faces) = wedge(&trunk, notch);
    kept.extend(&rest);
    // The trunk's outline is the cut's largest.
    let largest = (0..above.outlines.len()).max_by_key(|&i| above.outlines[i].len());
    let runs = largest.into_iter().flat_map(|i| beyond(&above.outlines[i], &notch.back(), false)).collect();
    let floor = Plane::horizontal(notch.floor);
    faces.extend(&fill(&close_runs(runs, notch.along()), Vec3::NEG_Y, &floor));
    notch.lay_grain(&mut faces);
    let others: Vec<Vec<Vec3>> =
        (0..above.outlines.len()).filter(|&i| Some(i) != largest).map(|i| above.outlines[i].clone()).collect();
    faces.extend(&fill(&others, Vec3::NEG_Y, &floor));
    (kept, faces)
}

/// `mesh` parted into its trunk, as [`carve_notch`] finds it, and everything else; all of it is
/// the rest when nothing crosses the floor by the axis.
fn trunk_piece(mesh: &MeshData, notch: &Notch) -> (MeshData, MeshData) {
    // Pieces: vertices joined by the triangles' edges and where they lie on each other.
    let n = mesh.positions.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut v: usize) -> usize {
        while parent[v] != v {
            parent[v] = parent[parent[v]];
            v = parent[v];
        }
        v
    }
    let join = |parent: &mut Vec<usize>, a: usize, b: usize| {
        let (a, b) = (root(parent, a), root(parent, b));
        if a != b {
            parent[a.max(b)] = a.min(b);
        }
    };
    let mut welded: HashMap<[i64; 3], usize> = HashMap::new();
    for (v, p) in mesh.positions.iter().enumerate() {
        let key = (*p * 1.0e4).round().as_i64vec3().to_array();
        let first = *welded.entry(key).or_insert(v);
        join(&mut parent, first, v);
    }
    for tri in mesh.indices.as_chunks::<3>().0 {
        join(&mut parent, tri[0] as usize, tri[1] as usize);
        join(&mut parent, tri[0] as usize, tri[2] as usize);
    }
    // The trunk: of the pieces whose cross-section at the floor surrounds the axis (crosses the
    // floor on all four sides of it within reach; a limb hanging close by crosses on one side),
    // the one with the most triangles.
    let reach = notch.radius * 1.5 + 0.05;
    let mut sides: HashMap<usize, (u8, usize)> = HashMap::new();
    for tri in mesh.indices.as_chunks::<3>().0 {
        let piece = sides.entry(root(&mut parent, tri[0] as usize)).or_default();
        piece.1 += 1;
        for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let (a, b) = (mesh.positions[a as usize], mesh.positions[b as usize]);
            let (da, db) = (a.y - notch.floor, b.y - notch.floor);
            if da * db > 0.0 || da == db {
                continue;
            }
            let crossing = a.lerp(b, da / (da - db));
            let offset = Vec2::new(crossing.x, crossing.z) - notch.axis;
            if offset.length() <= reach {
                piece.0 |= 1 << (usize::from(offset.x >= 0.0) + 2 * usize::from(offset.y >= 0.0));
            }
        }
    }
    let trunk = sides
        .into_iter()
        .filter(|&(_, (quadrants, _))| quadrants == 0b1111)
        .max_by_key(|&(_, (_, triangles))| triangles)
        .map(|(r, _)| r);
    let mut pieces = [
        Builder { mesh, kept: MeshData::default(), remap: vec![None; n], edges: HashMap::new() },
        Builder { mesh, kept: MeshData::default(), remap: vec![None; n], edges: HashMap::new() },
    ];
    for tri in mesh.indices.as_chunks::<3>().0 {
        let builder = &mut pieces[usize::from(Some(root(&mut parent, tri[0] as usize)) != trunk)];
        for &v in tri {
            let v = builder.vertex(v);
            builder.kept.indices.push(v);
        }
    }
    let [trunk, rest] = pieces;
    (trunk.kept, rest.kept)
}

/// The wood above a notch's floor (`above`, cut level there) with the notch's wedge taken out,
/// and the notch's face, closed along its back.
fn wedge(above: &MeshData, notch: &Notch) -> (MeshData, MeshData) {
    let mouth = Plane::horizontal(notch.mouth);
    let outer = notch.across(notch.outer);
    let band = cut_plane(above, &mouth, false);
    let near = cut_plane(&band.kept, &outer, false);
    let face = notch.face();
    let (mut kept, runs) = split(&near.kept, &face, true);
    let outlines = outermost(close_runs(runs, notch.along()), &face);
    kept.extend(&cut_plane(&band.kept, &outer, true).kept);
    kept.extend(&cut_plane(above, &mouth, true).kept);
    (kept, fill(&outlines, -face.normal, &face))
}

/// A plane: the points `p` with `normal · p = offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    /// Unit length; it points to the side called above.
    pub normal: Vec3,
    /// Its distance from the origin along `normal`.
    pub offset: f32,
}

impl Plane {
    /// The level plane at height `y`.
    pub fn horizontal(y: f32) -> Self {
        Self { normal: Vec3::Y, offset: y }
    }

    /// How far `p` lies above the plane (negative below).
    pub fn distance(&self, p: Vec3) -> f32 {
        self.normal.dot(p) - self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TreePreset, build_tree};
    use bevy::math::Vec3Swizzles;

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

    /// A notch in `wood` from +x, its floor at `floor`, `depth` of the way through the trunk, as
    /// the plugin cuts one.
    fn notch_at(wood: &MeshData, floor: f32, depth: f32) -> Notch {
        let level = cut(wood, floor, false);
        let centre_of = |o: &[Vec3]| o.iter().copied().sum::<Vec3>() / o.len() as f32;
        let outline = level.outlines.iter().min_by(|a, b| centre_of(a).xz().length().total_cmp(&centre_of(b).xz().length()));
        let outline = outline.expect("the trunk is cut");
        let centre = centre_of(outline);
        let front = outline.iter().map(|p| p.x - centre.x).fold(0.0, f32::max);
        let back = outline.iter().map(|p| centre.x - p.x).fold(0.0, f32::max);
        let inner = front - depth * (front + back);
        let rise = front - inner;
        Notch {
            floor,
            mouth: floor + 1.5 * rise + 0.05,
            axis: centre.xz(),
            toward: Vec2::X,
            inner,
            outer: front + 0.5 * rise + 0.05,
            radius: outline.iter().map(|p| p.distance(centre)).fold(0.0, f32::max),
            slope: 1.0,
        }
    }

    #[test]
    fn a_notch_takes_its_wedge_out_of_the_trunk_and_closes_it() {
        for preset in [TreePreset::Oak, TreePreset::Pine, TreePreset::Baobab, TreePreset::Birch] {
            let wood = build_tree(&preset.params(), f32::INFINITY).trunk;
            let before = open_edges(&wood);
            let notch = notch_at(&wood, 0.5, 0.4);
            let (kept, faces) = carve_notch(&wood, &notch);
            let mut closed = kept.clone();
            closed.extend(&faces);
            assert!(open_edges(&closed) <= before, "{preset:?}: the notch left {} open edges, the wood had {before}", open_edges(&closed));
            // No wood is left inside the wedge.
            let face = notch.face();
            let inside = kept.positions.iter().filter(|p| p.y > 0.5 + 1e-3 && p.y < notch.mouth - 1e-3 && face.distance(**p) < -1e-3);
            assert_eq!(inside.count(), 0, "{preset:?}: wood left in the notch");
            // Its two faces: some facing up (the floor), some facing out of the trunk and down (the
            // face), each wound the way it faces.
            let mut up = 0;
            let mut out = 0;
            for t in faces.indices.as_chunks::<3>().0 {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| faces.positions[i as usize]);
                let winding = (b - a).cross(c - a);
                let normal = faces.normals[t[0] as usize];
                assert!(winding.length() < 1e-9 || winding.dot(normal) > 0.0, "{preset:?}: a notch face is wound inward");
                if normal.y > 0.99 {
                    up += 1;
                } else if normal.x > 0.5 && normal.y < -0.5 {
                    out += 1;
                }
            }
            assert!(up > 0 && out > 0, "{preset:?}: the notch has {up} floor and {out} face triangles");
            // The end grain's rings centre on the trunk's middle and stay on the image.
            for uv in &faces.uvs {
                assert!((uv - Vec2::splat(0.5)).length() <= 0.5 + 1e-3, "{preset:?}: a notch face's uv {uv} leaves the disc");
            }
        }
    }

    #[test]
    fn a_notch_cuts_the_trunk_and_leaves_its_limbs_whole() {
        // Low limbs by a deep notch a metre up: a spruce's lowest whorl, a willow's hanging twigs.
        for preset in [TreePreset::Spruce, TreePreset::Willow] {
            let wood = build_tree(&preset.params(), f32::INFINITY).trunk;
            let before = open_edges(&wood);
            let notch = notch_at(&wood, 1.0, 0.5);
            let (trunk, limbs) = trunk_piece(&wood, &notch);
            assert!(!trunk.indices.is_empty() && !limbs.indices.is_empty(), "{preset:?}: no trunk or no limbs");
            // The trunk is the piece round the base, not a limb.
            let base = trunk.positions.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
            assert!(base < 0.01, "{preset:?}: the piece taken for the trunk starts {base} m up");
            let (kept, faces) = carve_notch(&wood, &notch);
            let mut closed = kept.clone();
            closed.extend(&faces);
            assert!(open_edges(&closed) <= before, "{preset:?}: the notch left {} open edges, the wood had {before}", open_edges(&closed));
            let key = |p: &Vec3| (*p * 1.0e4).round().as_ivec3();
            let carved: std::collections::HashSet<_> = kept.positions.iter().map(key).collect();
            let cut_away = limbs.positions.iter().filter(|p| !carved.contains(&key(p)));
            assert_eq!(cut_away.count(), 0, "{preset:?}: the notch cut into a limb");
        }
    }
}
