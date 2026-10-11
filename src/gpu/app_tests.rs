//! `GpuGrowth` in a headless app: what the systems upload and when.

use bevy::app::App;
use bevy::asset::{AssetApp, AssetEvent, AssetPlugin, Assets};
use bevy::ecs::prelude::*;
use bevy::mesh::{Mesh, MeshTag};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::render::storage::ShaderBuffer;

use super::render::GrowthPaint;
use super::state::GrowthState;
use super::store::GrowthStore;
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

/// The node state a tree has queued for the GPU, if it is painted for it and has not been
/// taken (nothing takes it in a headless app).
fn uploaded_buffer(app: &App, tree: Entity) -> Option<Vec<[f32; 4]>> {
    let render = app.world().get::<TreeRender>(tree)?;
    let Paint::Growth(paint) = &render.paint else { return None };
    paint.slice.queued()
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
        assert_eq!(modified::<ShaderBuffer>(&app), 0, "age {age}: buffer");
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

    // A colour change gives the tree another (shared) wood material, and touches no asset in place.
    let red = bevy::color::Color::srgb(1.0, 0.0, 0.0);
    app.world_mut().get_mut::<ProceduralTree>(tree).unwrap().0.trunk_color = red;
    let leaves = paint(&app, tree).foliage.clone();
    app.update();
    assert_eq!(modified::<GrowthMaterial>(&app), 0);
    let after = paint(&app, tree);
    assert_eq!(after.foliage, leaves, "the leaves' material is the same");
    assert_eq!(app.world().resource::<Assets<GrowthMaterial>>().get(&after.trunk).unwrap().base.base_color, red);
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

#[test]
fn the_plugin_can_be_added_before_the_asset_plugin() {
    // Plugins may be added in any order (0.3.5 did not care). GPU growth needs
    // the renderer, which is not there yet: it stays off and trees grow on the CPU.
    let mut app = App::new();
    app.add_plugins(FloraPlugin);
    app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.finish();
    app.cleanup();
    assert!(!app.world().resource::<GpuGrowthSupport>().0);

    let params = TreePreset::Oak.params();
    let tree = app
        .world_mut()
        .spawn((ProceduralTree(params.clone()), TreeAge(4.0), GpuGrowth))
        .id();
    app.update();
    assert!(app.world().get::<TreeRender>(tree).is_some());
    assert!(!is_gpu(&app, tree));
    assert_eq!(app.world().get::<Children>(tree).unwrap().len(), 2);
}

fn store(app: &App) -> GrowthStore {
    app.world().resource::<GrowthStore>().clone()
}

fn paint(app: &App, tree: Entity) -> &GrowthPaint {
    let Paint::Growth(paint) = &app.world().get::<TreeRender>(tree).expect("rendered").paint else {
        panic!("painted for the GPU")
    };
    paint
}

fn spawn(app: &mut App, params: &TreeParams, age: f32) -> Entity {
    app.world_mut().spawn((ProceduralTree(params.clone()), TreeAge(age), GpuGrowth)).id()
}

#[test]
fn trees_with_equal_looks_share_both_materials_and_the_node_buffer() {
    let params = TreePreset::Oak.params();
    let mut app = app(true);
    let trees: Vec<_> = (0..3).map(|i| spawn(&mut app, &params, 1.0 + i as f32)).collect();
    app.update();
    let [a, b, c] = [0, 1, 2].map(|i| paint(&app, trees[i]));
    assert_eq!(a.trunk, b.trunk);
    assert_eq!(a.trunk, c.trunk);
    assert_eq!(a.foliage, b.foliage);
    assert_ne!(a.trunk, a.foliage, "wood and leaves differ");
    assert_eq!(a.slice.buffer().id(), b.slice.buffer().id());
    assert_eq!(app.world().resource::<Assets<GrowthMaterial>>().len(), 2, "one pair for all three");
    let ranges = [a, b, c].map(|p| p.slice.base()..p.slice.base() + p.slice.len() as u32);
    assert!(ranges[0].end <= ranges[1].start && ranges[1].end <= ranges[2].start, "{ranges:?}");

    // Each tree has its own state in its own slice, found through the tag of its meshes.
    for (i, &tree) in trees.iter().enumerate() {
        let expected = expected_buffer(&params, 1.0 + i as f32);
        assert_eq!(uploaded_buffer(&app, tree).unwrap(), expected);
        let tagged: Vec<u32> = app
            .world()
            .get::<Children>(tree)
            .unwrap()
            .iter()
            .map(|child| app.world().get::<MeshTag>(child).expect("tagged").value)
            .collect();
        assert_eq!(tagged, [paint(&app, tree).slice.base(); 2]);
    }
}

#[test]
fn a_different_look_gets_its_own_material_only_where_it_differs() {
    let params = TreePreset::Oak.params();
    let mut autumn = params.clone();
    autumn.foliage.color = bevy::color::Color::srgb(0.8, 0.4, 0.1);
    let mut app = app(true);
    let green = spawn(&mut app, &params, 3.0);
    let orange = spawn(&mut app, &autumn, 3.0);
    app.update();
    let (g, o) = (paint(&app, green), paint(&app, orange));
    assert_eq!(g.trunk, o.trunk, "same wood");
    assert_ne!(g.foliage, o.foliage, "different leaves");
    assert_eq!(app.world().resource::<Assets<GrowthMaterial>>().len(), 3);

    // A different species wind response is a different look too, for wood and leaves.
    let mut stiff = params.clone();
    stiff.wind.sway = 0.1;
    let rigid = spawn(&mut app, &stiff, 3.0);
    app.update();
    assert_ne!(paint(&app, rigid).trunk, paint(&app, green).trunk);
    assert_ne!(paint(&app, rigid).foliage, paint(&app, green).foliage);
}

#[test]
fn recolouring_a_tree_leaves_the_trees_it_shared_with_alone() {
    let params = TreePreset::Oak.params();
    let mut app = app(true);
    let a = spawn(&mut app, &params, 3.0);
    let b = spawn(&mut app, &params, 3.0);
    app.update();
    let red = bevy::color::Color::srgb(1.0, 0.0, 0.0);
    app.world_mut().get_mut::<ProceduralTree>(a).unwrap().0.trunk_color = red;
    app.update();
    assert_ne!(paint(&app, a).trunk, paint(&app, b).trunk);
    assert_eq!(paint(&app, a).foliage, paint(&app, b).foliage, "the leaves did not change");
    let materials = app.world().resource::<Assets<GrowthMaterial>>();
    assert_eq!(materials.get(&paint(&app, a).trunk).unwrap().base.base_color, red);
    assert_ne!(materials.get(&paint(&app, b).trunk).unwrap().base.base_color, red);
    // The child draws with the new material, and nothing was modified in place.
    let child = app.world().get::<TreeRender>(a).unwrap().trunk_entity();
    assert_eq!(app.world().get::<MeshMaterial3d<GrowthMaterial>>(child).unwrap().0, paint(&app, a).trunk);
    assert_eq!(modified::<GrowthMaterial>(&app), 0);

    // Recolouring the other one the same way finds the material again instead of making a third.
    app.world_mut().get_mut::<ProceduralTree>(b).unwrap().0.trunk_color = red;
    app.update();
    assert_eq!(paint(&app, a).trunk, paint(&app, b).trunk);
    app.update();
    assert_eq!(app.world().resource::<Assets<GrowthMaterial>>().len(), 2, "the unused wood material is gone");
}

#[test]
fn a_trees_slice_is_freed_when_it_is_despawned_and_the_next_tree_reuses_it() {
    let params = TreePreset::Oak.params();
    let mut app = app(true);
    let keep = spawn(&mut app, &params, 2.0);
    let gone = spawn(&mut app, &params, 2.0);
    app.update();
    let gone_base = paint(&app, gone).slice.base();
    assert!(gone_base > paint(&app, keep).slice.base());

    app.world_mut().despawn(gone);
    app.update();
    let next = spawn(&mut app, &params, 5.0);
    app.update();
    assert_eq!(paint(&app, next).slice.base(), gone_base, "the freed slots are the first fit");
    assert_eq!(uploaded_buffer(&app, next).unwrap(), expected_buffer(&params, 5.0));
    assert_eq!(uploaded_buffer(&app, keep).unwrap(), expected_buffer(&params, 2.0), "the other tree is untouched");
    assert_eq!(store(&app).chunk_count(), 1);

    app.world_mut().despawn(keep);
    app.world_mut().despawn(next);
    app.update();
    assert!(store(&app).is_unused(), "every slice is back and nothing is left queued");
}

#[test]
fn a_slice_is_freed_when_the_tree_leaves_the_gpu_path() {
    let params = TreePreset::Willow.params();
    let mut app = app(true);
    let tree = spawn(&mut app, &params, 5.0);
    app.update();
    assert!(!store(&app).is_unused());

    app.world_mut().entity_mut(tree).remove::<GpuGrowth>();
    app.update();
    assert!(!is_gpu(&app, tree));
    assert!(store(&app).is_unused(), "no `GpuGrowth`: the slice went back");

    app.world_mut().entity_mut(tree).insert(GpuGrowth);
    app.world_mut().insert_resource(GpuGrowthSupport(false));
    app.update();
    assert!(store(&app).is_unused(), "no GPU support: no slice");
    app.world_mut().insert_resource(GpuGrowthSupport(true));
    app.update();
    assert!(!store(&app).is_unused());
    app.world_mut().insert_resource(GpuGrowthSupport(false));
    app.update();
    assert!(store(&app).is_unused(), "support lost: the slice went back");
}

#[test]
fn a_state_of_another_size_moves_the_tree_to_a_new_slice_without_touching_the_others() {
    let oak = TreePreset::Oak.params();
    let mut app = app(true);
    let before = spawn(&mut app, &oak, 3.0);
    let moving = spawn(&mut app, &oak, 3.0);
    let after = spawn(&mut app, &oak, 3.0);
    app.update();
    let old_base = paint(&app, moving).slice.base();

    let pine = TreePreset::Pine.params();
    assert_ne!(expected_buffer(&pine, 3.0).len(), expected_buffer(&oak, 3.0).len(), "the test needs a new size");
    app.world_mut().get_mut::<ProceduralTree>(moving).unwrap().0 = pine.clone();
    app.update();
    assert_eq!(uploaded_buffer(&app, moving).unwrap(), expected_buffer(&pine, 3.0));
    assert_eq!(uploaded_buffer(&app, before).unwrap(), expected_buffer(&oak, 3.0));
    assert_eq!(uploaded_buffer(&app, after).unwrap(), expected_buffer(&oak, 3.0));
    // Both children carry the new start, and the old slot is free for the next tree.
    let new_base = paint(&app, moving).slice.base();
    for &child in app.world().get::<Children>(moving).unwrap() {
        assert_eq!(app.world().get::<MeshTag>(child).unwrap().value, new_base);
    }
    let next = spawn(&mut app, &oak, 3.0);
    app.update();
    let slice = &paint(&app, next).slice;
    assert!(slice.base() + slice.len() as u32 <= new_base || slice.base() >= new_base + paint(&app, moving).slice.len() as u32);
    assert_ne!(old_base, new_base);
}

#[test]
fn a_tree_that_has_reached_maturity_is_not_written_again() {
    let params = TreePreset::Spruce.params();
    let mature_at = generate_skeleton(&params).mature_at(&params.growth);
    let mut app = app(true);
    let tree = spawn(&mut app, &params, mature_at * 0.5);
    app.update();
    assert!(store(&app).take_queued() > 0, "the first write");

    app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = mature_at * 0.75;
    app.update();
    assert_eq!(store(&app).take_queued(), 1, "growing: written");

    app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = mature_at;
    app.update();
    assert_eq!(store(&app).take_queued(), 1, "reaching maturity: written once");
    assert_eq!(uploaded_buffer(&app, tree), None, "taken");

    for age in [mature_at, mature_at + 1.0, mature_at + 50.0] {
        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = age;
        app.update();
        assert_eq!(store(&app).take_queued(), 0, "age {age}: nothing to write");
    }
}
