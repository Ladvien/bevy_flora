//! How well a tree is: [`TreeCondition`], a fact about one tree that a game sets and reads back.
//!
//! A game that wants a sick tree, a withering stand or a copse that dies in one season inserts
//! [`TreeCondition`] on the tree's entity (a [`ProceduralTree`](crate::ProceduralTree) or a
//! [`PooledTree`](crate::PooledTree)) and finds it again through [`FloraIndex`](crate::FloraIndex):
//! [`IndexedTree::condition`](crate::IndexedTree::condition) on every tree a query returns, or
//! [`FloraIndex::condition_of`](crate::FloraIndex::condition_of) for one entity.
//!
//! This is state, not looks: the crate draws a sick tree as it draws a healthy one. A game that wants
//! it to look different recolours it ([`RecolorTree`](crate::RecolorTree), or a
//! [`PooledLook`](crate::PooledLook) of a pool's own looks).
//!
//! Pure CPU, like `shape.rs`: it derives `Component` and nothing here touches rendering.

use bevy::ecs::prelude::*;
use bevy::reflect::Reflect;

/// The condition of a tree, in the three classes of a forest-health survey's crown assessment
/// (`docs/research/tree-condition.md`): a healthy crown, one in decline, one close to death. Ordered
/// from well to ill, so `condition >= TreeCondition::Sick` means "not healthy".
///
/// **Insert it on the tree's entity**, `commands.entity(tree).insert(TreeCondition::Dying)`, and
/// [`FloraIndex`](crate::FloraIndex) shows it from the next [`PostUpdate`](bevy::app::PostUpdate).
/// Insert [`TreeCondition::Healthy`] or remove the component to clear it: a tree without one is
/// healthy, so a forest of a hundred thousand pays nothing for the trees that are well. To react to a
/// change, query `Changed<TreeCondition>`.
///
/// The condition belongs to the **tree's entity**, not to a [`TreePool`](crate::TreePool) its meshes
/// come from: a [`PooledTree`](crate::PooledTree) that changes variant or pool, is taken off the entity
/// and put back, or whose pool reloads, keeps it. A tree set before it is indexed (a pooled tree whose
/// pool has not loaded yet) shows it as soon as it is. Despawning the tree takes it with it.
///
/// A felled tree leaves the index ([`Felled`](crate::Felled)): [`FloraIndex::condition_of`] answers
/// `None` for it, and the component stays on its entity as it was when the tree was cut, for a
/// `Query<&TreeCondition>` to read. Setting one on a felled tree does nothing else. There is no
/// `Dead` condition: a dead tree is the game's to fell ([`FellTree`](crate::FellTree)), replace, or
/// leave standing.
///
/// [`FloraIndex::condition_of`]: crate::FloraIndex::condition_of
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
#[reflect(Component)]
pub enum TreeCondition {
    /// A full crown: the state of a tree with no `TreeCondition`.
    #[default]
    Healthy,
    /// A crown in decline: thinning, off its colour. The tree is still standing and may recover.
    Sick,
    /// A crown close to death: mostly bare or browned.
    Dying,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FellTree, FloraIndex, IndexedTree, LodSettings, PoolColors, PooledTree, ProceduralTree, RecolorTree, TreeLod, TreePool, TreePreset,
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
}
