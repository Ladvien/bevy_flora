//! How well a tree is: [`TreeCondition`], a fact about one tree that a game sets, reads back and sees.
//!
//! A game that wants a sick tree, a withering stand or a copse that dies in one season inserts
//! [`TreeCondition`] on the tree's entity (a [`ProceduralTree`](crate::ProceduralTree) or a
//! [`PooledTree`](crate::PooledTree)) and finds it again through [`FloraIndex`](crate::FloraIndex):
//! [`IndexedTree::condition`](crate::IndexedTree::condition) on every tree a query returns, or
//! [`FloraIndex::condition_of`](crate::FloraIndex::condition_of) for one entity.
//!
//! The tree is drawn in its condition. A sick crown is thinned and mottled with dry leaves, a dying
//! one is mostly bare and brown ([`TreeCondition::defoliation`], [`TreeCondition::withering`]); the
//! wood, the fruit and the flowers are as they were. The class rides in the [`MeshTag`] of every
//! mesh the tree is drawn with, above the glide tag of a pooled look (`glide.rs`), so trees of one
//! mesh and material still batch whatever their conditions; the shaders (`condition_*` in
//! `common.wesl`) hide the leaves and move the colours of the rest. A game that wants a look of its
//! own on top recolours the tree ([`RecolorTree`](crate::RecolorTree), or a
//! [`PooledLook`](crate::PooledLook) of a pool's own looks); the two compose.
//!
//! Pure CPU like `shape.rs` (it derives `Component`), except `sync_condition_tags`, which writes
//! the meshes' `MeshTag` (a data component, as `Mesh3d` is).

use bevy::color::Color;
use bevy::ecs::prelude::*;
use bevy::mesh::{Mesh3d, MeshTag};
use bevy::reflect::Reflect;

/// The dry leaf colour a sick or dying tree's leaves move toward, whatever their own colour:
/// `FloraUniform::withered` is its ratio to the species' leaf colour. A design default
/// (`docs/research/tree-condition.md`).
pub(crate) const WITHERED_LEAF: Color = Color::srgb(0.47, 0.31, 0.11);

/// Where a tree's condition starts in the [`MeshTag`] of its meshes: the bits below it are the glide
/// tag of a pooled look (at most `MAX_GLIDE_LOOKS`, 64), the two bits from it the class. `common.wesl`'s
/// `CONDITION_SHIFT` is this number.
pub(crate) const TAG_SHIFT: u32 = 8;

/// The bits of a [`MeshTag`] that are a pooled look's.
pub(crate) const TAG_LOOK_MASK: u32 = (1 << TAG_SHIFT) - 1;

/// The condition of a tree, in the three classes of a forest-health survey's crown assessment
/// (`docs/research/tree-condition.md`): a healthy crown, one in decline, one close to death. Ordered
/// from well to ill, so `condition >= TreeCondition::Sick` means "not healthy".
///
/// **Insert it on the tree's entity**, `commands.entity(tree).insert(TreeCondition::Dying)`, and
/// [`FloraIndex`](crate::FloraIndex) shows it from the next [`PostUpdate`](bevy::app::PostUpdate);
/// the tree is drawn in it from then on (see the module's docs). Insert [`TreeCondition::Healthy`] or
/// remove the component to clear it: a tree without one is healthy, so a forest of a hundred
/// thousand pays nothing for the trees that are well. To react to a change, query
/// `Changed<TreeCondition>`.
///
/// The condition belongs to the **tree's entity**, not to a [`TreePool`](crate::TreePool) its meshes
/// come from: a [`PooledTree`](crate::PooledTree) that changes variant or pool, is taken off the entity
/// and put back, or whose pool reloads, keeps it. A tree set before it is indexed (a pooled tree whose
/// pool has not loaded yet) shows it as soon as it is. Despawning the tree takes it with it.
///
/// A felled tree leaves the index ([`Felled`](crate::Felled)): [`FloraIndex::condition_of`] answers
/// `None` for it, and the component stays on its entity as it was when the tree was cut, for a
/// `Query<&TreeCondition>` to read; the felled tree keeps its look as it falls. There is no
/// `Dead` condition: a dead tree is the game's to fell ([`FellTree`](crate::FellTree)), replace, or
/// leave standing.
///
/// The look needs the flora materials ([`FloraMaterialSupport`](crate::FloraMaterialSupport)): a
/// device that draws trees with plain materials (no vertex-stage storage buffers, WebGL2) shows
/// the state and not the look, as it shows no glide. A hull crown ([`CrownStyle::Hull`](crate::CrownStyle))
/// withers but keeps all its facets.
///
/// [`FloraIndex::condition_of`]: crate::FloraIndex::condition_of
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
#[reflect(Component)]
pub enum TreeCondition {
    /// A full crown: the state of a tree with no `TreeCondition`.
    #[default]
    Healthy,
    /// A crown in decline: thinned by [`defoliation`](Self::defoliation), its leaves half way to dry.
    /// The tree is still standing and may recover.
    Sick,
    /// A crown close to death: mostly bare, what leaves it has dry. The class of the survey's
    /// severe grade, ~80% of the crown gone.
    Dying,
}

impl TreeCondition {
    /// The share of a crown's leaves this condition has lost, 0 to 1: nothing, about a third, about
    /// four fifths (the survey's slightly to moderately and its severely declining stands,
    /// `docs/research/tree-condition.md`). Which leaves are lost is the tree's own, the same ones at
    /// every level of detail and in the shadow maps, and the same whatever the tree's age.
    /// Flowers and fruit are not leaves and are not lost.
    pub const fn defoliation(self) -> f32 {
        match self {
            Self::Healthy => 0.0,
            Self::Sick => 0.35,
            Self::Dying => 0.8,
        }
    }

    /// How far the leaves that stay have gone from their colour to a dry ochre-brown, 0 to 1 on
    /// average: each leaf goes its own way, up to 0.4 either side of this (never past all or none).
    pub const fn withering(self) -> f32 {
        match self {
            Self::Healthy => 0.0,
            Self::Sick => 0.5,
            Self::Dying => 1.0,
        }
    }

    /// The bits of a mesh's [`MeshTag`] that carry this condition (`common.wesl`'s `tree_condition`
    /// reads them back).
    pub(crate) const fn tag_bits(self) -> u32 {
        (self as u32) << TAG_SHIFT
    }
}

/// `leaf_decline_moment` in `common.wesl`: a leaf's draw for its tree's decline, 0 to 1, from its
/// phase (`sway.w`). A leaf is lost when it is below [`TreeCondition::defoliation`]. It is not the
/// season's moment, so the leaves a sick tree loses are not the ones that would turn first.
pub(crate) fn leaf_decline_moment(phase: f32) -> f32 {
    let x = phase * 4.61 + 0.719;
    x - x.floor()
}

/// A mesh's tag for a pooled look's glide tag `look` and the tree's condition (none: healthy).
pub(crate) fn tag_for(look: u32, condition: Option<&TreeCondition>) -> u32 {
    (look & TAG_LOOK_MASK) | condition.map_or(0, |condition| condition.tag_bits())
}

/// What a tree's condition has done to its leaves, for what the CPU draws from the tree's leaf mesh
/// (the leaves that fall from it): `leaf_defoliated` and `leaf_withering` of `common.wesl`,
/// mirrored, so a leaf the shaders have hidden does not fall and a leaf that falls is as dry as
/// those left on the tree (the burst carries how far, the shader the colour:
/// `ATTRIBUTE_FALL_FACE`'s w).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Decline {
    lost: f32,
    withering: f32,
}

impl Decline {
    /// For a tree in `condition` (none: healthy). A hull crown loses none of its facets.
    pub(crate) fn new(condition: Option<&TreeCondition>, hull: bool) -> Self {
        let condition = condition.copied().unwrap_or_default();
        Self { lost: if hull { 0.0 } else { condition.defoliation() }, withering: condition.withering() }
    }

    /// Whether the leaf with this phase (`sway.w`) is one the tree has lost.
    pub(crate) fn has_lost(&self, phase: f32) -> bool {
        self.lost > 0.0 && leaf_decline_moment(phase) < self.lost
    }

    /// How far the leaf with this phase has withered, 0 (its colour) to 1 (the dry leaf colour).
    pub(crate) fn withered(&self, phase: f32) -> f32 {
        if self.withering == 0.0 {
            return 0.0;
        }
        let x = phase * 2.83 + 0.37;
        (self.withering + 0.8 * ((x - x.floor()) - 0.5)).clamp(0.0, 1.0)
    }
}

/// Put each tree's condition in the [`MeshTag`] of its meshes: when the component is added or
/// changed, and when the tree's children are (a rebuild, a new level of detail, a pool's spawn).
/// The bits below the condition are left as they are (a pooled look's glide tag). A tree that
/// lost the component, or was set healthy, is cleared. Only trees that have the component are
/// visited, so a forest of healthy trees pays nothing; a tree whose meshes are tagged by what
/// spawned them ([`tag_for`]) is found already right.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_condition_tags(
    mut commands: Commands,
    ill: Query<(&TreeCondition, &Children), Or<(Changed<TreeCondition>, Changed<Children>)>>,
    mut removed: RemovedComponents<TreeCondition>,
    standing: Query<&Children>,
    has: Query<(), With<TreeCondition>>,
    mut meshes: Query<Option<&mut MeshTag>, With<Mesh3d>>,
) {
    // The component taken off first, so a removal and an insertion in one frame end at the insertion.
    for tree in removed.read() {
        if has.contains(tree) {
            continue;
        }
        let Ok(children) = standing.get(tree) else { continue };
        for child in children.iter() {
            if let Ok(Some(mut tag)) = meshes.get_mut(child)
                && tag.value & !TAG_LOOK_MASK != 0
            {
                tag.value &= TAG_LOOK_MASK;
            }
        }
    }
    for (condition, children) in &ill {
        let bits = condition.tag_bits();
        for child in children.iter() {
            match meshes.get_mut(child) {
                Ok(Some(mut tag)) => {
                    let value = (tag.value & TAG_LOOK_MASK) | bits;
                    if tag.value != value {
                        tag.value = value;
                    }
                }
                Ok(None) if bits != 0 => {
                    commands.entity(child).insert(MeshTag::new(bits));
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FellTree, FloraIndex, IndexedTree, LodSettings, PoolColors, PooledLook, PooledTree, ProceduralTree, RecolorTree, TreeLod, TreePool,
        TreePreset,
        build_pool,
    };
    use crate::felling::Felled;
    use crate::shading::FloraMaterial;
    use bevy::app::App;
    use bevy::asset::{AssetApp, AssetId, AssetPlugin, Assets};
    use bevy::color::Color;
    use bevy::math::Vec2;
    use bevy::mesh::Mesh;
    use bevy::pbr::{MeshMaterial3d, StandardMaterial};
    use bevy::time::TimeUpdateStrategy;
    use bevy::transform::components::Transform;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default(), bevy::transform::TransformPlugin));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(crate::FloraPlugin);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 30.0)));
        app
    }

    /// A pool of two variants of `preset`, grown, in the app's assets.
    fn pool(app: &mut App, preset: TreePreset) -> bevy::asset::Handle<TreePool> {
        let built = build_pool(&preset.params(), 2, f32::INFINITY, &LodSettings::default(), &mut app.world_mut().resource_mut::<Assets<Mesh>>());
        app.world_mut().resource_mut::<Assets<TreePool>>().add(built)
    }

    fn condition_of(app: &App, tree: Entity) -> Option<TreeCondition> {
        app.world().resource::<FloraIndex>().condition_of(tree)
    }

    fn settle(app: &mut App) {
        app.update();
        app.update();
    }

    #[test]
    fn a_condition_set_on_a_tree_reads_back_through_the_index_as_set_changed_and_cleared() {
        let mut app = app();
        let oak = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), Transform::from_xyz(10.0, 0.0, 0.0))).id();
        let birch = app.world_mut().spawn((ProceduralTree(TreePreset::Birch.params()), Transform::from_xyz(-10.0, 0.0, 0.0))).id();
        let handle = pool(&mut app, TreePreset::Pine);
        let pine = app.world_mut().spawn((PooledTree { pool: handle, variant: 1 }, Transform::from_xyz(0.0, 0.0, 20.0))).id();
        settle(&mut app);
        for tree in [oak, birch, pine] {
            assert_eq!(condition_of(&app, tree), Some(TreeCondition::Healthy), "a tree with no condition is healthy");
        }
        let before = *app.world().resource::<FloraIndex>().get(oak).unwrap();

        // Set on a procedural tree and on a pooled one; the third is left alone.
        app.world_mut().entity_mut(oak).insert(TreeCondition::Sick);
        app.world_mut().entity_mut(pine).insert(TreeCondition::Dying);
        settle(&mut app);
        assert_eq!(condition_of(&app, oak), Some(TreeCondition::Sick));
        assert_eq!(condition_of(&app, pine), Some(TreeCondition::Dying));
        assert_eq!(condition_of(&app, birch), Some(TreeCondition::Healthy));
        let index = app.world().resource::<FloraIndex>();
        assert_eq!(index.get(pine).unwrap().condition, TreeCondition::Dying);
        // The condition is the only thing it changed about the oak.
        assert_eq!(IndexedTree { condition: TreeCondition::Healthy, ..*index.get(oak).unwrap() }, before);
        // The queries hand back the condition with the tree.
        assert_eq!(index.nearest(Vec2::new(0.5, 20.0), 2.0).map(|t| (t.entity, t.condition)), Some((pine, TreeCondition::Dying)));
        let ill: Vec<Entity> = index.iter().filter(|t| t.condition >= TreeCondition::Sick).map(|t| t.entity).collect();
        assert_eq!(ill.len(), 2);
        assert!(ill.contains(&oak) && ill.contains(&pine));
        let near_the_oak: Vec<_> = index.within(Vec2::new(10.0, 0.0), 3.0).map(|t| (t.entity, t.condition)).collect();
        assert_eq!(near_the_oak, vec![(oak, TreeCondition::Sick)]);

        // Moved and regrown from another seed, its entry is made again: it is still sick.
        app.world_mut().get_mut::<Transform>(oak).unwrap().translation.x = 12.0;
        app.world_mut().get_mut::<ProceduralTree>(oak).unwrap().0.seed = 9;
        settle(&mut app);
        let moved = *app.world().resource::<FloraIndex>().get(oak).unwrap();
        assert!((moved.base.x - 12.0).abs() < 1e-3, "the oak stands at x = {}", moved.base.x);
        assert_eq!(moved.condition, TreeCondition::Sick, "a moved and rebuilt tree");

        // Changed, then cleared by inserting Healthy, and by taking the component off.
        app.world_mut().entity_mut(oak).insert(TreeCondition::Dying);
        app.world_mut().entity_mut(pine).insert(TreeCondition::Healthy);
        settle(&mut app);
        assert_eq!(condition_of(&app, oak), Some(TreeCondition::Dying));
        assert_eq!(condition_of(&app, pine), Some(TreeCondition::Healthy));
        app.world_mut().entity_mut(oak).remove::<TreeCondition>();
        settle(&mut app);
        assert_eq!(condition_of(&app, oak), Some(TreeCondition::Healthy), "no component, a healthy tree");

        // Taken off and put back in one frame, it ends at what was put back.
        app.world_mut().entity_mut(oak).insert(TreeCondition::Sick);
        settle(&mut app);
        app.world_mut().entity_mut(oak).remove::<TreeCondition>();
        app.world_mut().entity_mut(oak).insert(TreeCondition::Dying);
        settle(&mut app);
        assert_eq!(condition_of(&app, oak), Some(TreeCondition::Dying));

        // A despawned tree is gone with its condition.
        app.world_mut().despawn(oak);
        settle(&mut app);
        assert_eq!(condition_of(&app, oak), None);
    }

    #[test]
    fn a_pooled_trees_condition_is_its_own_and_survives_what_the_pool_does() {
        let mut app = app();
        let handle = pool(&mut app, TreePreset::Pine);
        let spawn = |app: &mut App, x: f32| {
            app.world_mut().spawn((PooledTree { pool: handle.clone(), variant: 0 }, Transform::from_xyz(x, 0.0, 0.0))).id()
        };
        let (marked, neighbour) = (spawn(&mut app, 0.0), spawn(&mut app, 20.0));
        app.world_mut().entity_mut(marked).insert(TreeCondition::Dying);
        settle(&mut app);
        // Per instance: the pool's other tree, from the same variant, is untouched.
        assert_eq!(condition_of(&app, marked), Some(TreeCondition::Dying));
        assert_eq!(condition_of(&app, neighbour), Some(TreeCondition::Healthy));

        // A change of variant re-indexes the tree; it is still dying.
        app.world_mut().get_mut::<PooledTree>(marked).unwrap().variant = 1;
        settle(&mut app);
        assert_eq!(condition_of(&app, marked), Some(TreeCondition::Dying), "after a change of variant");

        // The pool reloads: every tree of it is indexed again; the condition is read again.
        let _ = app.world_mut().resource_mut::<Assets<TreePool>>().get_mut(&handle);
        settle(&mut app);
        assert_eq!(condition_of(&app, marked), Some(TreeCondition::Dying), "after the pool was modified");
        assert_eq!(condition_of(&app, neighbour), Some(TreeCondition::Healthy));

        // Taken out of its pool and put back (a game's own recycle): out of the index in between, and
        // the condition on the entity all the while.
        let pooled = app.world_mut().entity_mut(marked).take::<PooledTree>().unwrap();
        settle(&mut app);
        assert_eq!(condition_of(&app, marked), None);
        assert_eq!(app.world().get::<TreeCondition>(marked), Some(&TreeCondition::Dying));
        app.world_mut().entity_mut(marked).insert(pooled);
        settle(&mut app);
        assert_eq!(condition_of(&app, marked), Some(TreeCondition::Dying), "after it was put back");
    }

    #[test]
    fn a_condition_set_before_the_tree_is_indexed_applies_when_it_is() {
        let mut app = app();
        // A pool that does not exist yet: its handle is reserved, the trees that use it wait.
        let handle = app.world().resource::<Assets<TreePool>>().reserve_handle();
        let tree = app
            .world_mut()
            .spawn((PooledTree { pool: handle.clone(), variant: 0 }, TreeCondition::Sick, Transform::from_xyz(4.0, 0.0, 4.0)))
            .id();
        settle(&mut app);
        assert_eq!(condition_of(&app, tree), None, "no pool, no trunk to index");
        // Changed while it waits: the change is not dropped.
        app.world_mut().entity_mut(tree).insert(TreeCondition::Dying);
        settle(&mut app);
        assert_eq!(condition_of(&app, tree), None);

        let built = build_pool(&TreePreset::Pine.params(), 1, f32::INFINITY, &LodSettings::default(), &mut app.world_mut().resource_mut::<Assets<Mesh>>());
        app.world_mut().resource_mut::<Assets<TreePool>>().insert(handle.id(), built).unwrap();
        settle(&mut app);
        assert_eq!(condition_of(&app, tree), Some(TreeCondition::Dying), "it shows what was last set");
    }

    #[test]
    fn a_felled_trees_condition_does_not_panic_and_stays_on_its_entity() {
        let mut app = app();
        let handle = pool(&mut app, TreePreset::Pine);
        let oak = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeCondition::Sick, Transform::default())).id();
        let pine = app
            .world_mut()
            .spawn((PooledTree { pool: handle, variant: 0 }, TreeCondition::Dying, Transform::from_xyz(30.0, 0.0, 0.0)))
            .id();
        settle(&mut app);
        assert_eq!((condition_of(&app, oak), condition_of(&app, pine)), (Some(TreeCondition::Sick), Some(TreeCondition::Dying)));

        for tree in [oak, pine] {
            app.world_mut().entity_mut(tree).insert(FellTree { speed: 6.0, ..Default::default() });
        }
        // Set while the tree falls, in the frame it is cut, and at rest: none of it may panic.
        for frame in 0..400 {
            if frame == 3 {
                app.world_mut().entity_mut(oak).insert(TreeCondition::Dying);
            }
            app.update();
        }
        for tree in [oak, pine] {
            assert!(app.world().get::<Felled>(tree).is_some_and(|f| f.at_rest), "the tree did not come to rest");
            assert_eq!(condition_of(&app, tree), None, "a felled tree is not in the index");
        }
        // What it was when it was cut stays on the entity, a pooled tree's included (it left its pool).
        assert_eq!(app.world().get::<TreeCondition>(oak), Some(&TreeCondition::Dying));
        assert_eq!(app.world().get::<TreeCondition>(pine), Some(&TreeCondition::Dying));
        // Set, changed and removed on the lying tree.
        app.world_mut().entity_mut(oak).insert(TreeCondition::Healthy);
        app.world_mut().entity_mut(pine).remove::<TreeCondition>();
        settle(&mut app);
        assert_eq!(app.world().get::<TreeCondition>(oak), Some(&TreeCondition::Healthy));
        assert!(app.world().get::<TreeCondition>(pine).is_none());
        assert_eq!(app.world().resource::<FloraIndex>().len(), 0);
    }

    #[test]
    fn a_recolour_and_a_condition_in_one_frame_both_apply_and_equal_looks_still_share_a_material() {
        const BLUE: PoolColors =
            PoolColors { wood: Color::srgb(0.10, 0.20, 0.60), leaves: Color::srgb(0.30, 0.55, 0.95), unlit: false };
        // Two oaks recoloured alike; with `condition` the first is also marked in the same frame.
        // Returns the app and the two trees.
        let run = |condition: bool| {
            let mut app = app();
            app.insert_resource(crate::FloraMaterialSupport);
            let trees: Vec<Entity> = (0..2)
                .map(|i| {
                    let at = Transform::from_xyz(15.0 * i as f32, 0.0, 0.0);
                    app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeLod::default(), at)).id()
                })
                .collect();
            settle(&mut app);
            for &tree in &trees {
                app.world_mut().write_message(RecolorTree { tree, colors: BLUE });
            }
            if condition {
                app.world_mut().entity_mut(trees[0]).insert(TreeCondition::Dying);
            }
            settle(&mut app);
            (app, trees)
        };
        let materials = |app: &App, tree: Entity| -> Vec<(AssetId<FloraMaterial>, Color)> {
            let children = app.world().get::<bevy::ecs::hierarchy::Children>(tree).expect("a built tree has children");
            let assets = app.world().resource::<Assets<FloraMaterial>>();
            let mut found = Vec::new();
            for child in children.iter() {
                if let Some(handle) = app.world().get::<MeshMaterial3d<FloraMaterial>>(child) {
                    let material = assets.get(&handle.0).expect("a live material");
                    let wanted = if material.extension.leaf { BLUE.leaves } else { BLUE.wood };
                    found.push((handle.0.id(), wanted));
                    assert_eq!(material.base.base_color, wanted, "the recolour applied (leaf: {})", material.extension.leaf);
                }
            }
            found
        };

        let (control, _) = run(false);
        let (app, trees) = run(true);
        let (marked, other) = (materials(&app, trees[0]), materials(&app, trees[1]));
        assert!(marked.len() >= 6, "every level of detail of the marked tree is recoloured");
        assert_eq!(condition_of(&app, trees[0]), Some(TreeCondition::Dying));
        assert_eq!(condition_of(&app, trees[1]), Some(TreeCondition::Healthy));
        // The two trees still hold the very same materials, and the world holds no more of them than
        // the recolour alone makes.
        assert_eq!(marked, other, "trees of equal colours share their materials, marked or not");
        let live = |app: &App| app.world().resource::<Assets<FloraMaterial>>().len();
        assert_eq!(live(&app), live(&control), "a condition made no material of its own");
    }

    /// The tags of every mesh a tree is drawn with (a mesh with none counts 0).
    fn tags(app: &App, tree: Entity) -> Vec<u32> {
        let world = app.world();
        let Some(children) = world.get::<Children>(tree) else { return Vec::new() };
        children
            .iter()
            .filter(|&child| world.get::<Mesh3d>(child).is_some())
            .map(|child| world.get::<MeshTag>(child).map_or(0, |tag| tag.value))
            .collect()
    }

    #[test]
    fn the_meshes_of_a_tree_carry_its_condition_as_set_changed_rebuilt_and_cleared() {
        let mut app = app();
        let oak = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeLod::default())).id();
        let other = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), Transform::from_xyz(20.0, 0.0, 0.0))).id();
        settle(&mut app);
        let all = |app: &App, tree: Entity, bits: u32| {
            let found = tags(app, tree);
            !found.is_empty() && found.iter().all(|&tag| tag == bits)
        };
        assert!(tags(&app, oak).len() >= 6, "wood and leaves at every level of detail");
        assert!(all(&app, oak, 0) && all(&app, other, 0), "a tree with no condition has untagged meshes");

        app.world_mut().entity_mut(oak).insert(TreeCondition::Sick);
        settle(&mut app);
        assert!(all(&app, oak, TreeCondition::Sick.tag_bits()));
        assert!(all(&app, other, 0), "no other tree is touched");

        app.world_mut().entity_mut(oak).insert(TreeCondition::Dying);
        settle(&mut app);
        assert!(all(&app, oak, TreeCondition::Dying.tag_bits()));

        // Regrown from another seed, its meshes are made again: they are still dying.
        app.world_mut().get_mut::<ProceduralTree>(oak).unwrap().0.seed = 9;
        settle(&mut app);
        assert!(all(&app, oak, TreeCondition::Dying.tag_bits()), "{:?}", tags(&app, oak));

        // Healthy, and the component taken off, both clear it; taken off and put back in one frame
        // ends at the put back.
        app.world_mut().entity_mut(oak).insert(TreeCondition::Healthy);
        settle(&mut app);
        assert!(all(&app, oak, 0));
        app.world_mut().entity_mut(oak).insert(TreeCondition::Dying);
        settle(&mut app);
        app.world_mut().entity_mut(oak).remove::<TreeCondition>();
        settle(&mut app);
        assert!(all(&app, oak, 0));
        app.world_mut().entity_mut(oak).insert(TreeCondition::Sick);
        settle(&mut app);
        app.world_mut().entity_mut(oak).remove::<TreeCondition>().insert(TreeCondition::Dying);
        settle(&mut app);
        assert!(all(&app, oak, TreeCondition::Dying.tag_bits()));
    }

    #[test]
    fn a_pooled_trees_meshes_keep_their_looks_tag_beside_the_condition() {
        const RED: PoolColors = PoolColors { wood: Color::srgb(0.3, 0.1, 0.1), leaves: Color::srgb(0.8, 0.1, 0.1), unlit: false };
        let mut app = app();
        let mut built = build_pool(&TreePreset::Pine.params(), 2, f32::INFINITY, &LodSettings::default(), &mut app.world_mut().resource_mut::<Assets<Mesh>>());
        built.add_look(RED);
        let handle = app.world_mut().resource_mut::<Assets<TreePool>>().add(built);
        let (a, b) = (PooledTree { pool: handle.clone(), variant: 0 }, PooledTree { pool: handle, variant: 1 });
        // Sick from the start (the tag is made with the meshes), and not at all.
        let sick = app.world_mut().spawn((a, TreeCondition::Sick)).id();
        let well = app.world_mut().spawn((b, Transform::from_xyz(9.0, 0.0, 0.0))).id();
        settle(&mut app);
        let (look, bits) = (|tag: u32| tag & TAG_LOOK_MASK, |tag: u32| tag & !TAG_LOOK_MASK);
        let first = tags(&app, sick);
        assert!(first.len() >= 6 && first.iter().all(|&tag| bits(tag) == TreeCondition::Sick.tag_bits()), "{first:?}");
        let glide = look(first[0]);
        assert!(glide != 0 && first.iter().all(|&tag| look(tag) == glide), "the look's tag is still there: {first:?}");
        assert!(tags(&app, well).iter().all(|&tag| bits(tag) == 0));

        // Another look: the tag of that look, still sick.
        app.world_mut().entity_mut(sick).insert(PooledLook(1));
        settle(&mut app);
        let moved = tags(&app, sick);
        let glide_now = look(moved[0]);
        assert!(glide_now != 0 && glide_now != glide, "the other look's tag: {moved:?}");
        assert!(moved.iter().all(|&tag| look(tag) == glide_now && bits(tag) == TreeCondition::Sick.tag_bits()), "{moved:?}");

        // Worse, then well: the look's tag stays.
        app.world_mut().entity_mut(sick).insert(TreeCondition::Dying);
        settle(&mut app);
        assert!(tags(&app, sick).iter().all(|&tag| look(tag) == glide_now && bits(tag) == TreeCondition::Dying.tag_bits()));
        app.world_mut().entity_mut(sick).remove::<TreeCondition>();
        settle(&mut app);
        assert!(tags(&app, sick).iter().all(|&tag| tag == glide_now), "{:?}", tags(&app, sick));
    }

    #[test]
    fn a_declined_leaf_is_lost_by_its_own_draw_and_withers_by_its_class() {
        let healthy = Decline::new(None, false);
        let sick = Decline::new(Some(&TreeCondition::Sick), false);
        let dying = Decline::new(Some(&TreeCondition::Dying), false);
        let hull = Decline::new(Some(&TreeCondition::Dying), true);
        let phases: Vec<f32> = (0..4000).map(|k| k as f32 * 0.00157).collect();
        let lost = |decline: &Decline| phases.iter().filter(|&&p| decline.has_lost(p)).count() as f32 / phases.len() as f32;
        assert_eq!(lost(&healthy), 0.0);
        assert!((lost(&sick) - TreeCondition::Sick.defoliation()).abs() < 0.03, "{}", lost(&sick));
        assert!((lost(&dying) - TreeCondition::Dying.defoliation()).abs() < 0.03, "{}", lost(&dying));
        assert_eq!(lost(&hull), 0.0, "a hull crown keeps its facets");
        // The ones a sick tree loses are among those a dying one has lost.
        assert!(phases.iter().all(|&p| !sick.has_lost(p) || dying.has_lost(p)));

        // A healthy leaf has not withered; the others have, by their class's share on average, a
        // dying tree's further than a sick one's, none past the dry colour.
        assert!(phases.iter().all(|&p| healthy.withered(p) == 0.0));
        let mean = |decline: &Decline| phases.iter().map(|&p| decline.withered(p)).sum::<f32>() / phases.len() as f32;
        assert!((mean(&sick) - TreeCondition::Sick.withering()).abs() < 0.03, "{}", mean(&sick));
        assert!(mean(&dying) > 0.85 && mean(&dying) <= 1.0, "{}", mean(&dying));
        assert!(phases.iter().all(|&p| (0.0..=1.0).contains(&sick.withered(p)) && (0.0..=1.0).contains(&dying.withered(p))));
        // Each leaf of a sick tree goes its own way: not all alike.
        let spread = phases.iter().map(|&p| sick.withered(p)).fold((1.0f32, 0.0f32), |(lo, hi), t| (lo.min(t), hi.max(t)));
        assert!(spread.1 - spread.0 > 0.6, "{spread:?}");
    }
}
