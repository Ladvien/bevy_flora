//! Tree skeleton: an explicit node tree (trunk, primaries, splits, sub-branches)
//! generated deterministically from [`TreeParams`].

use bevy::math::Vec3;
use std::f32::consts::{E, PI, TAU};

use crate::mesh::rings::perpendicular;
use crate::params::{BranchParams, TreeParams, TrunkParams, TrunkTermination};
use crate::rng::SeededRng;

/// Role of a node in the skeleton.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// Index 0, the only node without a parent.
    Trunk,
    /// Thin continuation of the trunk above its top.
    Leader,
    /// Branch attached to the trunk.
    Primary,
    /// One of the two halves of a Y-split primary (parent is the stub primary).
    Split,
    /// Recursive sub-branch.
    Sub,
}

/// One straight segment of the skeleton.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// Index of this node in the full (unaged) skeleton; stable across [`Skeleton::aged`].
    pub id: u32,
    /// Role of this node in the tree.
    pub kind: NodeKind,
    /// Parent index; `None` only for the trunk.
    pub parent: Option<u32>,
    /// Fraction along the parent's full length where this node attaches.
    pub attach_t: f32,
    /// Point on the parent's axis this node is attached to. Equals `start`
    /// except for trunk-attached nodes, which start on the trunk surface.
    pub anchor: Vec3,
    /// Start of the segment in tree-local space, in metres.
    pub start: Vec3,
    /// Unit direction the segment points in, before gravity bend.
    pub direction: Vec3,
    /// Full segment length in metres.
    pub length: f32,
    /// Downward bend of the wood toward the tip (`gravity_strength × (1 −
    /// stiffness)`; 0 = straight, always 0 for the trunk). See [`Node::axis`].
    pub bend: f32,
    /// Radius at the start of the segment, in metres.
    pub base_radius: f32,
    /// Radius at the tip of the segment, in metres.
    pub tip_radius: f32,
    /// Recursion depth (0 for trunk, primaries, leader and splits).
    pub depth: u8,
    /// Position within the crown (0 at the branch zone start, 1 at its end).
    pub height_ratio: f32,
    /// Age at which the node starts growing (set by [`crate::growth::assign_birth_times`]).
    pub birth: f32,
    /// Time from `birth` until full length.
    pub duration: f32,
    /// Length growth fraction, 1.0 after generation; set by [`Skeleton::aged`].
    pub progress: f32,
    /// Radius growth fraction, 1.0 after generation; set by [`Skeleton::aged`].
    pub radius_progress: f32,
    /// Number of direct children (maintained by [`Skeleton::recount_children`]).
    pub child_count: u32,
}

/// Nodes in pre-order: every parent index is smaller than its children's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Skeleton {
    /// All nodes; index 0 is the trunk.
    pub nodes: Vec<Node>,
}

impl Skeleton {
    /// True when node `i` has no children.
    pub fn is_terminal(&self, i: usize) -> bool {
        self.nodes[i].child_count == 0
    }

    /// End point of node `i` (the tip of its possibly bent wood).
    pub fn end(&self, i: usize) -> Vec3 {
        self.nodes[i].point_at(1.0)
    }

    /// Move every node hanging off a branch onto its parent's (bent) axis at
    /// `attach_t`, parents first so displacements cascade to descendants.
    /// [`Skeleton::aged`] places nodes the same way. With floor avoidance,
    /// each node's bent wood is then kept above `floor_level`.
    fn seat_on_parents(&mut self, branches: &BranchParams) {
        for i in 1..self.nodes.len() {
            let Some(p) = self.nodes[i].parent else { continue };
            let parent = &self.nodes[p as usize];
            if parent.kind != NodeKind::Trunk {
                let shift = parent.point_at(self.nodes[i].attach_t) - self.nodes[i].anchor;
                let node = &mut self.nodes[i];
                node.anchor += shift;
                node.start += shift;
            }
            if branches.floor_avoidance {
                self.nodes[i].keep_above(branches.floor_level);
            }
        }
    }

    /// Recompute `child_count` for every node.
    pub fn recount_children(&mut self) {
        for n in &mut self.nodes {
            n.child_count = 0;
        }
        for i in 0..self.nodes.len() {
            if let Some(p) = self.nodes[i].parent {
                self.nodes[p as usize].child_count += 1;
            }
        }
    }

    /// Indices of the direct children of node `i`.
    pub fn children(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        let parent = i as u32;
        self.nodes
            .iter()
            .enumerate()
            .filter(move |(_, n)| n.parent == Some(parent))
            .map(|(j, _)| j)
    }
}

/// Axis points of a bent branch; the branch mesher puts one ring on each.
pub const BENT_AXIS_POINTS: u32 = 6;

impl Node {
    /// `points` samples of the wood's axis, base to tip: `(center, direction)`.
    ///
    /// The direction at fraction `t` is `direction` with `t² · bend · h` taken
    /// off its vertical component, where `h` is the direction's horizontal
    /// length: gravity's lever arm, so a vertical branch does not bend and a
    /// near-vertical one cannot fold back on itself. The centers integrate
    /// those directions in equal steps of `length / (points − 1)`.
    pub fn axis(&self, points: u32) -> Vec<(Vec3, Vec3)> {
        let points = points.max(2);
        let step = self.length / (points - 1) as f32;
        let mut center = self.start;
        (0..points)
            .map(|k| {
                let t = k as f32 / (points - 1) as f32;
                let dir = self.bent_direction(t);
                let sample = (center, dir);
                center += dir * step;
                sample
            })
            .collect()
    }

    /// Point at fraction `f` (clamped to 0–1) of the way along the wood's axis.
    pub fn point_at(&self, f: f32) -> Vec3 {
        let f = f.clamp(0.0, 1.0);
        if self.bend <= 0.0 {
            return self.start + self.direction * (self.length * f);
        }
        let axis = self.axis(BENT_AXIS_POINTS);
        let x = f * (axis.len() - 1) as f32;
        let k = (x as usize).min(axis.len() - 2);
        axis[k].0.lerp(axis[k + 1].0, x - k as f32)
    }

    fn bent_direction(&self, t: f32) -> Vec3 {
        if self.bend <= 0.0 {
            return self.direction;
        }
        let d = self.direction;
        let lever = Vec3::new(d.x, 0.0, d.z).length();
        Vec3::new(d.x, d.y - t * t * self.bend * lever, d.z).normalize_or_zero()
    }

    /// Lowest point of the wood's surface (axis height minus the radius there).
    fn lowest(&self) -> f32 {
        let axis = self.axis(BENT_AXIS_POINTS);
        let last = (axis.len() - 1) as f32;
        axis.iter()
            .enumerate()
            .map(|(k, (c, _))| c.y - lerp(self.base_radius, self.tip_radius, k as f32 / last))
            .fold(f32::MAX, f32::min)
    }

    /// Keep the wood at or above `floor`: relax the gravity bend just enough
    /// (the lowest point only drops as the bend grows), and if the straight
    /// branch still points below the floor, shorten it to end on the floor.
    /// A branch whose base is already below the floor only has to stay above
    /// its base, so it keeps the droop it can.
    fn keep_above(&mut self, floor: f32) {
        let floor = if self.start.y < floor { floor.min(self.start.y - self.base_radius) } else { floor };
        if self.lowest() >= floor {
            return;
        }
        let full = self.bend;
        self.bend = 0.0;
        if self.lowest() >= floor {
            let (mut lo, mut hi) = (0.0, full);
            for _ in 0..16 {
                let mid = 0.5 * (lo + hi);
                self.bend = mid;
                if self.lowest() >= floor {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            self.bend = lo;
        } else if self.direction.y < 0.0 {
            let drop = self.start.y - floor - self.tip_radius;
            self.length = (drop / -self.direction.y).max(0.0);
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn random_vec(rng: &mut SeededRng) -> Vec3 {
    Vec3::new(
        rng.range(-1.0, 1.0),
        rng.range(-1.0, 1.0),
        rng.range(-1.0, 1.0),
    )
    .normalize_or_zero()
}

/// Horizontal offset of the trunk axis at normalized height `t`.
///
/// Shared by the trunk mesher and branch placement so branches sit on the
/// wobbling trunk surface.
pub fn trunk_wobble(trunk: &TrunkParams, seed: i32, t: f32) -> (f32, f32) {
    if trunk.randomness <= 0.0 {
        return (0.0, 0.0);
    }
    let seed_f = seed as f32;
    let scale = trunk.randomness * t * trunk.height * 0.3;
    (
        (t * PI + seed_f * 0.1).sin() * scale,
        (t * E + seed_f * 0.2).cos() * scale,
    )
}

/// Trunk radius at normalized height `t` (taper only, no root flare).
pub fn trunk_radius_at(trunk: &TrunkParams, t: f32) -> f32 {
    let taper_factor = t.powf(trunk.taper_curve);
    lerp(trunk.radius * trunk.flare, trunk.radius * trunk.taper, taper_factor)
}

/// How far `point` lies inside the trunk's tapered, wobbling cylinder
/// (positive = inside; root flares are ignored). Points above the trunk top
/// or below the ground are never inside.
pub fn trunk_depth(trunk: &TrunkParams, seed: i32, point: Vec3) -> f32 {
    let t = point.y / trunk.height;
    if !(0.0..=1.0).contains(&t) {
        return f32::NEG_INFINITY;
    }
    let (wx, wz) = trunk_wobble(trunk, seed, t);
    trunk_radius_at(trunk, t) - Vec3::new(point.x - wx, 0.0, point.z - wz).length()
}

/// Generate the full (mature) skeleton for `params`, including birth times.
pub fn generate_skeleton(params: &TreeParams) -> Skeleton {
    let mut rng = SeededRng::new(params.seed);
    let trunk = &params.trunk;

    let mut skeleton = Skeleton {
        nodes: vec![Node {
            id: 0,
            kind: NodeKind::Trunk,
            parent: None,
            attach_t: 0.0,
            anchor: Vec3::ZERO,
            start: Vec3::ZERO,
            direction: Vec3::Y,
            length: trunk.height,
            bend: 0.0,
            base_radius: trunk.radius * trunk.flare,
            tip_radius: trunk.radius * trunk.taper,
            depth: 0,
            height_ratio: 0.0,
            birth: 0.0,
            duration: 0.0,
            progress: 1.0,
            radius_progress: 1.0,
            child_count: 0,
        }],
    };

    // Draw order matches the original generator: all primaries first, then the
    // leader (and its sub-branches), then per-primary splits and sub-branches.
    let primaries = generate_primaries(params, &mut rng);

    if trunk.termination == TrunkTermination::LeaderBranch {
        let top_radius = trunk.radius * trunk.taper;
        let (wx, wz) = trunk_wobble(trunk, params.seed, 1.0);
        let start = Vec3::new(wx, trunk.height, wz);
        let leader_idx = push_node(
            &mut skeleton,
            Node {
                kind: NodeKind::Leader,
                parent: Some(0),
                attach_t: 1.0,
                anchor: start,
                start,
                direction: Vec3::Y,
                length: trunk.height * trunk.leader_length,
                base_radius: top_radius,
                tip_radius: top_radius * trunk.leader_taper,
                depth: 0,
                height_ratio: 1.0,
                ..blank_node()
            },
        );
        if trunk.leader_has_branches {
            generate_subs(&mut skeleton, leader_idx, params, &mut rng);
        }
    }

    for primary in primaries {
        let idx = push_node(&mut skeleton, primary);
        if let Some((child_a, child_b)) = try_split(&mut skeleton, idx, params, &mut rng) {
            let a = push_node(&mut skeleton, child_a);
            generate_subs(&mut skeleton, a, params, &mut rng);
            let b = push_node(&mut skeleton, child_b);
            generate_subs(&mut skeleton, b, params, &mut rng);
        } else {
            generate_subs(&mut skeleton, idx, params, &mut rng);
        }
    }

    skeleton.recount_children();
    let b = &params.branches;
    if b.gravity_strength > 0.0 {
        let bend = (b.gravity_strength * (1.0 - b.stiffness)).max(0.0);
        for node in skeleton.nodes.iter_mut().skip(1) {
            node.bend = bend;
        }
    }
    skeleton.seat_on_parents(&params.branches);
    crate::growth::assign_birth_times(&mut skeleton, &params.growth);
    skeleton
}

/// Placeholder values for fields filled in later (`..blank_node()`).
fn blank_node() -> Node {
    Node {
        id: 0,
        kind: NodeKind::Sub,
        parent: None,
        attach_t: 0.0,
        anchor: Vec3::ZERO,
        start: Vec3::ZERO,
        direction: Vec3::Y,
        length: 0.0,
        bend: 0.0,
        base_radius: 0.0,
        tip_radius: 0.0,
        depth: 0,
        height_ratio: 0.0,
        birth: 0.0,
        duration: 0.0,
        progress: 1.0,
        radius_progress: 1.0,
        child_count: 0,
    }
}

fn push_node(skeleton: &mut Skeleton, mut node: Node) -> usize {
    node.id = skeleton.nodes.len() as u32;
    skeleton.nodes.push(node);
    skeleton.nodes.len() - 1
}

/// Primary branches along the trunk on a phyllotaxis spiral.
fn generate_primaries(params: &TreeParams, rng: &mut SeededRng) -> Vec<Node> {
    let trunk = &params.trunk;
    let b = &params.branches;
    let mut out = Vec::new();
    let mut current_angle = 0.0f32;

    let start_height = trunk.height * b.start;
    let end_height = trunk.height * b.end;
    let zone_length = end_height - start_height;
    if zone_length <= 0.0 {
        return out;
    }

    let spacing = 1.0 / (b.density + 0.001);
    let branch_count = (zone_length / spacing).floor() as i32;

    for i in 0..branch_count {
        let base_height = start_height + spacing * i as f32;
        let height = base_height + rng.range(0.0, spacing * 0.5);
        if height > end_height {
            break;
        }

        if b.break_chance > 0.0 && rng.next_f32() < b.break_chance {
            continue;
        }

        current_angle += b.phyllotaxis_angle + rng.range(-10.0, 10.0);
        let angle_rad = current_angle.to_radians();

        let t = height / trunk.height;
        let trunk_r = trunk_radius_at(trunk, t);
        let (wobble_x, wobble_z) = trunk_wobble(trunk, params.seed, t);

        let anchor = Vec3::new(wobble_x, height, wobble_z);
        let start = Vec3::new(
            angle_rad.cos() * trunk_r + wobble_x,
            height,
            angle_rad.sin() * trunk_r + wobble_z,
        );

        let height_ratio = (height - start_height) / zone_length;
        let angle_adjustment = b.crown_angle_variation * (height_ratio - 0.5) * 30.0;
        let effective_angle = (b.angle + angle_adjustment).clamp(0.0, 90.0);

        let outward = Vec3::new(angle_rad.cos(), 0.0, angle_rad.sin());
        let base_dir = Vec3::Y.lerp(outward, effective_angle / 90.0);

        let flattened_dir = if b.flatness > 0.0 {
            Vec3::new(base_dir.x, base_dir.y * (1.0 - b.flatness), base_dir.z).normalize_or_zero()
        } else {
            base_dir
        };

        let random_offset = random_vec(rng) * b.randomness;
        let up_offset = Vec3::Y * b.up_attraction;
        let direction = (flattened_dir + random_offset + up_offset).normalize_or_zero();

        let base_radius = trunk_r * b.radius_ratio;
        let tip_radius = base_radius * (1.0 - b.taper);

        let shape_mult = b.crown_shape.length_multiplier(height_ratio);
        let final_mult = lerp(1.0, shape_mult, b.crown_influence);
        let variation_min = 1.0 - b.length_variation;
        let dominance_mult = 1.0 - (b.apical_dominance * height_ratio * 0.5);
        let length = trunk.height
            * b.length
            * final_mult
            * dominance_mult
            * rng.range(variation_min, 1.0);

        let direction = if b.angle_curve.abs() > 0.001 {
            let angle_adjustment = b.angle_curve * height_ratio * 30.0f32.to_radians();
            Vec3::new(direction.x, direction.y + angle_adjustment.sin(), direction.z)
                .normalize_or_zero()
        } else {
            direction
        };

        let (direction, length) = if b.floor_avoidance {
            let end_y = start.y + direction.y * length;
            if end_y < b.floor_level {
                if direction.y < -0.3 {
                    continue;
                } else if direction.y < 0.0 {
                    let clamped_dir = Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero();
                    let max_length = if clamped_dir.y.abs() < 0.001 {
                        length
                    } else {
                        ((start.y - b.floor_level) / -clamped_dir.y).max(0.1)
                    };
                    (clamped_dir, length.min(max_length))
                } else {
                    (direction, length)
                }
            } else {
                (direction, length)
            }
        } else {
            (direction, length)
        };

        out.push(Node {
            kind: NodeKind::Primary,
            parent: Some(0),
            attach_t: t,
            anchor,
            start,
            direction,
            length,
            base_radius,
            tip_radius,
            depth: 0,
            height_ratio,
            ..blank_node()
        });
    }

    out
}

/// Y-split of the primary at `idx`. On success the primary is shortened to the
/// stub and the two diverging children are returned (not yet pushed).
fn try_split(
    skeleton: &mut Skeleton,
    idx: usize,
    params: &TreeParams,
    rng: &mut SeededRng,
) -> Option<(Node, Node)> {
    let split = &params.split;
    if !split.enabled || rng.next_f32() > split.probability {
        return None;
    }
    let parent = &skeleton.nodes[idx];
    if parent.base_radius < split.radius_threshold {
        return None;
    }

    let split_t = split.position;
    let split_point = parent.start + parent.direction * parent.length * split_t;
    let remaining_length = parent.length * (1.0 - split_t);
    let split_radius = lerp(parent.base_radius, parent.tip_radius, split_t);

    let half_angle = (split.angle / 2.0).to_radians();
    let perp = perpendicular(parent.direction);
    let rotation = rng.range(0.0, TAU);
    let perp2 = parent.direction.cross(perp);
    let rotated_perp = perp * rotation.cos() + perp2 * rotation.sin();

    let dir1 = (parent.direction * half_angle.cos() + rotated_perp * half_angle.sin())
        .normalize_or_zero();
    let dir2 = (parent.direction * half_angle.cos() - rotated_perp * half_angle.sin())
        .normalize_or_zero();

    let branch_tip_radius = parent.tip_radius * 0.9;
    let template = Node {
        kind: NodeKind::Split,
        parent: Some(idx as u32),
        attach_t: 1.0,
        anchor: split_point,
        start: split_point,
        base_radius: split_radius * 0.7,
        tip_radius: branch_tip_radius * 0.7,
        depth: parent.depth,
        height_ratio: parent.height_ratio,
        ..blank_node()
    };
    let child_a = Node {
        direction: dir1,
        length: remaining_length * rng.range(0.9, 1.1),
        ..template.clone()
    };
    let child_b = Node {
        direction: dir2,
        length: remaining_length * rng.range(0.9, 1.1),
        ..template
    };

    // The stub tapers into its halves, so the fork shows no ledge.
    let stub = &mut skeleton.nodes[idx];
    stub.length *= split_t;
    stub.tip_radius = split_radius * 0.7;

    Some((child_a, child_b))
}

/// Recursively generate sub-branches of `parent_idx`, pushing in pre-order.
fn generate_subs(
    skeleton: &mut Skeleton,
    parent_idx: usize,
    params: &TreeParams,
    rng: &mut SeededRng,
) {
    let b = &params.branches;
    let depth = skeleton.nodes[parent_idx].depth as u32;
    if depth >= b.recursion {
        return;
    }

    for _ in 0..b.sub_branch_count {
        if b.break_chance > 0.0 && rng.next_f32() < b.break_chance {
            continue;
        }

        let parent = skeleton.nodes[parent_idx].clone();

        let base_t = rng.range(0.3, 0.8);
        let t = if b.sub_branch_position_bias > 0.0 {
            lerp(base_t, 0.8, b.sub_branch_position_bias)
        } else {
            lerp(base_t, 0.3, -b.sub_branch_position_bias)
        };
        let start = parent.start + parent.direction * parent.length * t;

        let spread = rng.range(30.0, 60.0).to_radians();
        let rotation = rng.range(0.0, TAU);

        let perp = perpendicular(parent.direction);
        let perp2 = parent.direction.cross(perp);
        let offset = perp * rotation.cos() + perp2 * rotation.sin();
        let spread_direction = |side: f32| {
            let d = (parent.direction * spread.cos() + offset * (side * spread.sin()))
                .normalize_or_zero();
            if b.flatness > 0.0 {
                Vec3::new(d.x, d.y * (1.0 - b.flatness), d.z).normalize_or_zero()
            } else {
                d
            }
        };

        let dominance_mult = 1.0 - (b.apical_dominance * parent.height_ratio * 0.4);
        let length = parent.length * b.sub_branch_scale * dominance_mult;
        let base_radius = lerp(parent.base_radius, parent.tip_radius, t) * b.radius_ratio;
        let tip_radius = base_radius * (1.0 - b.taper);

        // A sub-branch spreading toward the trunk would grow through it (steep
        // primaries hug the trunk); spread to the other side of the parent instead.
        let into_trunk = |dir: Vec3| {
            (1..=10).any(|k| {
                let p = start + dir * (length * k as f32 / 10.0);
                trunk_depth(&params.trunk, params.seed, p) > 0.0
            })
        };
        let mut direction = spread_direction(1.0);
        if into_trunk(direction) {
            direction = spread_direction(-1.0);
        }

        let (direction, force_terminal) = if b.floor_avoidance {
            let end_y = start.y + direction.y * length;
            if end_y < b.floor_level {
                if direction.y < -0.3 {
                    continue;
                } else if direction.y < 0.0 {
                    (
                        Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero(),
                        true,
                    )
                } else {
                    (direction, false)
                }
            } else {
                (direction, false)
            }
        } else {
            (direction, false)
        };

        let idx = push_node(
            skeleton,
            Node {
                kind: NodeKind::Sub,
                parent: Some(parent_idx as u32),
                attach_t: t,
                anchor: start,
                start,
                direction,
                length,
                base_radius,
                tip_radius,
                depth: (depth + 1) as u8,
                height_ratio: parent.height_ratio,
                ..blank_node()
            },
        );

        if !force_terminal {
            generate_subs(skeleton, idx, params, rng);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::TreePreset;

    #[test]
    fn skeleton_is_preorder() {
        for preset in TreePreset::ALL {
            let s = generate_skeleton(&preset.params());
            assert_eq!(s.nodes[0].kind, NodeKind::Trunk);
            for (i, n) in s.nodes.iter().enumerate().skip(1) {
                let p = n.parent.expect("non-trunk node has a parent") as usize;
                assert!(p < i, "{}: node {i} parent {p}", preset.name());
            }
        }
    }

    #[test]
    fn seed_determinism() {
        let params = TreePreset::Maple.params();
        let a = generate_skeleton(&params);
        let b = generate_skeleton(&params);
        assert_eq!(a.nodes.len(), b.nodes.len());
        for (x, y) in a.nodes.iter().zip(&b.nodes) {
            assert_eq!(x.start.to_array(), y.start.to_array());
            assert_eq!(x.length.to_bits(), y.length.to_bits());
        }
        let mut other = params.clone();
        other.seed += 1;
        let c = generate_skeleton(&other);
        assert!(a != c);
    }

    #[test]
    fn trunk_only_when_density_zero_zone() {
        let mut params = TreeParams::default();
        params.branches.start = 0.5;
        params.branches.end = 0.5;
        let s = generate_skeleton(&params);
        assert_eq!(s.nodes.len(), 1);
        assert!(s.is_terminal(0));
    }

    #[test]
    fn split_marks_primary_as_stub() {
        let mut unsplit = TreeParams::default();
        unsplit.branches.recursion = 0;
        unsplit.split.enabled = false;
        let mut split = unsplit.clone();
        split.split.enabled = true;
        split.split.probability = 1.0;
        split.split.radius_threshold = 0.0;

        let a = generate_skeleton(&unsplit);
        let b = generate_skeleton(&split);
        let primaries_a: Vec<&Node> = a.nodes.iter().filter(|n| n.kind == NodeKind::Primary).collect();
        let primaries_b: Vec<(usize, &Node)> = b
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.kind == NodeKind::Primary)
            .collect();
        assert!(!primaries_a.is_empty());
        assert_eq!(primaries_a.len(), primaries_b.len());
        for (orig, (idx, stub)) in primaries_a.iter().zip(&primaries_b) {
            assert!((stub.length - orig.length * split.split.position).abs() < 1e-5);
            assert_eq!(stub.child_count, 2);
            let kinds: Vec<NodeKind> = b.children(*idx).map(|c| b.nodes[c].kind).collect();
            assert_eq!(kinds, vec![NodeKind::Split, NodeKind::Split]);
        }
    }

    #[test]
    fn sub_branch_radius_follows_parent_at_attachment() {
        // `BranchParams::radius_ratio`: base radius relative to the parent
        // radius at the attachment point (as for primaries on the trunk).
        for preset in TreePreset::ALL {
            let params = preset.params();
            let s = generate_skeleton(&params);
            for n in s.nodes.iter().filter(|n| n.kind == NodeKind::Sub) {
                let parent = &s.nodes[n.parent.unwrap() as usize];
                let parent_r = lerp(parent.base_radius, parent.tip_radius, n.attach_t);
                let expected = parent_r * params.branches.radius_ratio;
                assert!(
                    (n.base_radius - expected).abs() <= 1e-6,
                    "{}: sub {} base radius {} != {expected}",
                    preset.name(),
                    n.id,
                    n.base_radius
                );
            }
        }
    }

    #[test]
    fn sub_branches_do_not_grow_into_the_trunk() {
        for preset in TreePreset::ALL {
            for seed in [42, 1, 2, 3, 4, 5, 6, 7] {
                let mut params = preset.params();
                params.seed = seed;
                let s = generate_skeleton(&params);
                for n in s.nodes.iter().filter(|n| n.kind == NodeKind::Sub) {
                    for k in 1..=10 {
                        let p = n.point_at(k as f32 / 10.0);
                        let depth = trunk_depth(&params.trunk, seed, p);
                        assert!(
                            depth <= 0.0,
                            "{} seed {seed}: sub {} is {depth} m inside the trunk at {p:?}",
                            preset.name(),
                            n.id
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn strong_gravity_never_folds_a_branch() {
        // Gravity bends wood toward the ground; it must never turn a branch
        // back on itself (consecutive axis directions more than 90° apart),
        // which pinches its tube inside out.
        let mut params = TreePreset::Oak.params();
        params.branches.gravity_strength = 2.0;
        params.branches.stiffness = 0.0;
        let s = generate_skeleton(&params);
        for n in s.nodes.iter().skip(1) {
            let axis = n.axis(BENT_AXIS_POINTS);
            for w in axis.windows(2) {
                assert!(
                    w[0].1.dot(w[1].1) > 0.0,
                    "{:?} {} folds: {:?} -> {:?}",
                    n.kind,
                    n.id,
                    w[0].1,
                    w[1].1
                );
            }
        }
    }

    #[test]
    fn bent_wood_stays_above_the_floor() {
        // Floor avoidance must hold for the bent axis, not just the straight
        // one: drooping willow twigs and downward branches low on the trunk.
        let mut low = TreePreset::Oak.params();
        low.branches.start = 0.05;
        low.branches.gravity_strength = 2.0;
        low.branches.stiffness = 0.0;
        low.branches.up_attraction = -1.0;
        let mut cases = vec![("droop low".to_string(), low)];
        for seed in 1..=8 {
            let mut willow = TreePreset::Willow.params();
            willow.seed = seed;
            cases.push((format!("willow seed {seed}"), willow));
        }
        for (label, params) in cases {
            let s = generate_skeleton(&params);
            for n in s.nodes.iter().skip(1) {
                let lowest = n.lowest();
                assert!(
                    lowest >= params.branches.floor_level - 1e-4,
                    "{label}: {:?} {} reaches {lowest} below the floor",
                    n.kind,
                    n.id
                );
            }
        }
    }

    #[test]
    fn branches_below_a_raised_floor_keep_their_droop() {
        // D-038: a branch that starts below the floor cannot satisfy it, so
        // it must not drop below its own base; it must not lose its whole
        // gravity bend either.
        let mut params = TreePreset::Oak.params();
        params.branches.floor_level = 2.0;
        params.branches.start = 0.1;
        let s = generate_skeleton(&params);
        let mut below = 0;
        for n in s.nodes.iter().skip(1).filter(|n| n.start.y < params.branches.floor_level) {
            below += 1;
            let floor = n.start.y - n.base_radius;
            assert!(n.lowest() >= floor - 1e-4, "{:?} {} dips below its own base", n.kind, n.id);
            if n.direction.y > 0.0 {
                assert!(n.bend > 0.0, "{:?} {} lost its gravity bend", n.kind, n.id);
            }
        }
        assert!(below > 0, "no branch starts below the raised floor");
    }

    #[test]
    fn split_stub_flows_into_its_halves() {
        // At a Y-split the stub must end no wider than the halves start, or
        // its end shows as a ledge around the fork.
        for preset in TreePreset::ALL {
            let mut params = preset.params();
            params.split.enabled = true;
            params.split.probability = 1.0;
            params.split.radius_threshold = 0.0;
            let s = generate_skeleton(&params);
            for (i, stub) in s.nodes.iter().enumerate() {
                for c in s.children(i).filter(|&c| s.nodes[c].kind == NodeKind::Split) {
                    let half = &s.nodes[c];
                    assert!(
                        stub.tip_radius <= half.base_radius + 1e-6,
                        "{}: stub {} ends at radius {} but half {} starts at {}",
                        preset.name(),
                        stub.id,
                        stub.tip_radius,
                        half.id,
                        half.base_radius
                    );
                }
            }
        }
    }
}
