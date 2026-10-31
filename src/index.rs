//! Finding trees: [`FloraIndex`] holds every standing tree by where it stands, for a game to ask
//! which trees are near a point (the player's axe, a blast, a fire) or which trunk a ray from the
//! camera or a weapon hits.
//!
//! The plugin keeps it up to date: every [`ProceduralTree`] (a [`ProceduralTreeAsset`] tree once
//! its asset has loaded) and every [`PooledTree`] (once its [`TreePool`] exists), at the age it
//! shows, from where it stands after transform propagation. A tree moved, rebuilt or grown is
//! updated that frame; a tree despawned or felled leaves it. Each tree is its trunk, a cone from its
//! foot to its top along the trunk's axis: what an axe or a bullet hits, not the crown, which a ray
//! passes through. The lying wood is kept apart ([`FloraIndex::obstacles_within`]): stumps, logs,
//! and the wood above the cut of a felled tree at rest (with a [`TreeShape`]) or a pooled
//! deadfall's lying tree, each as capsules, for a game's collision on the forest floor.
//!
//! It is a grid of [`FloraIndex::CELL`] square cells, so a query costs the trees near it, not all
//! of them: a hundred thousand pooled trees answer [`FloraIndex::nearest`] in microseconds.
//!
//! [`ProceduralTreeAsset`]: crate::ProceduralTreeAsset

use std::collections::HashMap;

use bevy::asset::{AssetEvent, Assets};
use bevy::ecs::prelude::*;
use bevy::math::{IVec2, Vec2, Vec3, Vec3Swizzles};
use bevy::shape::Ray3d;
use bevy::transform::components::GlobalTransform;

use crate::felling::{Felled, TreeLog, TreeStump};
use crate::shape::{TreeShape, WoodCapsule};
use crate::growth::fraction;
use crate::plugin::{ProceduralTree, TreeAge, TreeRender};
use crate::pool::{PooledTree, TreePool};
use crate::skeleton::Node;

/// Every standing tree by where it stands; see the [module docs](self). The plugin inserts and
/// maintains it; read it with `Res<FloraIndex>`.
#[derive(Resource, Default, Debug)]
pub struct FloraIndex {
    /// The trees in each cell their trunk reaches into, by cell.
    cells: HashMap<IVec2, Vec<Entity>>,
    /// Each tree, with the cells it is in (inclusive corners).
    trees: HashMap<Entity, (IndexedTree, IVec2, IVec2)>,
    /// The entities with lying wood in each cell it reaches into.
    obstacle_cells: HashMap<IVec2, Vec<Entity>>,
    /// Each entity's lying wood, with the cells it is in (inclusive corners).
    obstacles: HashMap<Entity, (Vec<Obstacle>, IVec2, IVec2)>,
}

/// A tree as [`FloraIndex`] knows it: its trunk, in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndexedTree {
    /// The tree's entity.
    pub entity: Entity,
    /// The middle of the trunk's foot.
    pub base: Vec3,
    /// Up the trunk, unit length.
    pub axis: Vec3,
    /// How long the trunk is, in world units (a growing tree's grows).
    pub height: f32,
    /// The trunk's radius at its foot, in world units.
    pub base_radius: f32,
    /// The trunk's radius at its top, in world units.
    pub top_radius: f32,
    /// A plant of the forest floor ([`TreeParams::undergrowth`](crate::TreeParams::undergrowth): the
    /// shrub, juniper and fern presets): skip it where only trees count.
    pub undergrowth: bool,
}

/// Where a ray met a trunk: [`FloraIndex::cast_ray`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeHit {
    /// The tree.
    pub tree: Entity,
    /// The point on the bark.
    pub point: Vec3,
    /// The bark's outward normal there, unit length.
    pub normal: Vec3,
    /// How far along the ray, in world units.
    pub distance: f32,
}

/// Lying wood as [`FloraIndex`] knows it: what a walker bumps into on the forest floor
/// ([`FloraIndex::obstacles_within`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obstacle {
    /// The entity it belongs to: the stump, the log, the felled tree, or the child of a pooled
    /// deadfall that holds its lying tree's shape.
    pub entity: Entity,
    /// What it is.
    pub kind: ObstacleKind,
    /// The wood, in world space and world units.
    pub capsule: WoodCapsule,
}

/// What an [`Obstacle`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ObstacleKind {
    /// A stump ([`TreeStump`]), felled or pooled: from its foot to its cut.
    Stump,
    /// A bucked log ([`TreeLog`]).
    Log,
    /// The trunk or a limb of a tree lying on the ground, the wood above its cut: a felled tree at
    /// rest that has a [`TreeShape`], or a pooled deadfall's lying tree.
    Lying,
}

impl Obstacle {
    /// How far its wood is from `at` (world x and z), across the ground: 0 at or inside it.
    pub fn distance_to(&self, at: Vec2) -> f32 {
        let (a, b) = (self.capsule.a.xz(), self.capsule.b.xz());
        let ab = b - a;
        let t = if ab.length_squared() > 0.0 { ((at - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        (at.distance(a + ab * t) - self.capsule.radius).max(0.0)
    }
}

/// On the child of a pooled deadfall that holds its lying tree's [`TreeShape`]: the height of the
/// cut in the tree's own space, below which the shape is the stump's.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct LyingWood {
    pub(crate) cut_y: f32,
}

impl IndexedTree {
    /// The middle of the trunk's top.
    pub fn top(&self) -> Vec3 {
        self.base + self.axis * self.height
    }

    /// The trunk's radius `h` world units up its axis (clamped to the trunk).
    pub fn radius_at(&self, h: f32) -> f32 {
        let t = if self.height > 0.0 { (h / self.height).clamp(0.0, 1.0) } else { 0.0 };
        self.base_radius + (self.top_radius - self.base_radius) * t
    }

    /// How far `at` (world x and z) is from the bark at the trunk's foot, in world units; 0 within it.
    pub fn distance_to(&self, at: Vec2) -> f32 {
        (self.base.xz().distance(at) - self.base_radius).max(0.0)
    }

    /// Where `ray` first meets the trunk's side within `max` world units, if it does.
    pub fn cast(&self, ray: Ray3d, max: f32) -> Option<TreeHit> {
        if self.height <= 0.0 {
            return None;
        }
        let (o, d, a) = (ray.origin, *ray.direction, self.axis);
        // The radius grows by `k` a metre up the axis: r(h) = base_radius + k h.
        let k = (self.top_radius - self.base_radius) / self.height;
        let w = o - self.base;
        let (wa, da) = (w.dot(a), d.dot(a));
        let (wp, dp) = (w - a * wa, d - a * da);
        let r0 = self.base_radius + k * wa;
        // |wp + t dp|² = (r0 + k da t)²
        let qa = dp.dot(dp) - k * k * da * da;
        let qb = 2.0 * (wp.dot(dp) - k * da * r0);
        let qc = wp.dot(wp) - r0 * r0;
        let mut roots = [f32::NAN; 2];
        if qa.abs() < 1.0e-9 {
            if qb.abs() > 1.0e-9 {
                roots[0] = -qc / qb;
            }
        } else {
            let disc = qb * qb - 4.0 * qa * qc;
            if disc < 0.0 {
                return None;
            }
            let s = disc.sqrt();
            roots = [(-qb - s) / (2.0 * qa), (-qb + s) / (2.0 * qa)];
            if roots[0] > roots[1] {
                roots.swap(0, 1);
            }
        }
        roots.into_iter().filter(|t| t.is_finite() && *t >= 0.0 && *t <= max).find_map(|t| {
            let h = wa + t * da;
            let radius = r0 + k * da * t;
            if !(0.0..=self.height).contains(&h) || radius <= 0.0 {
                return None;
            }
            let point = o + d * t;
            let out = (point - self.base - a * h).normalize_or(Vec3::X);
            Some(TreeHit { tree: self.entity, point, normal: (out - a * k).normalize(), distance: t })
        })
    }

    /// The cells the trunk reaches into, inclusive corners.
    fn cells(&self) -> (IVec2, IVec2) {
        let (top, reach) = (self.top(), self.base_radius.max(self.top_radius));
        let low = self.base.xz().min(top.xz()) - Vec2::splat(reach);
        let high = self.base.xz().max(top.xz()) + Vec2::splat(reach);
        (cell_of(low), cell_of(high))
    }
}

/// Whether an entry standing at `a` (world x, z), entity `first`, comes before one at `b`, `second`
/// in a cell's list: by x, then z, then (only for two at one spot) entity.
fn in_order(a: (f32, f32), first: Entity, b: (f32, f32), second: Entity) -> bool {
    a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)).then(first.to_bits().cmp(&second.to_bits())).is_lt()
}

fn cell_of(at: Vec2) -> IVec2 {
    (at / FloraIndex::CELL).floor().as_ivec2()
}

impl FloraIndex {
    /// The side of a grid cell, in world units (queries are in world units too: [`FloraUnits`](crate::FloraUnits)
    /// only scales what the crate measures in metres).
    pub const CELL: f32 = 8.0;

    /// How many trees it holds.
    pub fn len(&self) -> usize {
        self.trees.len()
    }

    /// Whether it holds no tree.
    pub fn is_empty(&self) -> bool {
        self.trees.is_empty()
    }

    /// The tree `entity`, if it is indexed.
    pub fn get(&self, entity: Entity) -> Option<&IndexedTree> {
        self.trees.get(&entity).map(|(tree, ..)| tree)
    }

    /// Every indexed tree, in no order.
    pub fn iter(&self) -> impl Iterator<Item = &IndexedTree> {
        self.trees.values().map(|(tree, ..)| tree)
    }

    /// The trees whose bark at the foot is within `radius` world units of `at` (world x and z), in an
    /// order that depends only on where they stand (cell by cell, and by position within a cell),
    /// not on the order they were spawned in.
    pub fn within(&self, at: Vec2, radius: f32) -> impl Iterator<Item = &IndexedTree> {
        let (low, high) = (cell_of(at - Vec2::splat(radius)), cell_of(at + Vec2::splat(radius)));
        (low.y..=high.y)
            .flat_map(move |y| (low.x..=high.x).map(move |x| IVec2::new(x, y)))
            .flat_map(move |cell| {
                self.cells.get(&cell).into_iter().flatten().filter_map(move |entity| {
                    let (tree, first, _) = self.trees.get(entity)?;
                    // A tree in several cells is met once: in the first of them the query covers.
                    (first.max(low) == cell && tree.distance_to(at) <= radius).then_some(tree)
                })
            })
    }

    /// The tree whose bark at the foot is nearest `at` (world x and z), within `max` world units;
    /// of two as near, the first in [`FloraIndex::within`]'s order.
    pub fn nearest(&self, at: Vec2, max: f32) -> Option<&IndexedTree> {
        self.within(at, max).min_by(|a, b| a.distance_to(at).total_cmp(&b.distance_to(at)))
    }

    /// The first trunk `ray` meets within `max` world units: what an axe swung, a bullet fired or the
    /// player's look along it hits. Crowns and leaves do not stop it.
    pub fn cast_ray(&self, ray: Ray3d, max: f32) -> Option<TreeHit> {
        let mut best: Option<TreeHit> = None;
        let consider = |cell: IVec2, best: &mut Option<TreeHit>| {
            for entity in self.cells.get(&cell).into_iter().flatten() {
                let Some((tree, ..)) = self.trees.get(entity) else { continue };
                let limit = best.map_or(max, |hit| hit.distance);
                if let Some(hit) = tree.cast(ray, limit) {
                    *best = Some(hit);
                }
            }
        };
        // Walk the cells the ray crosses over the ground (x, z), nearest first, until a hit is
        // nearer than the next cell.
        let flat = Vec2::new(ray.direction.x, ray.direction.z);
        let start = ray.origin.xz();
        let mut cell = cell_of(start);
        let speed = flat.length();
        if speed < 1.0e-6 {
            consider(cell, &mut best);
            return best;
        }
        let step = IVec2::new(flat.x.signum() as i32, flat.y.signum() as i32);
        let next_edge = |c: i32, s: i32| (c + i32::from(s > 0)) as f32 * Self::CELL;
        // Ray distance (3D) to the next x and y cell edges, and per cell.
        let along = |edge: f32, from: f32, rate: f32| if rate.abs() < 1.0e-9 { f32::INFINITY } else { (edge - from) / rate };
        let mut t_next = Vec2::new(
            along(next_edge(cell.x, step.x), start.x, flat.x),
            along(next_edge(cell.y, step.y), start.y, flat.y),
        );
        let t_step = Vec2::new(
            if flat.x.abs() < 1.0e-9 { f32::INFINITY } else { Self::CELL / flat.x.abs() },
            if flat.y.abs() < 1.0e-9 { f32::INFINITY } else { Self::CELL / flat.y.abs() },
        );
        loop {
            consider(cell, &mut best);
            let leave = t_next.x.min(t_next.y);
            if leave > max || best.is_some_and(|hit| hit.distance <= leave) {
                return best;
            }
            if t_next.x < t_next.y {
                cell.x += step.x;
                t_next.x += t_step.x;
            } else {
                cell.y += step.y;
                t_next.y += t_step.y;
            }
        }
    }

    /// Put `tree` in (again, if it was).
    fn insert(&mut self, tree: IndexedTree) {
        self.remove(tree.entity);
        let (low, high) = tree.cells();
        let Self { cells, trees, .. } = self;
        // Each cell keeps its trees by where they stand, so a query's order depends on the forest,
        // not on the order the trees were spawned or indexed in.
        let place = |e: &Entity| trees.get(e).map_or((0.0, 0.0), |(t, ..)| (t.base.x, t.base.z));
        for y in low.y..=high.y {
            for x in low.x..=high.x {
                let list = cells.entry(IVec2::new(x, y)).or_default();
                let at = list.partition_point(|e| in_order(place(e), *e, (tree.base.x, tree.base.z), tree.entity));
                list.insert(at, tree.entity);
            }
        }
        self.trees.insert(tree.entity, (tree, low, high));
    }

    /// Take `entity` out, if it is in.
    fn remove(&mut self, entity: Entity) {
        let Some((_, low, high)) = self.trees.remove(&entity) else { return };
        for y in low.y..=high.y {
            for x in low.x..=high.x {
                let cell = IVec2::new(x, y);
                if let Some(list) = self.cells.get_mut(&cell) {
                    list.retain(|e| *e != entity);
                    if list.is_empty() {
                        self.cells.remove(&cell);
                    }
                }
            }
        }
    }

    /// The lying wood within `radius` world units of `at` (world x and z): every stump, log and
    /// trunk or limb on the ground whose wood comes that near, in no order. What a walker bumps
    /// into on the forest floor, which [`FloraIndex::within`] (standing trees) does not hold.
    pub fn obstacles_within(&self, at: Vec2, radius: f32) -> impl Iterator<Item = &Obstacle> {
        let (low, high) = (cell_of(at - Vec2::splat(radius)), cell_of(at + Vec2::splat(radius)));
        (low.y..=high.y)
            .flat_map(move |y| (low.x..=high.x).map(move |x| IVec2::new(x, y)))
            .flat_map(move |cell| {
                self.obstacle_cells.get(&cell).into_iter().flatten().flat_map(move |entity| {
                    let found = self.obstacles.get(entity).filter(|(_, first, _)| first.max(low) == cell);
                    found.into_iter().flat_map(move |(wood, ..)| wood.iter().filter(move |o| o.distance_to(at) <= radius))
                })
            })
    }

    /// Every obstacle (stump, log, lying wood), in no order.
    pub fn obstacles(&self) -> impl Iterator<Item = &Obstacle> {
        self.obstacles.values().flat_map(|(wood, ..)| wood)
    }

    /// Put `entity`'s lying wood in (again, if it was); none takes it out.
    fn insert_obstacles(&mut self, entity: Entity, wood: Vec<Obstacle>) {
        self.remove_obstacles(entity);
        if wood.is_empty() {
            return;
        }
        let (mut lo, mut hi) = (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY));
        for capsule in wood.iter().map(|o| o.capsule) {
            for p in [capsule.a.xz(), capsule.b.xz()] {
                lo = lo.min(p - Vec2::splat(capsule.radius));
                hi = hi.max(p + Vec2::splat(capsule.radius));
            }
        }
        let (low, high) = (cell_of(lo), cell_of(hi));
        for y in low.y..=high.y {
            for x in low.x..=high.x {
                let list = self.obstacle_cells.entry(IVec2::new(x, y)).or_default();
                let place = |e: &Entity| self.obstacles.get(e).map_or((0.0, 0.0), |(wood, ..)| (wood[0].capsule.a.x, wood[0].capsule.a.z));
                let here = (wood[0].capsule.a.x, wood[0].capsule.a.z);
                let at = list.partition_point(|e| in_order(place(e), *e, here, entity));
                list.insert(at, entity);
            }
        }
        self.obstacles.insert(entity, (wood, low, high));
    }

    /// Take `entity`'s lying wood out, if it is in.
    fn remove_obstacles(&mut self, entity: Entity) {
        let Some((_, low, high)) = self.obstacles.remove(&entity) else { return };
        for y in low.y..=high.y {
            for x in low.x..=high.x {
                let cell = IVec2::new(x, y);
                if let Some(list) = self.obstacle_cells.get_mut(&cell) {
                    list.retain(|e| *e != entity);
                    if list.is_empty() {
                        self.obstacle_cells.remove(&cell);
                    }
                }
            }
        }
    }
}

/// A trunk in its tree's own space.
struct Trunk {
    start: Vec3,
    direction: Vec3,
    length: f32,
    base_radius: f32,
    tip_radius: f32,
}

impl Trunk {
    /// The trunk node `node`, `progress` of the way grown in length and `radius_progress` in
    /// girth.
    fn grown(node: &Node, progress: f32, radius_progress: f32) -> Self {
        Trunk {
            start: node.start,
            direction: node.direction,
            length: node.length * progress,
            base_radius: node.base_radius * radius_progress,
            tip_radius: node.tip_radius * radius_progress,
        }
    }

    /// The trunk where `place` stands it, as the index holds it.
    fn placed(&self, entity: Entity, place: &GlobalTransform, undergrowth: bool) -> IndexedTree {
        let base = place.transform_point(self.start);
        let top = place.transform_point(self.start + self.direction * self.length);
        let height = base.distance(top);
        let scale = place.affine().matrix3.x_axis.length();
        IndexedTree {
            entity,
            base,
            axis: if height > 0.0 { (top - base) / height } else { place.up().as_vec3() },
            height,
            base_radius: self.base_radius * scale,
            top_radius: self.tip_radius * scale,
            undergrowth,
        }
    }
}

/// Keep [`FloraIndex`] up to date: trees gone or felled out, trees new, moved, rebuilt or grown
/// in, and every tree of a pool that has just loaded or changed.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn index_trees(
    mut index: ResMut<FloraIndex>,
    pools: Option<Res<Assets<TreePool>>>,
    mut pool_events: MessageReader<AssetEvent<TreePool>>,
    mut gone_procedural: RemovedComponents<ProceduralTree>,
    mut gone_pooled: RemovedComponents<PooledTree>,
    felled: Query<Entity, Added<Felled>>,
    procedural: Query<
        (Entity, &ProceduralTree, &TreeAge, Option<&TreeRender>, &GlobalTransform),
        (
            Without<Felled>,
            Or<(Changed<GlobalTransform>, Changed<ProceduralTree>, Changed<TreeAge>, Changed<TreeRender>)>,
        ),
    >,
    moved_pooled: Query<(Entity, &PooledTree, &GlobalTransform), (Without<Felled>, Or<(Changed<GlobalTransform>, Changed<PooledTree>)>)>,
    all_pooled: Query<(Entity, &PooledTree, &GlobalTransform), Without<Felled>>,
) {
    for entity in gone_procedural.read().chain(gone_pooled.read()).chain(&felled) {
        index.remove(entity);
    }
    for (entity, tree, age, render, place) in &procedural {
        let shape = match render {
            // Its trunk at the age it shows.
            Some(render) => {
                let node = &render.full.nodes[0];
                let progress = fraction(age.0, node.birth, node.duration);
                let thickening = fraction(age.0, node.birth, node.duration * tree.0.growth.thickening_lag);
                Trunk::grown(node, progress, thickening)
            }
            // Not built yet: its trunk as the parameters give it, full grown.
            None => {
                let params = &tree.0.trunk;
                Trunk {
                    start: Vec3::ZERO,
                    direction: Vec3::Y,
                    length: params.height,
                    base_radius: params.radius * params.flare,
                    tip_radius: params.radius * params.taper,
                }
            }
        };
        index.insert(shape.placed(entity, place, tree.0.undergrowth));
    }
    let Some(pools) = pools else { return };
    let mut reloaded = Vec::new();
    for event in pool_events.read() {
        if let AssetEvent::Added { id } | AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id } = event {
            reloaded.push(*id);
        }
    }
    let mut add = |entity: Entity, pooled: &PooledTree, place: &GlobalTransform| {
        let Some(pool) = pools.get(&pooled.pool) else { return };
        let variant = &pool.variants[pooled.variant as usize % pool.variants.len()];
        index.insert(Trunk::grown(&variant.skeleton.nodes[0], 1.0, 1.0).placed(entity, place, pool.params.undergrowth));
    };
    for (entity, pooled, place) in &moved_pooled {
        add(entity, pooled, place);
    }
    if !reloaded.is_empty() {
        for (entity, pooled, place) in &all_pooled {
            if reloaded.contains(&pooled.pool.id()) {
                add(entity, pooled, place);
            }
        }
    }
}

/// Keep [`FloraIndex`]'s obstacles up to date: stumps and logs as they are made, moved or go; a
/// felled tree's wood above its cut once it lies still (it needs a [`TreeShape`]); a pooled
/// deadfall's lying tree ([`LyingWood`]).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn index_obstacles(
    mut index: ResMut<FloraIndex>,
    mut gone_stumps: RemovedComponents<TreeStump>,
    mut gone_logs: RemovedComponents<TreeLog>,
    mut gone_felled: RemovedComponents<Felled>,
    mut gone_lying: RemovedComponents<LyingWood>,
    stumps: Query<(Entity, &TreeStump, &GlobalTransform), Or<(Changed<TreeStump>, Changed<GlobalTransform>)>>,
    logs: Query<(Entity, &TreeLog, &GlobalTransform), Or<(Changed<TreeLog>, Changed<GlobalTransform>)>>,
    felled: Query<(Entity, &Felled, &TreeShape, &GlobalTransform), Or<(Changed<Felled>, Changed<TreeShape>, Changed<GlobalTransform>)>>,
    lying: Query<(Entity, &LyingWood, &TreeShape, &GlobalTransform), Or<(Changed<LyingWood>, Changed<TreeShape>, Changed<GlobalTransform>)>>,
) {
    for entity in gone_stumps.read().chain(gone_logs.read()).chain(gone_felled.read()).chain(gone_lying.read()) {
        index.remove_obstacles(entity);
    }
    for (entity, stump, place) in &stumps {
        index.insert_obstacles(entity, vec![Obstacle { entity, kind: ObstacleKind::Stump, capsule: stump.capsule.placed(place) }]);
    }
    for (entity, log, place) in &logs {
        index.insert_obstacles(entity, vec![Obstacle { entity, kind: ObstacleKind::Log, capsule: log.capsule.placed(place) }]);
    }
    let lying_wood = |entity: Entity, shape: &TreeShape, cut_y: f32, place: &GlobalTransform| -> Vec<Obstacle> {
        shape
            .capsules()
            .filter_map(|capsule| capsule.above(cut_y))
            .map(|capsule| Obstacle { entity, kind: ObstacleKind::Lying, capsule: capsule.placed(place) })
            .collect()
    };
    for (entity, fell, shape, place) in &felled {
        // A falling tree is no obstacle yet: it is where it is only for a frame.
        let wood = if fell.at_rest { lying_wood(entity, shape, fell.cut_y(), place) } else { Vec::new() };
        index.insert_obstacles(entity, wood);
    }
    for (entity, wood, shape, place) in &lying {
        index.insert_obstacles(entity, lying_wood(entity, shape, wood.cut_y, place));
    }
}

/// The index resource; its system is in the plugin's `PostUpdate`.
pub(crate) fn build(app: &mut bevy::app::App) {
    app.init_resource::<FloraIndex>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Dir3;

    /// A 6 m trunk, 0.5 m round at its foot and 0.25 m at its top, standing at (x, 0, z).
    fn tree(entity: Entity, x: f32, z: f32) -> IndexedTree {
        IndexedTree { entity, base: Vec3::new(x, 0.0, z), axis: Vec3::Y, height: 6.0, base_radius: 0.5, top_radius: 0.25, undergrowth: false }
    }

    #[test]
    fn a_ray_meets_the_trunk_on_its_bark_and_misses_above_and_beside_it() {
        let trunk = tree(World::new().spawn_empty().id(), 0.0, 0.0);
        // Level, 2 m up, where the radius is 0.5 - 0.25 * 2/6.
        let hit = trunk.cast(Ray3d::new(Vec3::new(-10.0, 2.0, 0.0), Dir3::X), 50.0).expect("a hit");
        let radius = 0.5 - 0.25 * 2.0 / 6.0;
        assert!((hit.point.x + radius).abs() < 1e-4, "hit at {}", hit.point);
        assert!((hit.distance - (10.0 - radius)).abs() < 1e-4);
        assert!(hit.normal.x < -0.99 && hit.normal.y > 0.0, "normal {}", hit.normal);
        // From inside the trunk the ray meets the far side.
        let inside = trunk.cast(Ray3d::new(Vec3::new(0.0, 1.0, 0.0), Dir3::Z), 5.0).expect("from inside");
        assert!(inside.point.z > 0.0);
        // Above the top, beside it, too short, or pointing away: nothing.
        for (origin, dir, max) in [
            (Vec3::new(-10.0, 6.5, 0.0), Dir3::X, 50.0),
            (Vec3::new(-10.0, 2.0, 0.6), Dir3::X, 50.0),
            (Vec3::new(-10.0, 2.0, 0.0), Dir3::X, 9.0),
            (Vec3::new(-10.0, 2.0, 0.0), Dir3::NEG_X, 50.0),
        ] {
            assert_eq!(trunk.cast(Ray3d::new(origin, dir), max), None, "{origin} {dir:?}");
        }
    }

    #[test]
    fn the_grid_finds_trees_near_a_point_and_along_a_ray_across_cells() {
        let mut world = World::new();
        let ids: Vec<Entity> = (0..11).map(|_| world.spawn_empty().id()).collect();
        let mut index = FloraIndex::default();
        // A row along x, 5 m apart, across several cells, and one standing on a cell corner.
        for (i, &id) in ids[..10].iter().enumerate() {
            index.insert(tree(id, -20.0 + 5.0 * i as f32, 3.0));
        }
        index.insert(tree(ids[10], 8.0, 8.0));
        assert_eq!(index.len(), 11);
        let mut near: Vec<Entity> = index.within(Vec2::new(0.0, 3.0), 5.6).map(|t| t.entity).collect();
        near.sort();
        let mut want = vec![ids[3], ids[4], ids[5]];
        want.sort();
        assert_eq!(near, want, "the trees within 5.6 m of (0, 3)");
        assert_eq!(index.within(Vec2::new(8.0, 8.0), 1.0).count(), 1, "a tree in four cells is met once");
        assert_eq!(index.nearest(Vec2::new(1.0, 4.0), 10.0).map(|t| t.entity), Some(ids[4]));
        assert!(index.nearest(Vec2::new(100.0, 100.0), 10.0).is_none());
        // Along the row from beyond its end: the first trunk it meets, not a farther one.
        let hit = index.cast_ray(Ray3d::new(Vec3::new(40.0, 1.0, 3.0), Dir3::NEG_X), 100.0).expect("a hit");
        assert_eq!(hit.tree, ids[9]);
        // Diagonally across cells to the tree on the corner.
        let from = Vec3::new(-6.0, 1.0, -6.0);
        let toward = Dir3::new(Vec3::new(8.0, 1.0, 8.0) - from).unwrap();
        assert_eq!(index.cast_ray(Ray3d::new(from, toward), 50.0).map(|h| h.tree), Some(ids[10]));
        // Taken out, it is gone from every cell.
        index.remove(ids[10]);
        assert!(index.within(Vec2::new(8.0, 8.0), 1.0).next().is_none());
        assert!(index.cells.values().all(|list| !list.contains(&ids[10])));
    }

    #[test]
    fn the_index_follows_trees_as_they_are_planted_grown_moved_felled_and_despawned() {
        use crate::{FellTree, LodSettings, TreePreset, build_pool};
        use bevy::asset::{AssetApp, AssetPlugin};
        use bevy::mesh::Mesh;
        use bevy::pbr::StandardMaterial;
        use bevy::time::TimeUpdateStrategy;
        use bevy::transform::components::Transform;
        let mut app = bevy::app::App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default(), bevy::transform::TransformPlugin));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(crate::FloraPlugin);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 30.0)));

        let oak = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), Transform::from_xyz(10.0, 0.0, 0.0))).id();
        let birch_params = TreePreset::Birch.params();
        let birch = app
            .world_mut()
            .spawn((ProceduralTree(birch_params.clone()), TreeAge(2.0), Transform::from_xyz(-10.0, 0.0, 0.0)))
            .id();
        let pool = build_pool(&TreePreset::Pine.params(), 2, f32::INFINITY, &LodSettings::default(), &mut app.world_mut().resource_mut::<Assets<Mesh>>());
        let pool = app.world_mut().resource_mut::<Assets<TreePool>>().add(pool);
        let pine = app.world_mut().spawn((PooledTree { pool, variant: 1 }, Transform::from_xyz(0.0, 0.0, 20.0))).id();
        app.update();
        app.update();

        let index = app.world().resource::<FloraIndex>();
        assert_eq!(index.len(), 3);
        assert_eq!(index.nearest(Vec2::new(9.0, 0.0), 5.0).map(|t| t.entity), Some(oak));
        assert_eq!(index.nearest(Vec2::new(0.5, 20.0), 2.0).map(|t| t.entity), Some(pine));
        // The young birch is indexed at its age: a shorter, thinner trunk than its grown one.
        let young = *index.get(birch).expect("the birch is indexed");
        assert!(young.height > 0.0 && young.height < birch_params.trunk.height, "a young trunk {} m tall", young.height);
        // A level ray from the middle meets the oak's bark, facing it.
        let hit = index.cast_ray(Ray3d::new(Vec3::new(0.0, 1.0, 0.0), bevy::math::Dir3::X), 50.0).expect("the oak is hit");
        let oak_tree = *index.get(oak).unwrap();
        assert_eq!(hit.tree, oak);
        assert!((hit.point.x - (10.0 - oak_tree.radius_at(1.0))).abs() < 0.01 && hit.normal.x < -0.9, "{hit:?}");

        // The birch grows: its trunk in the index grows with it.
        app.world_mut().get_mut::<TreeAge>(birch).unwrap().0 = 6.0;
        app.update();
        let grown = *app.world().resource::<FloraIndex>().get(birch).unwrap();
        assert!(grown.height > young.height && grown.base_radius > young.base_radius, "{young:?} -> {grown:?}");
        // The oak moves: it is found where it stands now.
        app.world_mut().get_mut::<Transform>(oak).unwrap().translation = Vec3::new(30.0, 0.0, 0.0);
        app.update();
        let index = app.world().resource::<FloraIndex>();
        assert!(index.nearest(Vec2::new(9.0, 0.0), 5.0).is_none());
        assert_eq!(index.nearest(Vec2::new(29.0, 0.0), 5.0).map(|t| t.entity), Some(oak));

        // The pine is felled: it leaves the index for good.
        app.world_mut().entity_mut(pine).insert(FellTree { speed: 4.0, ..Default::default() });
        for _ in 0..300 {
            app.update();
        }
        assert!(app.world().get::<Felled>(pine).is_some(), "the pine was not felled");
        assert!(app.world().resource::<FloraIndex>().get(pine).is_none(), "a felled tree is still indexed");
        // The oak is despawned: gone.
        app.world_mut().despawn(oak);
        app.update();
        let index = app.world().resource::<FloraIndex>();
        assert!(index.get(oak).is_none());
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn a_query_answers_in_an_order_set_by_where_the_trees_stand() {
        // Twelve trees 3 m apart, several to a cell.
        let spots: Vec<(f32, f32)> = (0..12).map(|i| ((i % 4) as f32 * 3.0 - 4.0, (i / 4) as f32 * 3.0 - 2.0)).collect();
        // The same trees, indexed in two orders as other entities.
        let ask = |reverse: bool| -> (Vec<(f32, f32)>, (f32, f32)) {
            let mut world = World::new();
            let mut index = FloraIndex::default();
            let mut placed = spots.clone();
            if reverse {
                placed.reverse();
                for _ in 0..7 {
                    world.spawn_empty();
                }
            }
            for (x, z) in placed {
                index.insert(tree(world.spawn_empty().id(), x, z));
            }
            let near = index.within(Vec2::ZERO, 20.0).map(|t| (t.base.x, t.base.z)).collect();
            // Halfway between two trees: the same one, both ways.
            let tie = index.nearest(Vec2::new(-2.5, -2.0), 5.0).map(|t| (t.base.x, t.base.z)).unwrap();
            (near, tie)
        };
        let (forward, backward) = (ask(false), ask(true));
        assert_eq!(forward.0.len(), 12);
        assert_eq!(forward, backward);
    }

    #[test]
    fn undergrowth_is_told_apart_from_trees() {
        use crate::{LodSettings, TreePreset, build_pool};
        use bevy::asset::{AssetApp, AssetPlugin, Assets};
        use bevy::mesh::Mesh;
        use bevy::pbr::StandardMaterial;
        use bevy::transform::components::Transform;
        let mut app = bevy::app::App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default(), bevy::transform::TransformPlugin));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(crate::FloraPlugin);
        let oak = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), Transform::default())).id();
        let fern = app.world_mut().spawn((ProceduralTree(TreePreset::Fern.params()), Transform::from_xyz(3.0, 0.0, 0.0))).id();
        let pool = {
            let built = app.world_mut().resource_scope(|_, mut meshes: bevy::ecs::world::Mut<Assets<Mesh>>| {
                build_pool(&TreePreset::Juniper.params(), 1, f32::INFINITY, &LodSettings::default(), &mut meshes)
            });
            app.world_mut().resource_mut::<Assets<TreePool>>().add(built)
        };
        let juniper = app.world_mut().spawn((PooledTree { pool, variant: 0 }, Transform::from_xyz(6.0, 0.0, 0.0))).id();
        app.update();
        app.update();
        let index = app.world().resource::<FloraIndex>();
        assert!(!index.get(oak).unwrap().undergrowth);
        assert!(index.get(fern).unwrap().undergrowth && index.get(juniper).unwrap().undergrowth);
        let trees: Vec<Entity> = index.within(Vec2::ZERO, 10.0).filter(|t| !t.undergrowth).map(|t| t.entity).collect();
        assert_eq!(trees, vec![oak]);
    }

    /// Runs `f` on its own thread and fails if it has not answered in five seconds: a query that
    /// walks 2^64 cells never does, and must not hang the whole test run.
    fn answers<T: Send + 'static>(what: &str, f: impl FnOnce() -> T + Send + 'static) -> T {
        let (send, answer) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = send.send(f());
        });
        answer.recv_timeout(std::time::Duration::from_secs(5)).unwrap_or_else(|_| panic!("{what} never returned"))
    }

    /// "The nearest standing tree, wherever it is": the chop clip's lumberjack written the obvious
    /// way. The walk covered every cell of the bound (2^64 for an infinite one), so it never ended
    /// (issue #92).
    #[test]
    fn an_unbounded_query_answers() {
        let mut world = World::new();
        let id = world.spawn_empty().id();
        let mut index = FloraIndex::default();
        index.insert(tree(id, 3.0, 4.0));
        let got = answers("an unbounded query", move || {
            let from = Vec3::new(-20.0, 1.0, 0.0);
            let nearest = index.nearest(Vec2::new(500.0, 0.0), f32::INFINITY).map(|t| t.entity);
            let in_reach = index.within(Vec2::new(500.0, 0.0), f32::MAX).count();
            // Along +z beside the tree: its ground track never crosses the trunk.
            let miss = index.cast_ray(Ray3d::new(from, Dir3::Z), f32::INFINITY);
            let far = index.cast_ray(Ray3d::new(from, Dir3::Z), f32::MAX);
            // Along the trunk's row from a million metres off still meets it.
            let hit = index.cast_ray(Ray3d::new(Vec3::new(-1.0e6, 1.0, 4.0), Dir3::X), f32::INFINITY).map(|h| h.tree);
            // A bound just short of the trunk does not reach it, however far the ray starts.
            let short = index.cast_ray(Ray3d::new(Vec3::new(-1.0e6, 1.0, 4.0), Dir3::X), 1.0e6);
            index.remove(id);
            let (emptied, emptied_ray) = (index.nearest(Vec2::ZERO, f32::INFINITY).is_none(), index.cast_ray(Ray3d::new(from, Dir3::X), f32::INFINITY).is_none());
            (nearest, in_reach, miss.is_none(), far.is_none(), hit, short.is_none(), emptied, emptied_ray)
        });
        assert_eq!(got, (Some(id), 1, true, true, Some(id), true, true, true));
    }

    /// A query bigger than the forest is cut to the cells the forest fills, and where those are
    /// mostly empty only the occupied ones are walked: the trees found, and the order they come in,
    /// are what they were when the whole box was walked cell by cell.
    #[test]
    fn a_huge_query_meets_the_same_trees_in_the_same_order() {
        let mut world = World::new();
        let mut index = FloraIndex::default();
        // A grove, a handful of loners 3 km apart (the box they fill is 750 cells wide, 60 of them
        // occupied), and two trees in one cell.
        let mut seed = 12345_u32;
        let mut next = |span: f32| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1 << 24) as f32 * span - span / 2.0
        };
        let mut spots: Vec<(f32, f32)> = (0..30).map(|_| (next(40.0), next(40.0))).collect();
        spots.extend((0..30).map(|_| (next(6000.0), next(6000.0))));
        for &(x, z) in &spots {
            let id = world.spawn_empty().id();
            index.insert(tree(id, x, z));
        }
        // Row by row of first cells, then by where a tree stands, then by entity: `within`'s order.
        let by_order = |a: &&IndexedTree, b: &&IndexedTree| {
            let (first_a, first_b) = (a.cells().0, b.cells().0);
            (first_a.y.cmp(&first_b.y))
                .then(first_a.x.cmp(&first_b.x))
                .then(a.base.x.total_cmp(&b.base.x))
                .then(a.base.z.total_cmp(&b.base.z))
                .then(a.entity.to_bits().cmp(&b.entity.to_bits()))
        };
        for at in [Vec2::ZERO, Vec2::new(-2500.0, 1800.0), Vec2::new(1.0e5, -1.0e5)] {
            for radius in [5.0, 60.0, 700.0, 2500.0, 1.0e5, 1.0e6, f32::MAX, f32::INFINITY] {
                let mut found: Vec<&IndexedTree> = index.within(at, radius).collect();
                let mut want: Vec<&IndexedTree> = index.iter().filter(|t| t.distance_to(at) <= radius).collect();
                assert_eq!(found.len(), want.len(), "{} trees within {radius} of {at}", want.len());
                if radius >= 1.0e6 {
                    // The whole forest is covered, so every tree is met in its first cell.
                    want.sort_by(by_order);
                    assert_eq!(found, want, "the order of everything within {radius} of {at}");
                } else {
                    found.sort_by_key(|t| t.entity);
                    want.sort_by_key(|t| t.entity);
                    assert_eq!(found, want, "the trees within {radius} of {at}");
                }
            }
        }
        let nearest = |at: Vec2, max: f32| index.nearest(at, max).map(|t| (t.base.x, t.base.z));
        let brute = |at: Vec2| index.iter().min_by(|a, b| a.distance_to(at).total_cmp(&b.distance_to(at))).map(|t| (t.base.x, t.base.z));
        for at in [Vec2::ZERO, Vec2::new(-2500.0, 1800.0), Vec2::new(1.0e5, -1.0e5)] {
            assert_eq!(nearest(at, f32::INFINITY), brute(at), "the nearest tree to {at}");
        }
    }

    /// The lying wood's walk is bounded the same way.
    #[test]
    fn an_unbounded_obstacle_query_answers() {
        let mut world = World::new();
        let mut index = FloraIndex::default();
        let stump = |entity: Entity, x: f32, z: f32| Obstacle {
            entity,
            kind: ObstacleKind::Stump,
            capsule: WoodCapsule { a: Vec3::new(x, 0.0, z), b: Vec3::new(x, 0.5, z), radius: 0.4 },
        };
        let ids: Vec<Entity> = (0..2).map(|_| world.spawn_empty().id()).collect();
        index.insert_obstacles(ids[0], vec![stump(ids[0], 5.0, 5.0)]);
        index.insert_obstacles(ids[1], vec![stump(ids[1], -400.0, 900.0)]);
        let found = answers("an unbounded obstacle query", move || {
            let all: Vec<Entity> = index.obstacles_within(Vec2::new(1.0e5, 1.0e5), f32::INFINITY).map(|o| o.entity).collect();
            let near = index.obstacles_within(Vec2::ZERO, 20.0).count();
            let empty = FloraIndex::default().obstacles_within(Vec2::ZERO, f32::INFINITY).count();
            (all, near, empty)
        });
        // Row by row: the one at z = 5, then the one at z = 900.
        assert_eq!(found, (vec![ids[0], ids[1]], 1, 0));
    }
}
