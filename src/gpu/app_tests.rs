//! `GpuGrowth` in a headless app: what the systems upload and when.

use bevy::app::App;
use bevy::asset::{AssetApp, AssetEvent, AssetPlugin, Assets};
use bevy::ecs::prelude::*;
use bevy::mesh::Mesh;
use bevy::pbr::StandardMaterial;
use bevy::render::storage::ShaderBuffer;

use super::state::GrowthState;
use super::{GpuGrowth, GpuGrowthSupport, GrowthMaterial};
use crate::params::TreeParams;
use crate::plugin::{FloraPlugin, Paint, ProceduralTree, TreeAge, TreeGenerated, TreeRender};
use crate::presets::TreePreset;
use crate::skeleton::generate_skeleton;
use crate::{build_tree, pipeline};

fn app(support: bool) -> App {
    let mut app = App::new();
    app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.add_plugins(FloraPlugin);
    app.insert_resource(GpuGrowthSupport(support));
    app
}

/// The node buffer a tree should hold at `age`.
fn expected_buffer(params: &TreeParams, age: f32) -> Vec<[f32; 4]> {
    let full = generate_skeleton(params);
    let mut state = GrowthState::default();
    state.update(&full, params, age);
    let mut out = vec![[0.0; 4]; state.vec4_len()];
    state.encode(&mut out);
    out
}

/// The node buffer a tree holds now, if it is painted for the GPU.
fn uploaded_buffer(app: &App, tree: Entity) -> Option<Vec<[f32; 4]>> {
    let render = app.world().get::<TreeRender>(tree)?;
    let Paint::Growth(paint) = &render.paint else { return None };
    let buffers = app.world().resource::<Assets<ShaderBuffer>>();
    Some(buffers.get(&paint.nodes)?.cast_slice::<[f32; 4]>()?.to_vec())
}

fn modified<A: bevy::asset::Asset>(app: &App) -> usize {
    app.world()
        .resource::<Messages<AssetEvent<A>>>()
        .iter_current_update_messages()
        .filter(|e| matches!(e, AssetEvent::Modified { .. }))
        .count()
}

fn is_gpu(app: &App, tree: Entity) -> bool {
    matches!(app.world().get::<TreeRender>(tree).map(|r| &r.paint), Some(Paint::Growth(_)))
}

#[test]
fn growing_on_the_gpu_writes_node_state_and_nothing_else() {
    let params = TreePreset::Oak.params();
    let mut app = app(true);
    let tree = app
        .world_mut()
        .spawn((ProceduralTree(params.clone()), TreeAge(2.0), GpuGrowth))
        .id();
    app.update();
    assert!(is_gpu(&app, tree));
    assert_eq!(app.world().get::<Children>(tree).unwrap().len(), 2);
    assert_eq!(uploaded_buffer(&app, tree).unwrap(), expected_buffer(&params, 2.0));

    let mature = build_tree(&params, f32::INFINITY);
    let mut cursor = app.world().resource::<Messages<TreeGenerated>>().get_cursor_current();
    for age in [3.0, 4.0, 5.0, 6.0, 7.5] {
        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = age;
        app.update();
        assert_eq!(uploaded_buffer(&app, tree).unwrap(), expected_buffer(&params, age), "age {age}");
        // Neither the mesh nor either material is touched: a modified asset is
        // re-prepared by the renderer, and the point is to upload only the buffer.
        assert_eq!(modified::<Mesh>(&app), 0, "age {age}: mesh");
        assert_eq!(modified::<GrowthMaterial>(&app), 0, "age {age}: material");
        let sent: Vec<_> = cursor.read(app.world().resource::<Messages<TreeGenerated>>()).copied().collect();
        assert_eq!(sent.len(), 1, "age {age}: {sent:?}");
        let progress = pipeline::build_tree(&params, age).skeleton.nodes[0].progress;
        assert!((sent[0].height - params.trunk.height * progress).abs() < 1e-4, "age {age}");
    }

    // The mesh in the asset store is the mature tree at every age.
    let world = app.world();
    let render = world.get::<TreeRender>(tree).unwrap();
    let wood = world.resource::<Assets<Mesh>>().get(render.trunk_mesh.as_ref().unwrap()).unwrap();
    assert!(wood.count_vertices() >= mature.trunk.positions.len());
    assert!(wood.attribute(super::ATTRIBUTE_GROWTH_ID).is_some());
}

#[test]
fn changing_the_parameters_rebuilds_the_mature_tree_and_the_buffer() {
    let mut params = TreePreset::Oak.params();
    let mut app = app(true);
    let tree = app
        .world_mut()
        .spawn((ProceduralTree(params.clone()), TreeAge(4.0), GpuGrowth))
        .id();
    app.update();

    params.seed = 11;
    params.trunk.height *= 1.3;
    app.world_mut().get_mut::<ProceduralTree>(tree).unwrap().0 = params.clone();
    app.update();
    assert_eq!(uploaded_buffer(&app, tree).unwrap(), expected_buffer(&params, 4.0));
    assert_eq!(modified::<Mesh>(&app), 2, "the wood and leaf meshes are replaced in place");

    // A colour change touches the materials, once, and not the mesh.
    let red = bevy::color::Color::srgb(1.0, 0.0, 0.0);
    app.world_mut().get_mut::<ProceduralTree>(tree).unwrap().0.trunk_color = red;
    app.update();
    assert_eq!(modified::<GrowthMaterial>(&app), 1);
    let world = app.world();
    let render = world.get::<TreeRender>(tree).unwrap();
    let Paint::Growth(paint) = &render.paint else { panic!("still on the GPU") };
    assert_eq!(world.resource::<Assets<GrowthMaterial>>().get(&paint.trunk).unwrap().base.base_color, red);
}

#[test]
fn a_tree_follows_its_gpu_growth_component_and_the_devices_support() {
    let params = TreePreset::Willow.params();
    let mut app = app(true);
    let tree = app.world_mut().spawn((ProceduralTree(params.clone()), TreeAge(5.0))).id();
    app.update();
    assert!(!is_gpu(&app, tree), "no GpuGrowth: the CPU path");

    app.world_mut().entity_mut(tree).insert(GpuGrowth);
    app.update();
    assert!(is_gpu(&app, tree));
    assert_eq!(app.world().get::<Children>(tree).unwrap().len(), 2, "the old children are gone");

    app.world_mut().entity_mut(tree).remove::<GpuGrowth>();
    app.update();
    assert!(!is_gpu(&app, tree));
    assert_eq!(app.world().get::<Children>(tree).unwrap().len(), 2);
    let world = app.world();
    let render = world.get::<TreeRender>(tree).unwrap();
    let uploaded = world.resource::<Assets<Mesh>>().get(render.trunk_mesh.as_ref().unwrap()).unwrap();
    assert_eq!(uploaded.count_vertices(), build_tree(&params, 5.0).trunk.positions.len());

    // A device without storage buffers keeps the tree on the CPU, and a device
    // that turns out to have them moves it over.
    app.world_mut().entity_mut(tree).insert(GpuGrowth);
    app.world_mut().insert_resource(GpuGrowthSupport(false));
    app.update();
    assert!(!is_gpu(&app, tree));
    app.world_mut().insert_resource(GpuGrowthSupport(true));
    app.update();
    assert!(is_gpu(&app, tree));
}

#[test]
fn growing_trees_reach_their_mature_state_on_the_gpu() {
    use crate::plugin::Growing;
    let params = TreePreset::Spruce.params();
    let mut app = app(true);
    let tree = app
        .world_mut()
        .spawn((ProceduralTree(params.clone()), TreeAge(0.0), Growing { rate: 40.0 }, GpuGrowth))
        .id();
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
    let mature_at = generate_skeleton(&params).mature_at(&params.growth);
    for _ in 0..4 {
        app.update();
    }
    let age = app.world().get::<TreeAge>(tree).unwrap().0;
    assert!(age > 0.0 && age <= mature_at);
    assert_eq!(uploaded_buffer(&app, tree).unwrap(), expected_buffer(&params, age));
}
