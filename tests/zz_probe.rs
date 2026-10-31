//! Throwaway: what a condition glide costs the frame at forest scale (deleted before commit).
use bevy::asset::{AssetApp, AssetPlugin, Assets};
use bevy::mesh::Mesh;
use bevy::pbr::StandardMaterial;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_flora::{ConditionGlide, LodSettings, PooledTree, TreeCondition, TreePool, TreePreset, build_pool};
use std::time::{Duration, Instant};

fn run(app: &mut App, frames: u32) -> (f64, f64) {
    let mut worst = 0.0f64;
    let start = Instant::now();
    for _ in 0..frames {
        let t = Instant::now();
        app.update();
        worst = worst.max(t.elapsed().as_secs_f64() * 1000.0);
    }
    (start.elapsed().as_secs_f64() * 1000.0 / frames as f64, worst)
}

#[test]
fn glide_cost_at_forest_scale() {
    let n: u32 = std::env::var("PROBE_TREES").ok().and_then(|v| v.parse().ok()).unwrap_or(10_000);
    let mut app = App::new();
    app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default(), bevy::transform::TransformPlugin));
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.add_plugins(bevy_flora::FloraPlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 60.0)));
    let built = build_pool(&TreePreset::Oak.params(), 4, f32::INFINITY, &LodSettings::default(), &mut app.world_mut().resource_mut::<Assets<Mesh>>());
    let handle = app.world_mut().resource_mut::<Assets<TreePool>>().add(built);
    let side = (n as f32).sqrt().ceil() as u32;
    let trees: Vec<Entity> = (0..n)
        .map(|i| {
            app.world_mut()
                .spawn((PooledTree { pool: handle.clone(), variant: i % 4 }, Transform::from_xyz((i % side) as f32 * 9.0, 0.0, (i / side) as f32 * 9.0)))
                .id()
        })
        .collect();
    run(&mut app, 5);
    let (a, aw) = run(&mut app, 60);
    println!("healthy, no condition:          {a:.3} ms/frame, worst {aw:.3}");

    app.world_mut().insert_resource(ConditionGlide { seconds: 0.0 });
    for &t in &trees {
        app.world_mut().entity_mut(t).insert(TreeCondition::Sick);
    }
    let (set, setw) = run(&mut app, 1);
    println!("the frame every tree is set (0 s): {set:.3} ms, worst {setw:.3}");
    run(&mut app, 5);
    let (b, bw) = run(&mut app, 60);
    println!("all settled sick:                {b:.3} ms/frame, worst {bw:.3}");

    app.world_mut().insert_resource(ConditionGlide { seconds: 30.0 });
    for &t in &trees {
        app.world_mut().entity_mut(t).insert(TreeCondition::Dying);
    }
    let (start, startw) = run(&mut app, 1);
    println!("the frame every tree starts a glide: {start:.3} ms, worst {startw:.3}");
    let (c, cw) = run(&mut app, 300);
    println!("all gliding (first 5 s), mean:     {c:.3} ms/frame, worst frame {cw:.3}");
    let (d, dw) = run(&mut app, 600);
    println!("all gliding (next 10 s), mean:     {d:.3} ms/frame, worst frame {dw:.3}");
    let (e, ew) = run(&mut app, 1200);
    println!("all gliding (to the end), mean:    {e:.3} ms/frame, worst frame {ew:.3}");
    let (f, fw) = run(&mut app, 60);
    println!("arrived (settled dying):           {f:.3} ms/frame, worst {fw:.3}");
}
