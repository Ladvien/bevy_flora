use std::collections::HashMap;

use bevy::math::Vec3;

use super::frame::{Class, Tag, position, wood_radius};
use super::mesh::{GrowthData, build_growth_data};
use super::state::{CONE, GrowthState, TOP_CAP};
use crate::foliage::{foliage_meshes, leaf_points_for_node, leaf_ranks};
use crate::mesh::branch::{WoodPiece, WoodSpan, skeleton_mesh_with};
use crate::mesh::trunk::{trunk_pieces, trunk_ring_count};
use crate::params::{FoliagePlacement, LeafOrientation, TreeParams, TrunkTermination};
use crate::pipeline::aged_skeleton;
use crate::presets::TreePreset;
use crate::skeleton::{Skeleton, generate_skeleton};

/// Metres a grown vertex may differ from the CPU mesher's.
const TOLERANCE: f32 = 2e-4;

/// Tiles a texture coordinate may differ by between the aged mesh and the mature one.
const UV_TOLERANCE: f32 = 1e-3;

const AGES: [f32; 7] = [0.3, 1.0, 2.5, 4.0, 6.0, 9.0, f32::INFINITY];

/// A preset with no smoothing and fixed side counts: the case where the shader's
/// formulas are exact.
fn exact(preset: TreePreset) -> TreeParams {
    let mut params = preset.params();
    params.mesh.smooth_iterations = 0;
    params.mesh.adaptive_resolution = false;
    params
}

fn key(piece: WoodPiece) -> u8 {
    match piece {
        WoodPiece::Trunk => 0,
        WoodPiece::Tube => 1,
        WoodPiece::Collar => 2,
        WoodPiece::Root { collar: false } => 3,
        WoodPiece::Root { collar: true } => 4,
        WoodPiece::Dome { collar: false } => 5,
        WoodPiece::Dome { collar: true } => 6,
    }
}

fn is_base(piece: WoodPiece) -> bool {
    matches!(piece, WoodPiece::Root { .. } | WoodPiece::Dome { .. })
}

#[test]
fn growth_tags_put_every_mature_vertex_back_where_it_was() {
    for preset in TreePreset::ALL {
        for smooth in [0, 2] {
            let mut params = preset.params();
            params.mesh.smooth_iterations = smooth;
            let full = generate_skeleton(&params);
            let data = build_growth_data(&full, &params);
            let tagged = [
                ("wood", &data.wood, &data.wood_tags),
                ("leaves", &data.leaves, &data.leaf_tags),
                ("flowers", &data.flowers, &data.flower_tags),
            ];
            for (what, mesh, tags) in tagged {
                assert_eq!(mesh.positions.len(), tags.len());
                for (i, (&p, tag)) in mesh.positions.iter().zip(tags).enumerate() {
                    // The trunk's top cap and tip cone are in the mesh for every
                    // age; the mature tree has only one of them.
                    let hidden = match tag.class {
                        Class::TrunkTopCap => data.state.trunk_flags & TOP_CAP == 0,
                        Class::TrunkCone => data.state.trunk_flags & CONE == 0,
                        _ => false,
                    };
                    if hidden {
                        continue;
                    }
                    let q = position(tag, &data.state);
                    assert!(
                        q.distance(p) < TOLERANCE,
                        "{} smooth {smooth} {what} vertex {i} {tag:?}: {q:?} vs {p:?}",
                        preset.name()
                    );
                }
            }
        }
    }
}

#[test]
fn ageing_never_turns_or_bends_wood() {
    // The shader keeps every ring frame and axis shape from the mature mesh;
    // that holds because only start, length and radii change with age.
    for preset in TreePreset::ALL {
        let params = preset.params();
        let full = generate_skeleton(&params);
        for age in AGES {
            for node in &aged_skeleton(&full, &params, age).nodes {
                let original = &full.nodes[node.id as usize];
                assert_eq!(node.direction, original.direction, "{} age {age}", preset.name());
                assert_eq!(node.bend, original.bend, "{} age {age}", preset.name());
            }
        }
    }
}

/// What [`assert_same_bark`] compares.
#[derive(Clone, Copy)]
struct BarkChecks {
    /// The texture coordinates.
    uv: bool,
    /// The radius the bark fades in with.
    radius: bool,
}

/// A vertex of the aged CPU wood (`cpu`, vertex `v`) and the same vertex of the mature mesh GPU
/// growth shrinks (`gpu_vertex` of `gpu`, whose tags are `tags`) wear the same bark: the same
/// texture coordinates, so bark never slides as a tree ages and both paths draw the same wood,
/// and the radius the bark fades in with: the CPU mesh's `uv1.x` is what the vertex shader
/// computes from the growth state (`wood_radius`).
#[allow(clippy::too_many_arguments)]
fn assert_same_bark(
    what: &str,
    cpu: &crate::mesh::MeshData,
    v: usize,
    gpu: &crate::mesh::MeshData,
    tags: &[super::frame::Tag],
    gpu_vertex: usize,
    state: &GrowthState,
    checks: BarkChecks,
) {
    if checks.uv {
        let (a, b) = (cpu.uvs[v], gpu.uvs[gpu_vertex]);
        assert!((a - b).abs().max_element() < UV_TOLERANCE, "{what}: uv {a:?} (aged) vs {b:?} (mature)");
    }
    if checks.radius {
        let (r, s) = (cpu.uv1[v].x, wood_radius(&tags[gpu_vertex], state));
        assert!((r - s).abs() < TOLERANCE, "{what}: radius {r} (built) vs {s} (grown)");
    }
}

/// Vertices of `span` in a wood mesh.
fn span_range(span: &WoodSpan) -> std::ops::Range<usize> {
    span.first as usize..(span.first + span.count) as usize
}

/// Compare the grown wood against the CPU mesher at `age`; returns the number
/// of primary bases whose root-or-dome kind differs from the mature one.
fn check_wood(name: &str, params: &TreeParams, full: &Skeleton, data: &GrowthData, age: f32) -> usize {
    let aged = aged_skeleton(full, params, age);
    let mut cpu_spans = Vec::new();
    let pieces = trunk_pieces(&params.trunk, &aged.nodes[0]);
    let cpu = skeleton_mesh_with(&aged, params, pieces, Some(&mut cpu_spans));
    let mut state = GrowthState::default();
    state.update(full, params, age);

    let gpu: HashMap<(u32, u8), &WoodSpan> = data
        .wood_spans
        .iter()
        .map(|s| ((s.node as u32, key(s.piece)), s))
        .collect();
    let at = |span: &WoodSpan, j: usize| position(&data.wood_tags[span.first as usize + j], &state);

    let mut seen = std::collections::HashSet::new();
    let mut kind_mismatches = 0;
    for span in &cpu_spans {
        let id = aged.nodes[span.node].id;
        let Some(&grown) = gpu.get(&(id, key(span.piece))) else {
            assert!(is_base(span.piece), "{name} age {age}: no grown {:?} for node {id}", span.piece);
            kind_mismatches += 1;
            continue;
        };
        seen.insert((id, key(span.piece)));
        if span.piece == WoodPiece::Trunk {
            check_trunk(name, params, age, &cpu, span, grown, &data.wood_tags, &state, &data.wood);
            continue;
        }
        assert_eq!(span.count, grown.count, "{name} age {age}: node {id} {:?}", span.piece);
        for (j, v) in span_range(span).enumerate() {
            let q = at(grown, j);
            assert!(
                q.distance(cpu.positions[v]) < TOLERANCE,
                "{name} age {age}: node {id} {:?} vertex {j}: grown {q:?}, built {:?}",
                span.piece,
                cpu.positions[v]
            );
            // A primary's rounded base is narrower than the radius the shader scales it by.
            let radius_matches = !matches!(span.piece, WoodPiece::Dome { .. });
            let what = format!("{name} age {age}: node {id} {:?} vertex {j}", span.piece);
            // Under the pipe model an aged tree's radii are not its mature ones scaled, which is
            // what the bark layout (tiles around, cap size) is measured from, so only the
            // radius itself must agree there.
            let checks = BarkChecks { uv: !params.mesh.pipe_radius_enabled, radius: radius_matches };
            assert_same_bark(&what, &cpu, v, &data.wood, &data.wood_tags, grown.first as usize + j, &state, checks);
        }
    }

    // Pieces the CPU did not build at this age must have collapsed to a point,
    // unless it built the other kind of base.
    for ((id, k), span) in &gpu {
        if seen.contains(&(*id, *k)) || span.piece == WoodPiece::Trunk {
            continue;
        }
        if is_base(span.piece) {
            continue;
        }
        let first = at(span, 0);
        for j in 0..span.count as usize {
            assert!(
                at(span, j).distance(first) < 1e-5,
                "{name} age {age}: node {id} {:?} should have collapsed",
                span.piece
            );
        }
    }
    kind_mismatches
}

#[allow(clippy::too_many_arguments)]
fn check_trunk(
    name: &str,
    params: &TreeParams,
    age: f32,
    cpu: &crate::mesh::MeshData,
    span: &WoodSpan,
    grown: &WoodSpan,
    tags: &[super::frame::Tag],
    state: &GrowthState,
    gpu_mesh: &crate::mesh::MeshData,
) {
    let trunk = &params.trunk;
    let ring_len = trunk.radial_segments.max(3) as usize + 1;
    let rings = trunk_ring_count(trunk) as usize;
    let cpu_has_cone = state.trunk_flags & super::state::CONE != 0;
    let cpu_has_top = state.trunk_flags & super::state::TOP_CAP != 0;
    let gpu_has_cone = trunk.termination == TrunkTermination::PointedTip;
    let at = |j: usize| position(&tags[grown.first as usize + j], state);
    let built = |j: usize| cpu.positions[span.first as usize + j];

    // Rings are laid out the same.
    for j in 0..rings * ring_len {
        assert!(at(j).distance(built(j)) < TOLERANCE, "{name} age {age}: trunk ring vertex {j}");
        let v = span.first as usize + j;
        let checks = BarkChecks { uv: true, radius: true };
        let what = format!("{name} age {age}: trunk ring vertex {j}");
        assert_same_bark(&what, cpu, v, gpu_mesh, tags, grown.first as usize + j, state, checks);
    }
    let (mut cpu_at, mut gpu_at) = (rings * ring_len, rings * ring_len);
    if cpu_has_cone {
        for j in 0..ring_len {
            assert!(at(gpu_at + j).distance(built(cpu_at + j)) < TOLERANCE, "{name} age {age}: cone vertex {j}");
        }
        cpu_at += ring_len;
    } else if gpu_has_cone {
        // Hidden: the apex ring lies on the top ring.
        for j in 0..ring_len {
            let top = at((rings - 1) * ring_len + j);
            assert!(at(gpu_at + j).distance(top) < 1e-5, "{name} age {age}: hidden cone vertex {j}");
        }
    }
    if gpu_has_cone {
        gpu_at += ring_len;
    }
    // Bottom cap.
    for j in 0..1 + ring_len {
        assert!(
            at(gpu_at + j).distance(built(cpu_at + j)) < TOLERANCE,
            "{name} age {age}: bottom cap vertex {j}"
        );
        let v = span.first as usize + cpu_at + j;
        // A cap's centre has no radius of its own: the mesh gives it the rim's at one angle, the
        // shader the ring's without the root bulge.
        let what = format!("{name} age {age}: bottom cap vertex {j}");
        let checks = BarkChecks { uv: true, radius: j > 0 };
        assert_same_bark(&what, cpu, v, gpu_mesh, tags, grown.first as usize + gpu_at + j, state, checks);
    }
    cpu_at += 1 + ring_len;
    gpu_at += 1 + ring_len;
    // Top cap.
    if cpu_has_top {
        for j in 0..1 + ring_len {
            assert!(
                at(gpu_at + j).distance(built(cpu_at + j)) < TOLERANCE,
                "{name} age {age}: top cap vertex {j}"
            );
        }
        cpu_at += 1 + ring_len;
    } else {
        let first = at(gpu_at);
        for j in 0..1 + ring_len {
            assert!(at(gpu_at + j).distance(first) < 1e-5, "{name} age {age}: hidden top cap vertex {j}");
        }
    }
    gpu_at += 1 + ring_len;
    assert_eq!(cpu_at as u32, span.count, "{name} age {age}: CPU trunk layout");
    assert_eq!(gpu_at as u32, grown.count, "{name} age {age}: GPU trunk layout");
    let _ = gpu_mesh;
}

#[test]
fn grown_wood_matches_the_cpu_meshers() {
    let mut mismatches = 0;
    let mut bases = 0;
    for preset in TreePreset::ALL {
        for pipe in [false, true] {
            let mut params = exact(preset);
            params.mesh.pipe_radius_enabled = pipe;
            let full = generate_skeleton(&params);
            let data = build_growth_data(&full, &params);
            bases += full.nodes.iter().filter(|n| n.kind == crate::skeleton::NodeKind::Primary).count() * AGES.len();
            for age in AGES {
                mismatches += check_wood(&format!("{} pipe {pipe}", preset.name()), &params, &full, &data, age);
            }
        }
    }
    // A primary's base is a root or a dome by how it leaves the trunk, which
    // the grown tree decides once, at maturity. On the presets that decision
    // is the same at every age.
    assert_eq!(mismatches, 0, "{mismatches} of {bases} primary bases differ in kind");
}

#[test]
fn grown_wood_matches_at_every_age() {
    // A fine sweep finds the ages where a piece appears, vanishes or changes
    // kind, which the coarse ages above can step over.
    for preset in [TreePreset::Oak, TreePreset::Willow, TreePreset::Spruce, TreePreset::Palm] {
        let params = exact(preset);
        let full = generate_skeleton(&params);
        let data = build_growth_data(&full, &params);
        let mature_at = full.mature_at(&params.growth);
        let mut age = 0.02;
        while age < mature_at + 0.5 {
            let kinds = check_wood(preset.name(), &params, &full, &data, age);
            assert_eq!(kinds, 0, "{} age {age}", preset.name());
            age += 0.11;
        }
    }
}

#[test]
fn all_branches_leaves_appear_as_the_branch_lengthens() {
    let mut params = TreePreset::Oak.params();
    params.foliage.placement = FoliagePlacement::AllBranches;
    params.mesh.smooth_iterations = 0;
    let full = generate_skeleton(&params);
    let data = build_growth_data(&full, &params);
    // leaves the grown tree shows, per node: those whose first vertex has a size
    let mut ordinals: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut seen = std::collections::HashSet::new();
    for tag in &data.leaf_tags {
        if seen.insert((tag.node, tag.ordinal)) {
            ordinals.entry(tag.node).or_default().push(tag.ordinal);
        }
    }
    let mut checked = 0;
    for age in [1.5, 3.0, 4.5, 6.0, 8.0, f32::INFINITY] {
        let aged = aged_skeleton(&full, &params, age);
        let mut state = GrowthState::default();
        state.update(&full, &params, age);
        for (i, node) in aged.nodes.iter().enumerate() {
            let mut points = Vec::new();
            leaf_points_for_node(&aged, i, &params.foliage, &params.growth, &params.trunk, params.seed, &mut points);
            let shown = ordinals.get(&node.id).map_or(0, |list| {
                list.iter()
                    .filter(|&&o| {
                        let tag = data.leaf_tags.iter().find(|t| t.node == node.id && t.ordinal == o).unwrap();
                        super::frame::placement(tag, &state).scale > 0.0
                    })
                    .count()
            });
            assert_eq!(shown, points.len(), "age {age}: node {}", node.id);
            checked += points.len();
        }
    }
    assert!(checked > 500, "checked only {checked} leaves");
}

#[test]
fn grown_leaves_and_flowers_match_the_cpu_meshers() {
    let mut compared = 0;
    let mut flowers = 0;
    let mut mirrored = 0;
    for preset in TreePreset::ALL {
        let mut params = preset.params();
        params.mesh.smooth_iterations = 0;
        if !params.foliage.enabled {
            continue;
        }
        // Leaves that face out from their branch's tip face out from the tip it has now on the CPU
        // and from the mature tip on the GPU (a documented difference, and one of orientation, not
        // of place): every `AllBranches` preset is compared with its leaves along their branches.
        if params.foliage.placement == FoliagePlacement::AllBranches && params.foliage.orientation == LeafOrientation::RadialOutward {
            params.foliage.orientation = LeafOrientation::FollowBranch;
        }
        // Every compared tree flowers, so each flower is held to its leaf's growth too.
        params.foliage.blossom = Some(bevy::color::Color::WHITE);
        let full = generate_skeleton(&params);
        let data = build_growth_data(&full, &params);
        // (node id, ordinal) -> vertex range of that leaf (or its flower) in the grown mesh
        let ranges = |tags: &[Tag]| {
            let mut ranges: HashMap<(u32, u32), (usize, usize)> = HashMap::new();
            for (v, tag) in tags.iter().enumerate() {
                assert_eq!(tag.class, Class::Leaf);
                let entry = ranges.entry((tag.node, tag.ordinal)).or_insert((v, v));
                entry.1 = v + 1;
            }
            ranges
        };
        let grown = [(&data.leaf_tags, ranges(&data.leaf_tags)), (&data.flower_tags, ranges(&data.flower_tags))];
        // The mature tree's leaf directions, to tell a mirrored tip cluster.
        let mature = aged_skeleton(&full, &params, f32::INFINITY);
        let mut mature_points = Vec::new();
        let mut mature_direction = HashMap::new();
        // Under `AllBranches` the GPU's ordinal of a leaf is its rank: when it appears as the branch
        // lengthens (`leaf_ranks`); the leaf of slot `slot` of a branch with `total` of them.
        let all_branches = params.foliage.placement == FoliagePlacement::AllBranches;
        let ordinal = |slot: usize, total: usize| if all_branches { leaf_ranks(total)[slot] } else { slot as u32 };
        let mut mature_total = HashMap::new();
        for i in 0..mature.nodes.len() {
            let before = mature_points.len();
            leaf_points_for_node(&mature, i, &params.foliage, &params.growth, &params.trunk, params.seed, &mut mature_points);
            let total = mature_points.len() - before;
            mature_total.insert(mature.nodes[i].id, total);
            for (slot, point) in mature_points[before..].iter().enumerate() {
                mature_direction.insert((mature.nodes[i].id, ordinal(slot, total)), point.direction);
            }
        }
        for age in AGES {
            let aged = aged_skeleton(&full, &params, age);
            let mut state = GrowthState::default();
            state.update(&full, &params, age);
            let mut points = Vec::new();
            let mut owners = Vec::new();
            for i in 0..aged.nodes.len() {
                let before = points.len();
                leaf_points_for_node(&aged, i, &params.foliage, &params.growth, &params.trunk, params.seed, &mut points);
                let id = aged.nodes[i].id;
                // The slots of the grown branch that have a leaf now, in order: the branch's leaves.
                let total = mature_total.get(&id).copied().unwrap_or(0);
                let shown = points.len() - before;
                let slots: Vec<usize> = if all_branches {
                    let ranks = leaf_ranks(total);
                    (0..total).filter(|&slot| (ranks[slot] as usize) < shown).collect()
                } else {
                    (0..shown).collect()
                };
                assert_eq!(slots.len(), shown, "{} age {age}: node {id}'s leaves", preset.name());
                owners.extend(slots.into_iter().map(|slot| (id, ordinal(slot, total))));
            }
            let cpu = foliage_meshes(&points, &params.foliage, params.style);
            let built = [(&cpu.leaves, &cpu.firsts[0]), (&cpu.flowers, &cpu.firsts[1])];
            for ((what, (tags, leaves)), (mesh, firsts)) in ["leaf", "flower"].into_iter().zip(&grown).zip(built) {
                for (p, &(id, ordinal)) in owners.iter().enumerate() {
                    let end = firsts.get(p + 1).map_or(mesh.positions.len(), |&n| n as usize);
                    let &(from, to) = leaves.get(&(id, ordinal)).expect("the mature tree has every leaf");
                    assert_eq!(to - from, end - firsts[p] as usize, "{} age {age}: {what} vertex count", preset.name());
                    if points[p].direction.dot(mature_direction[&(id, ordinal)]) < 0.99 {
                        mirrored += to - from;
                        continue;
                    }
                    for j in 0..to - from {
                        let q = position(&tags[from + j], &state);
                        let built = mesh.positions[firsts[p] as usize + j];
                        assert!(
                            q.distance(built) < TOLERANCE,
                            "{} age {age}: node {id} {what} {ordinal} vertex {j}: grown {q:?}, built {built:?}",
                            preset.name()
                        );
                        compared += 1;
                        flowers += usize::from(what == "flower");
                    }
                }
                // Leaves (and flowers) the CPU did not grow yet have no size.
                let alive: std::collections::HashSet<(u32, u32)> = owners.iter().copied().collect();
                for (&(id, ordinal), &(from, to)) in leaves {
                    if alive.contains(&(id, ordinal)) {
                        continue;
                    }
                    let first = position(&tags[from], &state);
                    for v in from..to {
                        assert!(
                            position(&tags[v], &state).distance(first) < 1e-5,
                            "{} age {age}: node {id} {what} {ordinal} should have no size",
                            preset.name()
                        );
                    }
                }
            }
        }
    }
    assert!(compared > 10_000 && flowers > 5_000, "compared only {compared} vertices, {flowers} of flowers");
    // Tip clusters flip to the far side of the twig when they would sit inside
    // the mature trunk; the grown tree keeps the mature choice (a documented
    // difference), which only matters where twigs hang close to a thin young trunk.
    assert!(mirrored * 100 <= compared, "{mirrored} of {compared} leaf vertices are on the other side");
}

#[test]
fn state_buffer_has_the_documented_layout() {
    let params = TreePreset::Oak.params();
    let full = generate_skeleton(&params);
    let mut state = GrowthState::default();
    state.update(&full, &params, 3.0);
    let mut out = vec![[f32::NAN; 4]; state.vec4_len()];
    state.encode(&mut out);
    assert_eq!(out[0][0] as usize, full.nodes.len());
    assert_eq!(out[0][1] as usize, trunk_ring_count(&params.trunk) as usize);
    assert!(out.iter().flatten().all(|v| v.is_finite()), "every slot is written and finite");
    let node = &state.nodes[3];
    let at = super::state::HEADER_VEC4S + 3 * super::state::NODE_VEC4S;
    assert_eq!(Vec3::new(out[at][0], out[at][1], out[at][2]), node.start);
    assert_eq!(out[at][3], node.length);
}
