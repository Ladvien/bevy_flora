//! Continuous age model.
//!
//! Every skeleton node gets a `birth` age and a growth `duration` derived from
//! topology ([`assign_birth_times`]); [`Skeleton::aged`] then produces the
//! skeleton as it looks at any age. Progress is linear on purpose: a child's
//! birth coincides exactly with its parent reaching `attach_t`, and a primary
//! at height `y` is born exactly when the trunk reaches `y`.

use bevy::reflect::Reflect;

use crate::skeleton::{Node, NodeKind, Skeleton};

/// Timing of growth, in age units ("years").
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(default, deny_unknown_fields))]
pub struct GrowthParams {
    /// Age at which trunk-attached growth reaches the parametric target.
    pub maturity_age: f32,
    /// Trunk reaches full height at `maturity_age * trunk_fraction`.
    pub trunk_fraction: f32,
    /// Primary, leader and split nodes grow for `maturity_age * branch_fraction`.
    pub branch_fraction: f32,
    /// Sub-branch duration = parent duration × this.
    pub child_duration_scale: f32,
    /// Radii reach full size after `duration * thickening_lag`.
    pub thickening_lag: f32,
    /// Leaves appear once a node's length progress reaches this; leaf size
    /// scales to full at progress 1.
    pub leaf_threshold: f32,
}

impl Default for GrowthParams {
    fn default() -> Self {
        Self {
            maturity_age: 10.0,
            trunk_fraction: 0.4,
            branch_fraction: 0.35,
            child_duration_scale: 0.8,
            thickening_lag: 2.0,
            leaf_threshold: 0.5,
        }
    }
}

/// Fill `birth`/`duration` for every node (parents first).
pub fn assign_birth_times(skeleton: &mut Skeleton, growth: &GrowthParams) {
    let branch_duration = growth.maturity_age * growth.branch_fraction;
    for i in 0..skeleton.nodes.len() {
        let Some(parent_idx) = skeleton.nodes[i].parent else {
            let trunk = &mut skeleton.nodes[i];
            trunk.birth = 0.0;
            trunk.duration = growth.maturity_age * growth.trunk_fraction;
            continue;
        };
        let parent = &skeleton.nodes[parent_idx as usize];
        let (birth, duration) = match skeleton.nodes[i].kind {
            NodeKind::Trunk => unreachable!("trunk has no parent"),
            NodeKind::Primary | NodeKind::Leader => {
                (parent.duration * skeleton.nodes[i].attach_t, branch_duration)
            }
            NodeKind::Split => (parent.birth + parent.duration, branch_duration),
            NodeKind::Sub => (
                parent.birth + parent.duration * skeleton.nodes[i].attach_t,
                parent.duration * growth.child_duration_scale,
            ),
        };
        let node = &mut skeleton.nodes[i];
        node.birth = birth;
        node.duration = duration;
    }
}

/// How far a node born at `birth` and growing for `duration` has grown at `age`, 0 to 1.
pub(crate) fn fraction(age: f32, birth: f32, duration: f32) -> f32 {
    if duration <= 0.0 {
        if age >= birth { 1.0 } else { 0.0 }
    } else {
        ((age - birth) / duration).clamp(0.0, 1.0)
    }
}

impl Skeleton {
    /// The skeleton at `age`.
    ///
    /// Nodes not yet born (`age <= birth`) are dropped with their descendants.
    /// Surviving nodes get `progress = clamp((age - birth) / duration)`,
    /// `length *= progress`, `radius_progress = clamp((age - birth) /
    /// (duration * thickening_lag))`, radii scaled by `radius_progress`, and
    /// trunk-attached starts pulled toward the trunk axis by the trunk's
    /// `radius_progress`. Nodes on a branch sit on the aged parent's (bent)
    /// axis at their attachment distance. Indices are remapped; `child_count`
    /// keeps the full-grown count, so terminal-branch leaves never move.
    /// The trunk is always kept (with `progress = 0` at `age <= 0`);
    /// `age = f32::INFINITY` returns a copy with every progress at 1.
    pub fn aged(&self, age: f32, growth: &GrowthParams) -> Skeleton {
        let mut nodes: Vec<Node> = Vec::with_capacity(self.nodes.len());
        // Old index -> new index for kept nodes.
        let mut remap: Vec<Option<u32>> = vec![None; self.nodes.len()];
        // Start displacement of each kept node, by new index.
        let mut shift: Vec<bevy::math::Vec3> = Vec::with_capacity(self.nodes.len());

        for (i, node) in self.nodes.iter().enumerate() {
            let parent_new = match node.parent {
                None => None,
                Some(p) => match remap[p as usize] {
                    Some(new_p) if age > node.birth => Some(new_p),
                    _ => continue,
                },
            };

            let progress = fraction(age, node.birth, node.duration);
            let radius_progress = fraction(age, node.birth, node.duration * growth.thickening_lag);

            // `anchor` stays on the parent's axis: for trunk-attached nodes the
            // trunk axis point (only `start` moves in as the trunk is thinner),
            // for nodes on a branch the attachment point itself.
            let (start, anchor) = match parent_new {
                None => (node.start, node.anchor),
                Some(p) => {
                    let parent = &nodes[p as usize];
                    if parent.kind == NodeKind::Trunk {
                        let anchor = node.anchor + shift[p as usize];
                        let radial = (node.start - node.anchor) * parent.radius_progress;
                        (anchor + radial, anchor)
                    } else {
                        // Same distance along the parent as in the full skeleton.
                        let along = if parent.progress > 0.0 {
                            node.attach_t / parent.progress
                        } else {
                            0.0
                        };
                        let start = parent.point_at(along);
                        (start, start)
                    }
                }
            };

            remap[i] = Some(nodes.len() as u32);
            shift.push(start - node.start);
            nodes.push(Node {
                parent: parent_new,
                anchor,
                start,
                length: node.length * progress,
                base_radius: node.base_radius * radius_progress,
                tip_radius: node.tip_radius * radius_progress,
                progress,
                radius_progress,
                ..node.clone()
            });
        }

        Skeleton { nodes }
    }

    /// Age after which [`Skeleton::aged`] no longer changes anything.
    pub fn mature_at(&self, growth: &GrowthParams) -> f32 {
        self.nodes
            .iter()
            .map(|n| n.birth + n.duration * growth.thickening_lag.max(1.0))
            .fold(0.0, f32::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::TreePreset;
    use crate::skeleton::generate_skeleton;

    #[test]
    fn age_zero_has_only_trunk() {
        let params = TreePreset::Oak.params();
        let full = generate_skeleton(&params);
        let aged = full.aged(0.0, &params.growth);
        assert_eq!(aged.nodes.len(), 1);
        assert_eq!(aged.nodes[0].kind, NodeKind::Trunk);
        assert_eq!(aged.nodes[0].progress, 0.0);
        assert_eq!(aged.nodes[0].length, 0.0);
    }

    #[test]
    fn mature_equals_unaged() {
        let params = TreePreset::Oak.params();
        let full = generate_skeleton(&params);
        let aged = full.aged(f32::INFINITY, &params.growth);
        assert_eq!(aged.nodes.len(), full.nodes.len());
        for (a, b) in aged.nodes.iter().zip(&full.nodes) {
            assert_eq!(a.length, b.length);
            assert_eq!(a.start, b.start);
            assert_eq!(a.base_radius, b.base_radius);
            assert_eq!(a.progress, 1.0);
            assert_eq!(a.child_count, b.child_count);
        }
    }

    #[test]
    fn child_born_when_parent_reaches_attach() {
        let params = TreePreset::Oak.params();
        let full = generate_skeleton(&params);
        let (child_idx, child) = full
            .nodes
            .iter()
            .enumerate()
            .find(|(_, n)| n.kind == NodeKind::Sub)
            .expect("oak has sub-branches");
        let parent_idx = child.parent.unwrap() as usize;
        let parent_full_length = full.nodes[parent_idx].length;

        let age = child.birth + 1e-3;
        let aged = full.aged(age, &params.growth);
        // The child exists, and its parent's length is at attach_t of its full length.
        let aged_child = aged
            .nodes
            .iter()
            .find(|n| n.kind == NodeKind::Sub && n.birth == child.birth && n.attach_t == child.attach_t)
            .unwrap_or_else(|| panic!("child {child_idx} should exist at age {age}"));
        let aged_parent = &aged.nodes[aged_child.parent.unwrap() as usize];
        let expected = parent_full_length * child.attach_t;
        assert!(
            (aged_parent.length - expected).abs() < 1e-3,
            "parent length {} vs expected {expected}",
            aged_parent.length
        );
        // The child starts where the parent currently ends.
        let parent_end = aged_parent.start + aged_parent.direction * aged_parent.length;
        assert!(aged_child.start.distance(parent_end) < 1e-3);
    }

    #[test]
    fn monotone_node_count() {
        let params = TreePreset::Oak.params();
        let full = generate_skeleton(&params);
        let mut last = 0;
        let mut age = 0.0;
        while age <= params.growth.maturity_age {
            let n = full.aged(age, &params.growth).nodes.len();
            assert!(n >= last, "node count dropped from {last} to {n} at age {age}");
            last = n;
            age += 1.0;
        }
    }

    #[test]
    fn primaries_sit_on_thin_trunk() {
        let mut params = TreePreset::Oak.params();
        params.trunk.randomness = 0.0;
        let full = generate_skeleton(&params);
        let g = &params.growth;
        let trunk_duration = g.maturity_age * g.trunk_fraction;
        let aged = full.aged(trunk_duration, g);
        let trunk = &aged.nodes[0];
        assert!(trunk.radius_progress < 1.0);
        for n in aged.nodes.iter().filter(|n| n.kind == NodeKind::Primary) {
            let t = n.start.y / params.trunk.height;
            let r = crate::skeleton::trunk_radius_at(&params.trunk, t) * trunk.radius_progress;
            let horizontal = (n.start.x * n.start.x + n.start.z * n.start.z).sqrt();
            assert!((horizontal - r).abs() < 1e-4, "{horizontal} vs {r}");
        }
    }

    #[test]
    fn trunk_attached_anchors_stay_on_the_trunk_axis() {
        let params = TreePreset::Oak.params(); // wobbling trunk
        let full = generate_skeleton(&params);
        for age in [2.0, 4.0, 6.0, f32::INFINITY] {
            let aged = full.aged(age, &params.growth);
            for n in aged.nodes.iter().filter(|n| n.kind == NodeKind::Primary) {
                let t = n.anchor.y / params.trunk.height;
                let (wx, wz) = crate::skeleton::trunk_wobble(&params.trunk, params.seed, t);
                let off_axis = (n.anchor.x - wx).hypot(n.anchor.z - wz);
                assert!(off_axis < 1e-4, "age {age}: primary {} anchor {off_axis} m off the trunk axis", n.id);
            }
        }
    }
}
