//! Bevy integration: components, the `FloraPlugin`, and the systems that
//! grow trees and rebuild their meshes.
//!
//! A tree entity carries [`ProceduralTree`] (its parameters) and [`TreeAge`].
//! Whenever either changes, [`rebuild_trees`] regenerates the meshes and keeps
//! two child entities (wood and leaves) in sync. Adding [`Growing`] advances
//! `TreeAge` every frame until the tree is mature.
//!
//! Performance: a growing tree rebuilds both meshes every frame (0.1-0.4 ms of
//! CPU per tree); `Growing` is meant for a few hero trees, not a forest. The
//! full-grown skeleton is kept between frames and regenerated only when
//! [`ProceduralTree`] changes, and the materials are only touched when a colour
//! changes, so the renderer does not re-prepare them every frame.
//!
//! For a forest, add [`GpuGrowth`] to the trees: the mature meshes are built and
//! uploaded once, and each frame only a few kilobytes of per-node state are
//! written (a few microseconds of CPU per tree); the vertex shader does the
//! growing. See its docs for what differs from the CPU path.

use bevy::asset::{Assets, Handle};
use bevy::ecs::prelude::*;
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::reflect::Reflect;
use bevy::render::render_resource::Face;
use bevy::camera::visibility::Visibility;
use bevy::time::Time;
use bevy::transform::components::Transform;

use crate::gpu::render::{GrowthPaint, detect_gpu_growth_support, rebuild_gpu_trees, sync_growth_mode};
use crate::gpu::{GpuGrowth, GpuGrowthSupport};
use crate::mesh::MeshData;
use crate::params::TreeParams;
use crate::pipeline::build_aged;
use crate::skeleton::{Skeleton, generate_skeleton};

/// Parameters of a procedural tree. Changing it rebuilds the meshes.
#[derive(Component, Clone, Debug, PartialEq, Reflect)]
#[reflect(Component)]
#[require(TreeAge, Transform, Visibility)]
pub struct ProceduralTree(pub TreeParams);

/// Age of the tree in growth units (see [`crate::GrowthParams`]).
/// Defaults to `f32::INFINITY`: fully grown.
#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct TreeAge(pub f32);

impl Default for TreeAge {
    fn default() -> Self {
        Self(f32::INFINITY)
    }
}

/// Advances [`TreeAge`] by `rate` age units per second until mature.
#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct Growing {
    /// Age units added to [`TreeAge`] per second.
    pub rate: f32,
}

impl Default for Growing {
    fn default() -> Self {
        Self { rate: 1.0 }
    }
}

/// Sent after a tree's meshes were (re)built.
#[derive(Message, Clone, Copy, Debug)]
pub struct TreeGenerated {
    /// The tree entity whose meshes were rebuilt.
    pub entity: Entity,
    /// Current trunk height (full height × growth progress).
    pub height: f32,
    /// Trunk radius parameter.
    pub radius: f32,
}

/// Render state owned by the plugin for one tree entity.
#[derive(Component)]
pub struct TreeRender {
    pub(crate) trunk: Entity,
    /// `None` until the trunk first has geometry.
    pub(crate) trunk_mesh: Option<Handle<Mesh>>,
    pub(crate) paint: Paint,
    pub(crate) foliage: Option<(Entity, Handle<Mesh>)>,
    /// Full-grown skeleton generated from the current parameters; aging it
    /// is all a growing tree needs per frame.
    pub(crate) full: Skeleton,
    /// Age after which rebuilding changes nothing.
    pub mature_at: f32,
}

/// How a tree's wood and leaves are drawn.
pub(crate) enum Paint {
    /// Meshes rebuilt on the CPU, drawn with plain materials.
    Standard {
        trunk: Handle<StandardMaterial>,
        foliage: Handle<StandardMaterial>,
    },
    /// The mature meshes, grown in the vertex shader.
    Growth(GrowthPaint),
}

impl TreeRender {
    /// Child entity carrying the wood mesh.
    pub fn trunk_entity(&self) -> Entity {
        self.trunk
    }

    /// Child entity carrying the leaf mesh, if any leaves are currently visible.
    pub fn foliage_entity(&self) -> Option<Entity> {
        self.foliage.as_ref().map(|(e, _)| *e)
    }
}

/// Marker on the wood child entity.
#[derive(Component)]
pub struct TreeTrunkMesh;

/// Marker on the leaves child entity.
#[derive(Component)]
pub struct TreeFoliageMesh;

/// Registers the tree components and the grow/rebuild systems.
pub struct FloraPlugin;

impl bevy::app::Plugin for FloraPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_message::<TreeGenerated>()
            .register_type::<ProceduralTree>()
            .register_type::<TreeAge>()
            .register_type::<Growing>()
            .add_systems(
                bevy::app::Update,
                (
                    detect_gpu_growth_support,
                    grow_trees,
                    sync_growth_mode,
                    rebuild_trees,
                    rebuild_gpu_trees,
                )
                    .chain(),
            );
        crate::gpu::build(app);
    }

    fn finish(&self, app: &mut bevy::app::App) {
        crate::gpu::finish(app);
    }
}

/// Share of diffuse light a leaf lets through to its other side, so leaves
/// turned away from the sun are lit from behind instead of near-black.
const LEAF_TRANSLUCENCY: f32 = 0.5;

pub(crate) fn material(base_color: bevy::color::Color) -> StandardMaterial {
    StandardMaterial {
        base_color,
        perceptual_roughness: 1.0,
        cull_mode: Some(Face::Back),
        ..Default::default()
    }
}

pub(crate) fn leaf_material(base_color: bevy::color::Color) -> StandardMaterial {
    StandardMaterial {
        diffuse_transmission: LEAF_TRANSLUCENCY,
        ..material(base_color)
    }
}

/// Advance the age of every [`Growing`] tree that already has render state.
pub fn grow_trees(time: Res<Time>, mut trees: Query<(&mut TreeAge, &Growing, &TreeRender)>) {
    let dt = time.delta_secs();
    for (mut age, growing, render) in &mut trees {
        let next = (age.0 + growing.rate * dt).min(render.mature_at);
        if next != age.0 {
            age.0 = next;
        }
    }
}

/// Rebuild meshes for trees whose parameters or age changed.
#[allow(clippy::type_complexity)]
pub fn rebuild_trees(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    support: Res<GpuGrowthSupport>,
    mut generated: MessageWriter<TreeGenerated>,
    mut trees: Query<
        (Entity, Ref<ProceduralTree>, &TreeAge, Option<&mut TreeRender>, Has<GpuGrowth>),
        Or<(Changed<ProceduralTree>, Changed<TreeAge>, Without<TreeRender>)>,
    >,
) {
    for (entity, tree, age, mut render, gpu) in &mut trees {
        // A tree that grows on the GPU is `rebuild_gpu_trees`'s.
        if gpu && support.0 {
            continue;
        }
        let params = &tree.0;
        let standard = match render.as_deref() {
            Some(render) => match &render.paint {
                Paint::Standard { trunk, foliage } => Some((trunk.clone(), foliage.clone())),
                // Left over from before the tree stopped growing on the GPU;
                // `sync_growth_mode` removes it.
                Paint::Growth(_) => continue,
            },
            None => None,
        };
        // Only the age changed: age the skeleton generated last time.
        let full = match render.as_mut() {
            Some(render) if !tree.is_changed() => std::mem::take(&mut render.full),
            _ => generate_skeleton(params),
        };
        let out = build_aged(&full, params, age.0);
        let trunk_progress = out.skeleton.nodes[0].progress;
        match (render, standard) {
            (None, _) => {
                let trunk_material = materials.add(material(params.trunk_color));
                let foliage_material = materials.add(leaf_material(params.foliage.color));
                let trunk = commands
                    .spawn((
                        MeshMaterial3d(trunk_material.clone()),
                        TreeTrunkMesh,
                        Visibility::Hidden,
                        ChildOf(entity),
                    ))
                    .id();
                let mut trunk_mesh = None;
                sync_trunk(&mut commands, &mut meshes, trunk, &mut trunk_mesh, out.trunk);
                let foliage = (!out.foliage.is_empty()).then(|| {
                    let handle = meshes.add(out.foliage.into_bevy_mesh());
                    let child = commands
                        .spawn((
                            Mesh3d(handle.clone()),
                            MeshMaterial3d(foliage_material.clone()),
                            TreeFoliageMesh,
                            ChildOf(entity),
                        ))
                        .id();
                    (child, handle)
                });
                commands.entity(entity).insert(TreeRender {
                    trunk,
                    trunk_mesh,
                    paint: Paint::Standard {
                        trunk: trunk_material,
                        foliage: foliage_material,
                    },
                    foliage,
                    full,
                    mature_at: out.mature_at,
                });
            }
            (Some(mut render), Some((trunk_material, foliage_material))) => {
                render.full = full;
                render.mature_at = out.mature_at;
                let trunk_entity = render.trunk;
                sync_trunk(
                    &mut commands,
                    &mut meshes,
                    trunk_entity,
                    &mut render.trunk_mesh,
                    out.trunk,
                );
                set_color(&mut materials, &trunk_material, params.trunk_color);
                set_color(&mut materials, &foliage_material, params.foliage.color);

                match (&mut render.foliage, out.foliage.is_empty()) {
                    (Some((child, handle)), false) => {
                        let child = *child;
                        replace_mesh(&mut commands, &mut meshes, child, handle, out.foliage.into_bevy_mesh());
                    }
                    (None, false) => {
                        let handle = meshes.add(out.foliage.into_bevy_mesh());
                        let child = commands
                            .spawn((
                                Mesh3d(handle.clone()),
                                MeshMaterial3d(foliage_material.clone()),
                                TreeFoliageMesh,
                                ChildOf(entity),
                            ))
                            .id();
                        render.foliage = Some((child, handle));
                    }
                    (Some((child, _)), true) => {
                        commands.entity(*child).despawn();
                        render.foliage = None;
                    }
                    (None, true) => {}
                }
            }
            // A tree painted for the GPU was skipped above.
            (Some(_), None) => {}
        }

        generated.write(TreeGenerated {
            entity,
            height: params.trunk.height * trunk_progress,
            radius: params.trunk.radius,
        });
    }
}

/// Set the material's base colour, marking it modified only when the colour
/// differs: a modified material is re-extracted and re-prepared by the
/// renderer, which a growing tree would otherwise trigger every frame.
fn set_color(materials: &mut Assets<StandardMaterial>, handle: &Handle<StandardMaterial>, color: bevy::color::Color) {
    if let Some(mut material) = materials.get_mut(handle)
        && material.base_color != color
    {
        material.base_color = color;
    }
}

/// Bring the trunk child in sync with `data`. An empty trunk (age 0) hides the
/// child and uploads nothing; the mesh asset is created the first time the
/// trunk has geometry.
fn sync_trunk(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    child: Entity,
    handle: &mut Option<Handle<Mesh>>,
    data: MeshData,
) {
    if data.is_empty() {
        commands.entity(child).insert(Visibility::Hidden);
        return;
    }
    commands.entity(child).insert(Visibility::Inherited);
    match handle {
        Some(handle) => replace_mesh(commands, meshes, child, handle, data.into_bevy_mesh()),
        None => {
            let new = meshes.add(data.into_bevy_mesh());
            commands.entity(child).insert(Mesh3d(new.clone()));
            *handle = Some(new);
        }
    }
}

/// Overwrite the asset behind `handle` in place; if it was removed externally,
/// allocate a new one and re-point `child`'s `Mesh3d` at it.
pub(crate) fn replace_mesh(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    child: Entity,
    handle: &mut Handle<Mesh>,
    mesh: Mesh,
) {
    if let Some(mut slot) = meshes.get_mut(&*handle) {
        *slot = mesh;
        return;
    }
    *handle = meshes.add(mesh);
    commands.entity(child).insert(Mesh3d(handle.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::TreePreset;
    use bevy::app::App;
    use bevy::asset::{AssetApp, AssetPlugin};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(FloraPlugin);
        app
    }

    #[test]
    fn spawns_children_and_drops_foliage_at_age_zero() {
        let mut app = app();
        let tree = app
            .world_mut()
            .spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(f32::INFINITY)))
            .id();
        app.update();

        {
            let world = app.world();
            let render = world.get::<TreeRender>(tree).expect("TreeRender inserted");
            assert!(render.foliage.is_some());
            assert_eq!(world.get::<Children>(tree).unwrap().len(), 2);
            let meshes = world.resource::<Assets<Mesh>>();
            let trunk = meshes.get(render.trunk_mesh.as_ref().unwrap()).unwrap();
            assert!(trunk.count_vertices() > 0);
            assert_eq!(world.get::<Visibility>(render.trunk_entity()), Some(&Visibility::Inherited));
            assert!(render.mature_at.is_finite());
        }

        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = 0.0;
        app.update();
        {
            let world = app.world();
            let render = world.get::<TreeRender>(tree).unwrap();
            assert!(render.foliage.is_none());
            assert_eq!(world.get::<Children>(tree).unwrap().len(), 1);
            assert_eq!(world.get::<Visibility>(render.trunk_entity()), Some(&Visibility::Hidden));
        }

        // Back to mature: foliage child returns.
        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = f32::INFINITY;
        app.update();
        assert_eq!(app.world().get::<Children>(tree).unwrap().len(), 2);
        let world = app.world();
        let render = world.get::<TreeRender>(tree).unwrap();
        assert_eq!(world.get::<Visibility>(render.trunk_entity()), Some(&Visibility::Inherited));
    }

    #[test]
    fn trunk_born_empty_gets_its_mesh_once_it_grows() {
        let mut app = app();
        let tree = app
            .world_mut()
            .spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(0.0)))
            .id();
        app.update();
        let trunk = {
            let world = app.world();
            let render = world.get::<TreeRender>(tree).unwrap();
            assert!(render.trunk_mesh.is_none());
            let trunk = render.trunk_entity();
            assert_eq!(world.get::<Visibility>(trunk), Some(&Visibility::Hidden));
            assert!(world.get::<Mesh3d>(trunk).is_none());
            trunk
        };

        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = 2.0;
        app.update();
        let world = app.world();
        let render = world.get::<TreeRender>(tree).unwrap();
        let handle = render.trunk_mesh.as_ref().expect("mesh created once the trunk exists");
        assert!(world.resource::<Assets<Mesh>>().get(handle).unwrap().count_vertices() > 0);
        assert_eq!(world.get::<Mesh3d>(trunk).map(|m| &m.0), Some(handle));
        assert_eq!(world.get::<Visibility>(trunk), Some(&Visibility::Inherited));
    }

    #[test]
    fn growing_stops_at_maturity() {
        let mut app = app();
        let tree = app
            .world_mut()
            .spawn((
                ProceduralTree(TreePreset::Oak.params()),
                TreeAge(0.0),
                Growing { rate: 1.0e7 },
            ))
            .id();
        app.update(); // first build at age 0 (no TreeRender yet when grow runs)
        app.update(); // grows to mature_at
        let age = app.world().get::<TreeAge>(tree).unwrap().0;
        let mature_at = app.world().get::<TreeRender>(tree).unwrap().mature_at;
        assert_eq!(age, mature_at);
        app.update();
        assert_eq!(app.world().get::<TreeAge>(tree).unwrap().0, mature_at);
    }

    /// Wood vertex positions the plugin uploaded for `tree`.
    fn uploaded_trunk(app: &App, tree: Entity) -> Vec<[f32; 3]> {
        let world = app.world();
        let render = world.get::<TreeRender>(tree).unwrap();
        let mesh = world.resource::<Assets<Mesh>>().get(render.trunk_mesh.as_ref().unwrap()).unwrap();
        mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap().to_vec()
    }

    fn expected_trunk(params: &TreeParams, age: f32) -> Vec<[f32; 3]> {
        crate::pipeline::build_tree(params, age).trunk.positions.iter().map(|p| p.to_array()).collect()
    }

    #[test]
    fn growing_trees_reuse_their_skeleton_until_the_parameters_change() {
        // The full skeleton is kept between age-only rebuilds; the meshes must
        // still be exactly what `build_tree` makes, and a parameter change
        // must regenerate it rather than age the old tree.
        let mut app = app();
        let mut params = TreePreset::Oak.params();
        let tree = app.world_mut().spawn((ProceduralTree(params.clone()), TreeAge(2.0))).id();
        app.update();
        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = 5.0;
        app.update();
        assert_eq!(uploaded_trunk(&app, tree), expected_trunk(&params, 5.0));

        params.seed = 7;
        params.trunk.height *= 1.2;
        app.world_mut().get_mut::<ProceduralTree>(tree).unwrap().0 = params.clone();
        app.update();
        assert_eq!(uploaded_trunk(&app, tree), expected_trunk(&params, 5.0));
        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = 6.0;
        app.update();
        assert_eq!(uploaded_trunk(&app, tree), expected_trunk(&params, 6.0));
    }

    #[test]
    fn growing_leaves_materials_alone_until_a_colour_changes() {
        // A modified material is re-extracted and re-prepared by the renderer;
        // a growing tree used to mark both of its materials modified every frame.
        let modified = |app: &App| {
            app.world()
                .resource::<Messages<bevy::asset::AssetEvent<StandardMaterial>>>()
                .iter_current_update_messages()
                .filter(|e| matches!(e, bevy::asset::AssetEvent::Modified { .. }))
                .count()
        };
        let mut app = app();
        let tree = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(2.0))).id();
        app.update();
        for age in [3.0, 4.0] {
            app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = age;
            app.update();
            assert_eq!(modified(&app), 0, "age {age}");
        }
        let red = bevy::color::Color::srgb(1.0, 0.0, 0.0);
        app.world_mut().get_mut::<ProceduralTree>(tree).unwrap().0.trunk_color = red;
        app.update();
        assert_eq!(modified(&app), 1);
        let world = app.world();
        let render = world.get::<TreeRender>(tree).unwrap();
        let Paint::Standard { trunk, .. } = &render.paint else { panic!("a plain tree has plain materials") };
        assert_eq!(world.resource::<Assets<StandardMaterial>>().get(trunk).unwrap().base_color, red);
    }
}
