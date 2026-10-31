//! Throwaway smoke probe for TreeCondition at forest scale (deleted before committing).
use std::time::Instant;

use bevy::app::App;
use bevy::asset::{AssetApp, AssetPlugin, Assets};
use bevy::ecs::prelude::*;
use bevy::math::Vec2;
use bevy::mesh::Mesh;
use bevy::pbr::StandardMaterial;
use bevy::time::TimeUpdateStrategy;
use bevy::transform::components::Transform;
use bevy_flora::{FellTree, FloraIndex, LodSettings, PooledTree, TreeCondition, TreePool, TreePreset, build_pool};

fn main() {
    let mut app = App::new();
    app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default(), bevy::transform::TransformPlugin));
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.add_plugins(bevy_flora::FloraPlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 30.0)));

    let built = build_pool(&TreePreset::Oak.params(), 4, f32::INFINITY, &LodSettings::default(), &mut app.world_mut().resource_mut::<Assets<Mesh>>());
    let pool = app.world_mut().resource_mut::<Assets<TreePool>>().add(built);
    let side = 100;
    let mut trees = Vec::new();
    for x in 0..side {
        for z in 0..side {
            let at = Transform::from_xyz(x as f32 * 6.0, 0.0, z as f32 * 6.0);
            trees.push(app.world_mut().spawn((PooledTree { pool: pool.clone(), variant: (x + z) % 4 }, at)).id());
        }
    }
    let t = Instant::now();
    app.update();
    println!("spawn + first update, {} trees: {:.1} ms", trees.len(), t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    app.update();
    println!("indexed {}: second update {:.1} ms", app.world().resource::<FloraIndex>().len(), t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    for _ in 0..10 {
        app.update();
    }
    println!("idle update: {:.2} ms", t.elapsed().as_secs_f64() * 1e2);

    // A blight takes the stand within 150 m of the corner: found through the index, set by insert.
    let centre = Vec2::new(150.0, 150.0);
    let hit: Vec<Entity> = app.world().resource::<FloraIndex>().within(centre, 150.0).map(|t| t.entity).collect();
    let t = Instant::now();
    for &tree in &hit {
        app.world_mut().entity_mut(tree).insert(TreeCondition::Dying);
    }
    println!("inserted {} conditions: {:.2} ms", hit.len(), t.elapsed().as_secs_f64() * 1e3);
    let t = Instant::now();
    app.update();
    println!("update after {} changes: {:.2} ms", hit.len(), t.elapsed().as_secs_f64() * 1e3);
    let index = app.world().resource::<FloraIndex>();
    let dying = index.iter().filter(|t| t.condition == TreeCondition::Dying).count();
    let outside_dying = index.iter().filter(|t| t.condition == TreeCondition::Dying && Vec2::new(t.base.x, t.base.z).distance(centre) > 150.0 + t.base_radius + 0.01).count();
    println!("dying in the index: {dying} (set {}), outside the radius: {outside_dying}", hit.len());
    let t = Instant::now();
    let count = index.iter().filter(|t| t.condition >= TreeCondition::Sick).count();
    println!("a scan for the ill among {}: {count} in {:.2} ms", index.len(), t.elapsed().as_secs_f64() * 1e3);

    // One of them is felled: it leaves the index, keeps the component, nothing panics.
    let victim = hit[hit.len() / 2];
    app.world_mut().entity_mut(victim).insert(FellTree { speed: 6.0, ..Default::default() });
    for _ in 0..300 {
        app.update();
    }
    app.world_mut().entity_mut(victim).insert(TreeCondition::Sick);
    app.update();
    println!(
        "felled: in index {:?}, component {:?}, index holds {}",
        app.world().resource::<FloraIndex>().condition_of(victim),
        app.world().get::<TreeCondition>(victim),
        app.world().resource::<FloraIndex>().len()
    );
}
