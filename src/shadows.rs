//! Shadows near the camera only: [`FloraShadows`].

use bevy::camera::{Camera, Camera3d, Projection};
use bevy::ecs::prelude::*;
use bevy::light::NotShadowCaster;
use bevy::math::{Vec2, Vec3Swizzles};
use bevy::platform::collections::HashSet;
use bevy::reflect::Reflect;
use bevy::transform::components::GlobalTransform;

use crate::felling::Felled;
use crate::ground::FloraGround;
use crate::index::FloraIndex;
use crate::plugin::ProceduralTree;
use crate::pool::{PooledTree, ground_distance};
use crate::units::FloraUnits;

/// Only the standing trees near the main camera cast shadows: a tree, pooled or a
/// [`ProceduralTree`], whose trunk stands farther than `radius` metres from the camera's place on
/// the ground gets [`NotShadowCaster`] (every mesh of it, impostors included), and loses it when the
/// camera comes near again. The place is under a perspective camera; under an orthographic one it
/// is where the view meets the ground ([`FloraGround`] when there is one), the middle of what it
/// shows. The main camera is the active `Camera3d` with the lowest `order`.
///
/// It is kept through [`FloraIndex`] as the camera moves (looked at again once its place has moved
/// a hundredth of the radius) and as trees are planted, moved or felled, so a frame costs the
/// trees near the edge, not the forest; the first frame looks at every tree. A tree the game quiets
/// itself (`NotShadowCaster` before the range reached it) is left alone; a tree that is felled
/// casts again, and so does every tree when the resource is removed.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct FloraShadows {
    /// How far from the camera's place trees still cast, across the ground, in metres, times
    /// [`FloraUnits`] in the world (like [`LodDistances`](crate::LodDistances)).
    pub radius: f32,
}

/// On a tree [`cut_far_shadows`] quieted: the [`NotShadowCaster`] beside it is the range's to take
/// back.
#[derive(Component)]
pub(crate) struct ShadowCut;

/// Where and how wide the range was last applied.
#[derive(Default)]
pub(crate) struct Applied {
    centre: Option<Vec2>,
    radius: f32,
}

/// The share of the radius the camera's place moves before the trees are looked at again.
const STEP: f32 = 0.01;

/// Keep [`FloraShadows`]: quiet the casting trees outside the radius, give the shadow back to the
/// ones it quieted that are inside, whenever the camera's place moved, the radius changed or the
/// index did. The casting trees left after a pass are the near ones, so a pass costs those and
/// the trees new since.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn cut_far_shadows(
    mut commands: Commands,
    shadows: Option<Res<FloraShadows>>,
    index: Option<Res<FloraIndex>>,
    ground: Option<Res<FloraGround>>,
    cameras: Query<(&Camera, &Projection, &GlobalTransform), With<Camera3d>>,
    casting: Query<Entity, (Or<(With<PooledTree>, With<ProceduralTree>)>, Without<NotShadowCaster>, Without<Felled>)>,
    cut: Query<Entity, With<ShadowCut>>,
    felled: Query<Entity, (With<ShadowCut>, Added<Felled>)>,
    units: Res<FloraUnits>,
    mut applied: Local<Applied>,
) {
    for tree in &felled {
        commands.entity(tree).try_remove::<(NotShadowCaster, ShadowCut)>();
    }
    let (Some(shadows), Some(index)) = (shadows, index) else {
        // No range (any more): every shadow it took is given back.
        if applied.centre.take().is_some() {
            for tree in &cut {
                commands.entity(tree).try_remove::<(NotShadowCaster, ShadowCut)>();
            }
        }
        return;
    };
    let main = cameras.iter().filter(|(camera, ..)| camera.is_active).min_by_key(|(camera, ..)| camera.order);
    let Some((_, projection, at)) = main else { return };
    let eye = at.translation();
    let centre = match projection {
        Projection::Orthographic(_) => ground_distance(at, ground.as_deref()).map_or(eye, |d| eye + at.forward().as_vec3() * d),
        _ => eye,
    }
    .xz();
    // Metres to world units; `max` drops a NaN radius to 0.
    let radius = shadows.radius.max(0.0) * units.units_per_metre;
    let moved = applied.centre.is_none_or(|was| was.distance(centre) > radius * STEP);
    if !moved && radius == applied.radius && !index.is_changed() {
        return;
    }
    if moved {
        applied.centre = Some(centre);
    }
    applied.radius = radius;
    let centre = applied.centre.unwrap_or(centre);
    // The trees within the radius: by the cells it covers, or past as many cells as there are
    // trees, by every tree.
    let side = (2.0 * radius / FloraIndex::CELL).ceil() + 1.0;
    let near: HashSet<Entity> = if side * side > index.len() as f32 {
        index.iter().filter(|tree| tree.distance_to(centre) <= radius).map(|tree| tree.entity).collect()
    } else {
        index.within(centre, radius).map(|tree| tree.entity).collect()
    };
    for &tree in &near {
        if cut.contains(tree) {
            commands.entity(tree).try_remove::<(NotShadowCaster, ShadowCut)>();
        }
    }
    for tree in &casting {
        if !near.contains(&tree) {
            commands.entity(tree).try_insert((NotShadowCaster, ShadowCut));
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::asset::{AssetApp, AssetPlugin, Assets, Handle};
    use bevy::camera::OrthographicProjection;
    use bevy::math::Vec3;
    use bevy::mesh::{Mesh, Mesh3d};
    use bevy::pbr::StandardMaterial;
    use bevy::transform::components::Transform;

    use super::*;
    use crate::lod::{LodDistances, LodSettings};
    use crate::plugin::FloraPlugin;
    use crate::pool::{TreePool, build_pool};
    use crate::presets::TreePreset;

    fn app() -> (App, Handle<TreePool>) {
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default(), bevy::transform::TransformPlugin));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(FloraPlugin);
        let lod = LodSettings { distances: LodDistances { impostor: f32::INFINITY, ..Default::default() }, ..Default::default() };
        let pool = {
            let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
            build_pool(&TreePreset::Oak.params(), 1, f32::INFINITY, &lod, &mut meshes)
        };
        let pool = app.world_mut().resource_mut::<Assets<TreePool>>().add(pool);
        (app, pool)
    }

    /// Which of `trees` cast: no `NotShadowCaster` on the tree nor on any of its meshes.
    fn casting(app: &App, trees: &[Entity]) -> Vec<bool> {
        trees
            .iter()
            .map(|&tree| {
                let world = app.world();
                let meshes: Vec<Entity> =
                    world.get::<Children>(tree).into_iter().flat_map(|c| c.iter()).filter(|c| world.get::<Mesh3d>(*c).is_some()).collect();
                assert!(!meshes.is_empty(), "the tree has its meshes");
                let quiet = meshes.iter().filter(|m| world.get::<NotShadowCaster>(**m).is_some()).count();
                assert!(quiet == 0 || quiet == meshes.len(), "its meshes agree: {quiet} of {}", meshes.len());
                world.get::<NotShadowCaster>(tree).is_none() && quiet == 0
            })
            .collect()
    }

    fn settle(app: &mut App) {
        for _ in 0..3 {
            app.update();
        }
    }

    #[test]
    fn only_trees_near_the_camera_cast_and_the_range_follows_it() {
        let (mut app, pool) = app();
        // A row of trees 20 m apart along x, and one the game quieted itself at x = 10.
        let trees: Vec<Entity> = (0..5)
            .map(|i| app.world_mut().spawn((PooledTree { pool: pool.clone(), variant: 0 }, Transform::from_xyz(i as f32 * 20.0, 0.0, 0.0))).id())
            .collect();
        let own = app.world_mut().spawn((PooledTree { pool: pool.clone(), variant: 0 }, Transform::from_xyz(10.0, 0.0, 0.0), NotShadowCaster)).id();
        let camera = app.world_mut().spawn((Camera3d::default(), Transform::from_xyz(0.0, 2.0, -5.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y))).id();
        settle(&mut app);
        assert_eq!(casting(&app, &trees), [true; 5], "no range, every tree casts");

        app.insert_resource(FloraShadows { radius: 30.0 });
        settle(&mut app);
        assert_eq!(casting(&app, &trees), [true, true, false, false, false], "within 30 m of the camera at x = 0");
        // The camera walks to the far end: the range goes with it.
        app.world_mut().entity_mut(camera).insert(Transform::from_xyz(80.0, 2.0, -5.0).looking_at(Vec3::new(80.0, 1.0, 0.0), Vec3::Y));
        settle(&mut app);
        assert_eq!(casting(&app, &trees), [false, false, false, true, true], "within 30 m of x = 80");
        assert_eq!(casting(&app, &[own]), [false], "the game's own quiet tree stays quiet");

        // An orthographic camera high over x = 20, looking down at 45°: its place is where it looks.
        let ortho = Projection::Orthographic(OrthographicProjection::default_3d());
        app.world_mut()
            .entity_mut(camera)
            .insert((ortho, Transform::from_xyz(20.0, 50.0, -50.0).looking_at(Vec3::new(20.0, 0.0, 0.0), Vec3::Y)));
        settle(&mut app);
        assert_eq!(casting(&app, &trees), [true, true, true, false, false], "within 30 m of x = 20, under the orthographic camera");

        // Without the range, every tree it quieted casts again; the game's own stays quiet.
        app.world_mut().remove_resource::<FloraShadows>();
        settle(&mut app);
        assert_eq!(casting(&app, &trees), [true; 5]);
        assert_eq!(casting(&app, &[own]), [false]);
    }

    #[test]
    fn a_tree_planted_far_away_is_quieted_and_one_planted_near_casts() {
        let (mut app, pool) = app();
        app.world_mut().spawn((Camera3d::default(), Transform::from_xyz(0.0, 2.0, 0.0)));
        app.insert_resource(FloraShadows { radius: 30.0 });
        settle(&mut app);
        let near = app.world_mut().spawn((PooledTree { pool: pool.clone(), variant: 0 }, Transform::from_xyz(5.0, 0.0, 5.0))).id();
        let far = app.world_mut().spawn((PooledTree { pool, variant: 0 }, Transform::from_xyz(-200.0, 0.0, 40.0))).id();
        settle(&mut app);
        assert_eq!(casting(&app, &[near, far]), [true, false]);
    }
}
