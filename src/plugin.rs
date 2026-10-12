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
//! [`ProceduralTree`] changes. Trees built or rebuilt in the same frame are built
//! on all cores. Trees with equal looks (equal colours, trunk height, wind response and leaf
//! shading; seeds do not matter) share their materials, which are never
//! written in place, so the renderer prepares each look once and does not
//! re-prepare anything as a tree grows.
//!
//! For a forest, add [`GpuGrowth`] to the trees: the mature meshes are built and
//! uploaded once, and each frame only a few kilobytes of per-node state are
//! written (a few microseconds of CPU per tree); the vertex shader does the
//! growing. See its docs for what differs from the CPU path.

use bevy::asset::{Assets, Handle};
use bevy::ecs::prelude::*;
use bevy::ecs::system::EntityCommands;
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::reflect::Reflect;
use bevy::render::render_resource::Face;
use bevy::camera::visibility::Visibility;
use bevy::tasks::{ComputeTaskPool, TaskPool};
use bevy::time::Time;
use bevy::transform::components::Transform;

use crate::gpu::render::{GrowthPaint, detect_gpu_growth_support, rebuild_gpu_trees, sync_growth_mode};
use crate::gpu::{GpuGrowth, GpuGrowthSupport};
use crate::impostor::{ImpostorSupport, drive_impostor_bakes, start_impostor_bakes};
use crate::params::TreeParams;
use crate::pipeline::build_aged;
use crate::pool::spawn_pooled_trees;
use crate::shading::{
    FloraMaterial, FloraMaterialSupport, FloraWind, FloraWindBuffer, SharedFlora, TextureLibrary, detect_flora_support,
    flora_decided, sync_wind,
};
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
    /// Meshes rebuilt on the CPU, drawn with plain materials (the renderer's plugins came after
    /// this one, or the device has no storage buffers in the vertex stage, so
    /// [`FloraMaterial`] cannot be drawn).
    Standard {
        trunk: Handle<StandardMaterial>,
        foliage: Handle<StandardMaterial>,
    },
    /// Meshes rebuilt on the CPU, drawn with leaf shading and wind.
    Flora {
        trunk: Handle<FloraMaterial>,
        foliage: Handle<FloraMaterial>,
    },
    /// The mature meshes, grown in the vertex shader.
    Growth(GrowthPaint),
}

impl Paint {
    /// This paint's handles, for a tree rebuilt on the CPU; `None` for one grown on the GPU.
    fn cloned_cpu(&self) -> Option<Paint> {
        match self {
            Paint::Standard { trunk, foliage } => Some(Paint::Standard {
                trunk: trunk.clone(),
                foliage: foliage.clone(),
            }),
            Paint::Flora { trunk, foliage } => Some(Paint::Flora {
                trunk: trunk.clone(),
                foliage: foliage.clone(),
            }),
            Paint::Growth(_) => None,
        }
    }
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
                    detect_flora_support,
                    sync_wind.run_if(resource_changed::<FloraWind>),
                    grow_trees,
                    sync_growth_mode,
                    rebuild_trees.run_if(flora_decided),
                    rebuild_gpu_trees,
                )
                    .chain(),
            )
            .add_systems(
                bevy::app::Update,
                (
                    (start_impostor_bakes, drive_impostor_bakes)
                        .chain()
                        .run_if(resource_exists::<ImpostorSupport>),
                    spawn_pooled_trees.run_if(flora_decided),
                )
                    .chain(),
            );
        // Before `gpu`: its shaders import the flora library.
        crate::shading::build(app);
        crate::gpu::build(app);
        crate::impostor::build(app);
        crate::pool::build(app);
    }

    fn finish(&self, app: &mut bevy::app::App) {
        crate::shading::finish(app);
        crate::gpu::finish(app);
        crate::impostor::finish(app);
        crate::pool::finish(app);
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
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn rebuild_trees(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut flora: ResMut<Assets<FloraMaterial>>,
    mut shared: ResMut<SharedFlora>,
    mut textures: TextureLibrary,
    flora_support: Option<Res<FloraMaterialSupport>>,
    wind: Option<Res<FloraWindBuffer>>,
    support: Res<GpuGrowthSupport>,
    mut generated: MessageWriter<TreeGenerated>,
    mut trees: Query<
        (Entity, Ref<ProceduralTree>, &TreeAge, Option<&mut TreeRender>, Has<GpuGrowth>),
        Or<(Changed<ProceduralTree>, Changed<TreeAge>, Without<TreeRender>)>,
    >,
) {
    let wind = wind.as_deref().filter(|_| flora_support.is_some());

    // Phase 1: collect what to build.
    let mut jobs = Vec::new();
    for (entity, tree, age, mut render, gpu) in &mut trees {
        // A tree that grows on the GPU is `rebuild_gpu_trees`'s.
        if gpu && support.0 {
            continue;
        }
        // Left over from before the tree stopped growing on the GPU;
        // `sync_growth_mode` removes it.
        if render.as_deref().is_some_and(|render| render.paint.cloned_cpu().is_none()) {
            continue;
        }
        // Only the age changed: age the skeleton generated last time.
        let kept = match render.as_mut() {
            Some(render) if !tree.is_changed() => Some(std::mem::take(&mut render.full)),
            _ => None,
        };
        jobs.push(RebuildJob { entity, params: tree.0.clone(), age: age.0, kept });
    }

    // Phase 2 builds on all cores, meshes included; phase 3 applies each result here, in order.
    let build = |job: RebuildJob| {
        let full = job.kept.unwrap_or_else(|| generate_skeleton(&job.params));
        let out = build_aged(&full, &job.params, job.age);
        RebuiltTree {
            entity: job.entity,
            full,
            trunk: (!out.trunk.is_empty()).then(|| out.trunk.into_bevy_mesh()),
            foliage: (!out.foliage.is_empty()).then(|| out.foliage.into_bevy_mesh()),
            mature_at: out.mature_at,
            trunk_progress: out.skeleton.nodes[0].progress,
        }
    };
    // Ageing a kept skeleton costs about 20 us a tree, less than waking the pool for a few
    // hundred of them (the growing-tree case); generating a skeleton is worth spreading.
    let parallel = jobs.len() > 1000 || jobs.iter().filter(|job| job.kept.is_none()).count() >= 2;
    build_in_batches(jobs, parallel, build, |out| {
        let (entity, full, trunk_progress) = (out.entity, out.full, out.trunk_progress);
        let Ok((_, tree, _, render, _)) = trees.get_mut(entity) else { return };
        let params = &tree.0;
        let existing = render.as_deref().and_then(|render| render.paint.cloned_cpu());
        match (render, existing) {
            (None, _) => {
                let paint = if let Some(wind) = wind {
                    Paint::Flora {
                        trunk: shared.resolve(&mut flora, &textures.wood(params), wind),
                        foliage: shared.resolve(&mut flora, &textures.leaves(params), wind),
                    }
                } else {
                    Paint::Standard {
                        trunk: materials.add(material(params.trunk_color)),
                        foliage: materials.add(leaf_material(params.foliage.color)),
                    }
                };
                let mut trunk_child = commands.spawn((TreeTrunkMesh, Visibility::Hidden, ChildOf(entity)));
                paint_child(&mut trunk_child, &paint, false);
                let trunk = trunk_child.id();
                let mut trunk_mesh = None;
                sync_trunk(&mut commands, &mut meshes, trunk, &mut trunk_mesh, out.trunk);
                let foliage = out.foliage.map(|mesh| {
                    let handle = meshes.add(mesh);
                    let mut child = commands.spawn((Mesh3d(handle.clone()), TreeFoliageMesh, ChildOf(entity)));
                    paint_child(&mut child, &paint, true);
                    (child.id(), handle)
                });
                commands.entity(entity).insert(TreeRender {
                    trunk,
                    trunk_mesh,
                    paint,
                    foliage,
                    full,
                    mature_at: out.mature_at,
                });
            }
            (Some(mut render), Some(paint)) => {
                let render = &mut *render;
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
                match &paint {
                    Paint::Standard { trunk, foliage } => {
                        set_color(&mut materials, trunk, params.trunk_color);
                        set_color(&mut materials, foliage, params.foliage.color);
                    }
                    // An age-only rebuild cannot change the looks, so it resolves nothing.
                    Paint::Flora { .. } if tree.is_changed() => {
                        let wind = wind.expect("flora paint implies the wind buffer");
                        let wood = shared.resolve(&mut flora, &textures.wood(params), wind);
                        let leaves = shared.resolve(&mut flora, &textures.leaves(params), wind);
                        if let Paint::Flora { trunk, foliage } = &mut render.paint {
                            if *trunk != wood {
                                commands.entity(render.trunk).insert(MeshMaterial3d(wood.clone()));
                                *trunk = wood;
                            }
                            if *foliage != leaves {
                                if let Some((child, _)) = &render.foliage {
                                    commands.entity(*child).insert(MeshMaterial3d(leaves.clone()));
                                }
                                *foliage = leaves;
                            }
                        }
                    }
                    Paint::Flora { .. } => {}
                    Paint::Growth(_) => unreachable!("trees grown on the GPU were skipped above"),
                }

                match (&mut render.foliage, out.foliage) {
                    (Some((child, handle)), Some(mesh)) => {
                        let child = *child;
                        replace_mesh(&mut commands, &mut meshes, child, handle, mesh);
                    }
                    (None, Some(mesh)) => {
                        let handle = meshes.add(mesh);
                        let mut child = commands.spawn((Mesh3d(handle.clone()), TreeFoliageMesh, ChildOf(entity)));
                        paint_child(&mut child, &render.paint, true);
                        render.foliage = Some((child.id(), handle));
                    }
                    (Some((child, _)), None) => {
                        commands.entity(*child).despawn();
                        render.foliage = None;
                    }
                    (None, None) => {}
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
    });
}

/// Run `build` on every job and hand each result to `apply` in the order of
/// `jobs`. With `parallel` the jobs are built on the compute task pool, a few per
/// thread at a time, so a large input never holds all its results at once;
/// without it (or with fewer than two jobs) they run inline, without a scope.
/// `build` must be a pure function of its job: the results do not depend on the
/// number of threads.
pub(crate) fn build_in_batches<J: Send, R: Send + 'static>(
    jobs: Vec<J>,
    parallel: bool,
    build: impl Fn(J) -> R + Sync,
    mut apply: impl FnMut(R),
) {
    if !parallel || jobs.len() < 2 {
        jobs.into_iter().for_each(|job| apply(build(job)));
        return;
    }
    let pool = ComputeTaskPool::get_or_init(TaskPool::default);
    let batch = pool.thread_num() * 4;
    let build = &build;
    let mut jobs = jobs.into_iter();
    loop {
        let results = pool.scope(|scope| {
            for job in jobs.by_ref().take(batch) {
                scope.spawn(async move { build(job) });
            }
        });
        if results.is_empty() {
            return;
        }
        results.into_iter().for_each(&mut apply);
    }
}

/// Give a wood or leaf child the material `paint` holds for it.
fn paint_child(child: &mut EntityCommands, paint: &Paint, leaf: bool) {
    match (paint, leaf) {
        (Paint::Standard { trunk, .. }, false) => child.insert(MeshMaterial3d(trunk.clone())),
        (Paint::Standard { foliage, .. }, true) => child.insert(MeshMaterial3d(foliage.clone())),
        (Paint::Flora { trunk, .. }, false) => child.insert(MeshMaterial3d(trunk.clone())),
        (Paint::Flora { foliage, .. }, true) => child.insert(MeshMaterial3d(foliage.clone())),
        (Paint::Growth(_), _) => unreachable!("trees grown on the GPU are spawned by rebuild_gpu_trees"),
    };
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
    mesh: Option<Mesh>,
) {
    let Some(mesh) = mesh else {
        commands.entity(child).insert(Visibility::Hidden);
        return;
    };
    commands.entity(child).insert(Visibility::Inherited);
    match handle {
        Some(handle) => replace_mesh(commands, meshes, child, handle, mesh),
        None => {
            let new = meshes.add(mesh);
            commands.entity(child).insert(Mesh3d(new.clone()));
            *handle = Some(new);
        }
    }
}

/// What a worker makes of one [`RebuildJob`]: the skeleton and the Bevy meshes
/// (`None` for an empty one).
struct RebuiltTree {
    entity: Entity,
    full: Skeleton,
    trunk: Option<Mesh>,
    foliage: Option<Mesh>,
    mature_at: f32,
    trunk_progress: f32,
}

/// One tree for `rebuild_trees` to build: its parameters, its age and, when only the age
/// changed, the full-grown skeleton kept from last time.
struct RebuildJob {
    entity: Entity,
    params: TreeParams,
    age: f32,
    kept: Option<Skeleton>,
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
    use crate::shading::{GustFront, wind_vectors};
    use bevy::app::App;
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::render::storage::ShaderBuffer;

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
    fn many_trees_built_in_one_frame_match_the_serial_build() {
        // More trees than one batch, so the scope runs more than once; a changed tree and an
        // aged tree go through the regenerate and kept-skeleton paths.
        let mut app = app();
        let mut specs = Vec::new();
        for i in 0..40 {
            let mut params = TreePreset::ALL[i % TreePreset::ALL.len()].params();
            params.seed = i as i32;
            let age = 1.0 + i as f32 * 0.4;
            let tree = app.world_mut().spawn((ProceduralTree(params.clone()), TreeAge(age))).id();
            specs.push((tree, params, age));
        }
        app.update();
        let check = |app: &App, specs: &[(Entity, TreeParams, f32)]| {
            for (tree, params, age) in specs {
                let want = crate::pipeline::build_tree(params, *age);
                let world = app.world();
                let render = world.get::<TreeRender>(*tree).unwrap();
                let meshes = world.resource::<Assets<Mesh>>();
                // Every attribute and the indices, not just the positions.
                let same = |handle: &Handle<Mesh>, data: crate::mesh::MeshData| {
                    let got = meshes.get(handle).unwrap();
                    let want = data.into_bevy_mesh();
                    assert_eq!(got.attributes().count(), want.attributes().count());
                    for (attribute, values) in want.attributes() {
                        assert_eq!(got.attribute(attribute.id), Some(values), "{}", attribute.name);
                    }
                    assert_eq!(got.indices(), want.indices());
                };
                match &render.trunk_mesh {
                    Some(handle) => same(handle, want.trunk),
                    None => assert!(want.trunk.is_empty()),
                }
                match &render.foliage {
                    Some((_, handle)) => same(handle, want.foliage),
                    None => assert!(want.foliage.is_empty()),
                }
                assert_eq!(render.mature_at, want.mature_at);
            }
        };
        check(&app, &specs);

        specs[3].1.trunk.height *= 1.3;
        specs[3].1.seed = 99;
        app.world_mut().get_mut::<ProceduralTree>(specs[3].0).unwrap().0 = specs[3].1.clone();
        specs[5].2 += 2.0;
        app.world_mut().get_mut::<TreeAge>(specs[5].0).unwrap().0 = specs[5].2;
        app.update();
        check(&app, &specs);
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

    /// The flora materials of one tree drawn with `FloraMaterial`: `(wood, leaves)`.
    fn flora_pair(app: &App, tree: Entity) -> (Handle<FloraMaterial>, Handle<FloraMaterial>) {
        let render = app.world().get::<TreeRender>(tree).unwrap();
        let Paint::Flora { trunk, foliage } = &render.paint else { panic!("expected flora materials") };
        (trunk.clone(), foliage.clone())
    }

    /// Run one update and return the ids of the flora materials it marked modified.
    fn update_and_modified(app: &mut App) -> Vec<bevy::asset::AssetId<FloraMaterial>> {
        let mut cursor = app
            .world()
            .resource::<Messages<bevy::asset::AssetEvent<FloraMaterial>>>()
            .get_cursor_current();
        app.update();
        let messages = app.world().resource::<Messages<bevy::asset::AssetEvent<FloraMaterial>>>();
        cursor
            .read(messages)
            .filter_map(|e| match e {
                bevy::asset::AssetEvent::Modified { id } => Some(*id),
                _ => None,
            })
            .collect()
    }

    fn flora_app() -> App {
        let mut app = app();
        app.insert_resource(FloraMaterialSupport);
        app
    }

    fn spawn_oak(app: &mut App, edit: impl FnOnce(&mut TreeParams)) -> Entity {
        let mut params = TreePreset::Oak.params();
        edit(&mut params);
        app.world_mut().spawn((ProceduralTree(params), TreeAge(f32::INFINITY))).id()
    }

    fn material_of(app: &App, child: Entity) -> Handle<FloraMaterial> {
        app.world().get::<MeshMaterial3d<FloraMaterial>>(child).unwrap().0.clone()
    }

    #[test]
    fn trees_use_flora_materials_where_the_renderer_registered_them() {
        let mut app = flora_app();
        let tree = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(2.0))).id();
        app.update();
        let (trunk, foliage) = flora_pair(&app, tree);
        {
            let assets = app.world().resource::<Assets<FloraMaterial>>();
            assert!(assets.get(&foliage).unwrap().extension.leaf, "the leaves are drawn as leaves");
            assert!(!assets.get(&trunk).unwrap().extension.leaf, "the wood is not");
        }
        for age in [3.0, 4.0, 5.0] {
            app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = age;
            assert!(update_and_modified(&mut app).is_empty(), "age {age} rewrote a material");
        }
        let render = app.world().get::<TreeRender>(tree).unwrap();
        assert_eq!(material_of(&app, render.trunk), trunk);
    }

    #[test]
    fn equal_looks_share_materials() {
        let mut app = flora_app();
        let a = spawn_oak(&mut app, |_| {});
        let b = spawn_oak(&mut app, |p| p.seed = 99);
        let c = spawn_oak(&mut app, |p| p.foliage.color = bevy::color::Color::srgb(1.0, 0.0, 0.0));
        let d = spawn_oak(&mut app, |p| p.foliage.shading.translucency = 0.2);
        let e = spawn_oak(&mut app, |p| p.wind.sway = 2.0);
        app.update();
        let (aw, al) = flora_pair(&app, a);
        let (bw, bl) = flora_pair(&app, b);
        assert_eq!((&aw, &al), (&bw, &bl));
        let (cw, cl) = flora_pair(&app, c);
        assert_eq!(aw, cw);
        assert_ne!(al, cl);
        let (dw, dl) = flora_pair(&app, d);
        assert_eq!(aw, dw);
        assert_ne!(al, dl);
        let (ew, el) = flora_pair(&app, e);
        assert_ne!(aw, ew);
        assert_ne!(al, el);
        let render = app.world().get::<TreeRender>(b).unwrap();
        assert_eq!(material_of(&app, render.trunk), aw);
        assert_eq!(material_of(&app, render.foliage.as_ref().unwrap().0), al);
    }

    #[test]
    fn changing_one_trees_look_leaves_the_sharing_tree_alone() {
        let mut app = flora_app();
        let a = spawn_oak(&mut app, |_| {});
        let b = spawn_oak(&mut app, |p| p.seed = 5);
        app.update();
        let (aw, al) = flora_pair(&app, a);
        app.world_mut().get_mut::<ProceduralTree>(b).unwrap().0.foliage.shading.translucency = 0.2;
        assert!(update_and_modified(&mut app).is_empty(), "the shared asset is not modified");
        assert_eq!(flora_pair(&app, a), (aw.clone(), al.clone()));
        let (bw, bl) = flora_pair(&app, b);
        assert_eq!(bw, aw);
        assert_ne!(bl, al);
        let assets = app.world().resource::<Assets<FloraMaterial>>();
        assert_eq!(assets.get(&bl).unwrap().extension.flora.leaf.x, 0.2);
        assert_ne!(assets.get(&al).unwrap().extension.flora.leaf.x, 0.2);
        let render = app.world().get::<TreeRender>(b).unwrap();
        assert_eq!(material_of(&app, render.foliage.as_ref().unwrap().0), bl);
    }

    #[test]
    fn a_look_nobody_uses_any_more_is_made_again() {
        let mut app = flora_app();
        let a = spawn_oak(&mut app, |_| {});
        app.update();
        app.world_mut().entity_mut(a).despawn();
        // Dropped handles are freed over a couple of updates.
        for _ in 0..3 {
            app.update();
        }
        let b = spawn_oak(&mut app, |p| p.seed = 3);
        app.update();
        let (bw, bl) = flora_pair(&app, b);
        let assets = app.world().resource::<Assets<FloraMaterial>>();
        assert!(assets.get(&bw).is_some() && assets.get(&bl).is_some());
    }

    /// The contents of the shared wind buffer.
    fn wind_buffer(app: &App) -> Vec<[f32; 4]> {
        let handle = &app.world().resource::<FloraWindBuffer>().0;
        let buffers = app.world().resource::<Assets<ShaderBuffer>>();
        buffers.get(handle).unwrap().cast_slice::<[f32; 4]>().unwrap().to_vec()
    }

    /// Run one update and count the `Modified` events of shader buffers it produced.
    fn modified_buffers(app: &mut App) -> usize {
        let mut cursor = app
            .world()
            .resource::<Messages<bevy::asset::AssetEvent<ShaderBuffer>>>()
            .get_cursor_current();
        app.update();
        let messages = app.world().resource::<Messages<bevy::asset::AssetEvent<ShaderBuffer>>>();
        cursor.read(messages).filter(|e| matches!(e, bevy::asset::AssetEvent::Modified { .. })).count()
    }

    #[test]
    fn changing_the_wind_modifies_no_material() {
        let mut app = app();
        app.insert_resource(FloraMaterialSupport);
        app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(f32::INFINITY)));
        for seed in 1..4 {
            spawn_oak(&mut app, |p| p.seed = seed);
        }
        app.update();
        assert!(update_and_modified(&mut app).is_empty());
        assert_eq!(wind_buffer(&app), wind_vectors(&FloraWind::default()).map(|v| v.to_array()));

        app.world_mut().resource_mut::<FloraWind>().strength = 0.9;
        assert!(update_and_modified(&mut app).is_empty(), "a new strength modified a material");
        let wind = *app.world().resource::<FloraWind>();
        assert_eq!(wind_buffer(&app), wind_vectors(&wind).map(|v| v.to_array()));
        assert_eq!(wind_buffer(&app)[0][2], 0.9);

        // A gust front is wind too.
        app.world_mut().resource_mut::<FloraWind>().launch_gust(GustFront::default(), &Time::default());
        assert!(update_and_modified(&mut app).is_empty(), "a launched gust modified a material");
        let wind = *app.world().resource::<FloraWind>();
        assert_eq!(wind_buffer(&app), wind_vectors(&wind).map(|v| v.to_array()));
        assert!(wind_buffer(&app)[4][2] > 0.0);

        // Touching the resource without changing the wind writes nothing.
        app.world_mut().resource_mut::<FloraWind>().set_changed();
        assert_eq!(modified_buffers(&mut app), 0, "an unchanged wind wrote the buffer");
        app.world_mut().resource_mut::<FloraWind>().strength = 0.2;
        assert_eq!(modified_buffers(&mut app), 1, "a changed wind writes the buffer once");
    }

    #[test]
    fn a_material_made_after_a_wind_change_sees_the_current_wind() {
        let mut app = app();
        app.insert_resource(FloraMaterialSupport);
        app.world_mut().resource_mut::<FloraWind>().strength = 0.77;
        app.update();
        let tree = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeAge(f32::INFINITY))).id();
        app.update();
        let (_, foliage) = flora_pair(&app, tree);
        // The material holds the shared buffer, not a copy of the wind.
        let assets = app.world().resource::<Assets<FloraMaterial>>();
        assert_eq!(assets.get(&foliage).unwrap().extension.wind, app.world().resource::<FloraWindBuffer>().0);
        assert_eq!(wind_buffer(&app)[0][2], 0.77);
    }
}
