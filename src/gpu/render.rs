//! The systems that put a [`GpuGrowth`] tree on screen.

use bevy::asset::{Assets, Handle};
use bevy::ecs::prelude::*;
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::MeshMaterial3d;
use bevy::render::renderer::RenderDevice;
use bevy::render::storage::ShaderBuffer;

use super::material::{GrowthMaterial, growth_material};
use super::mesh::build_growth_data;
use super::state::GrowthState;
use super::{GpuGrowth, GpuGrowthSupport, GrowthMaterialAdded};
use crate::params::TreeParams;
use crate::plugin::{
    Paint, ProceduralTree, TreeAge, TreeFoliageMesh, TreeGenerated, TreeRender, TreeTrunkMesh, build_in_batches,
    replace_mesh,
};
use crate::shading::{BarkLibrary, FloraWindBuffer, Surface};
use crate::skeleton::generate_skeleton;

/// What a tree that grows on the GPU draws with.
pub(crate) struct GrowthPaint {
    pub trunk: Handle<GrowthMaterial>,
    pub foliage: Handle<GrowthMaterial>,
    /// The per-node state both materials bind.
    pub nodes: Handle<ShaderBuffer>,
    /// What was last written to `nodes`.
    pub state: GrowthState,
}

/// Decide once the renderer exists whether trees can grow on this device: it
/// must hold the per-node state (a storage buffer readable in the vertex stage)
/// and the plugin must have been added after the renderer's, which is what
/// registers [`GrowthMaterial`] with it.
pub(crate) fn detect_gpu_growth_support(
    device: Option<Res<RenderDevice>>,
    material: Option<Res<GrowthMaterialAdded>>,
    mut support: ResMut<GpuGrowthSupport>,
    mut checked: Local<bool>,
) {
    if *checked {
        return;
    }
    let Some(device) = device else { return };
    *checked = true;
    let storage = !bevy::render::storage_buffers_are_unsupported(&device.limits());
    if !storage {
        bevy::log::warn!(
            "this device has no storage buffers in the vertex stage (WebGL2?); trees with `GpuGrowth` grow on the CPU instead"
        );
    } else if material.is_none() {
        bevy::log::warn!(
            "`FloraPlugin` was added before the renderer's plugins, so `GpuGrowth` is off; add it after `DefaultPlugins`"
        );
    }
    let supported = storage && material.is_some();
    if support.0 != supported {
        support.0 = supported;
    }
}

/// Take a tree out of the mode it is no longer in: when its [`GpuGrowth`]
/// appears or goes, or when GPU growth becomes (un)available, drop its render
/// state and children so the right system rebuilds it.
pub(crate) fn sync_growth_mode(
    mut commands: Commands,
    support: Res<GpuGrowthSupport>,
    added: Query<(), Added<GpuGrowth>>,
    mut removed: RemovedComponents<GpuGrowth>,
    trees: Query<(Entity, &TreeRender, Has<GpuGrowth>)>,
) {
    let removed_any = removed.read().count() > 0;
    if !(support.is_changed() || removed_any || !added.is_empty()) {
        return;
    }
    for (entity, render, gpu) in &trees {
        let wanted = gpu && support.0;
        if wanted != matches!(render.paint, Paint::Growth(_)) {
            commands.entity(render.trunk).despawn();
            if let Some((child, _)) = &render.foliage {
                commands.entity(*child).despawn();
            }
            commands.entity(entity).remove::<TreeRender>();
        }
    }
}

/// Write `state` into the buffer behind `handle`, in place when the size is
/// unchanged (a plain write to the existing GPU buffer) and by replacing the
/// buffer when it is not.
fn write_state(buffers: &mut Assets<ShaderBuffer>, handle: &Handle<ShaderBuffer>, state: &GrowthState) {
    let Some(mut buffer) = buffers.get_mut(handle) else { return };
    if let Some(slice) = buffer.cast_slice_mut::<[f32; 4]>()
        && slice.len() == state.vec4_len()
    {
        state.encode(slice);
        return;
    }
    *buffer = state_buffer(state);
}

fn state_buffer(state: &GrowthState) -> ShaderBuffer {
    let mut data = vec![[0.0f32; 4]; state.vec4_len()];
    state.encode(&mut data);
    ShaderBuffer::from(data)
}

/// Bring a material's base colour, flora uniform and detail textures up to date, marking it
/// modified only when one differs.
fn sync_growth(materials: &mut Assets<GrowthMaterial>, handle: &Handle<GrowthMaterial>, surface: &Surface) {
    let (detail_color, detail_normal) = surface.detail();
    if let Some(mut material) = materials.get_mut(handle)
        && (material.base.base_color != surface.color
            || material.extension.flora != surface.uniform
            || material.extension.detail_color != detail_color
            || material.extension.detail_normal != detail_normal)
    {
        material.base.base_color = surface.color;
        material.extension.flora = surface.uniform;
        material.extension.detail_color = detail_color;
        material.extension.detail_normal = detail_normal;
    }
}

/// Build (or bring up to date) the render state of trees that grow on the GPU.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn rebuild_gpu_trees(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GrowthMaterial>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    support: Res<GpuGrowthSupport>,
    wind: Res<FloraWindBuffer>,
    mut bark: BarkLibrary,
    mut generated: MessageWriter<TreeGenerated>,
    mut trees: Query<
        (Entity, Ref<ProceduralTree>, &TreeAge, Option<&mut TreeRender>, Has<GpuGrowth>),
        Or<(Changed<ProceduralTree>, Changed<TreeAge>, Without<TreeRender>)>,
    >,
) {
    // Trees that only aged are updated here; the others are built on all cores below.
    let mut jobs = Vec::new();
    for (entity, tree, age, mut render, gpu) in &mut trees {
        if !(gpu && support.0) {
            continue;
        }
        match render.as_deref_mut() {
            // Only the age changed: age the kept skeleton and write the node state.
            Some(render) if !tree.is_changed() => {
                let TreeRender {
                    full,
                    paint: Paint::Growth(paint),
                    ..
                } = render
                else {
                    continue;
                };
                paint.state.update(full, &tree.0, age.0);
                write_state(&mut buffers, &paint.nodes, &paint.state);
                generated.write(TreeGenerated {
                    entity,
                    height: tree.0.trunk.height * paint.state.trunk_progress,
                    radius: tree.0.trunk.radius,
                });
            }
            // The parameters changed: rebuild the mature meshes in place.
            Some(render) => {
                if matches!(render.paint, Paint::Growth(_)) {
                    jobs.push(GrowthJob { entity, params: tree.0.clone(), age: age.0 });
                }
            }
            None => jobs.push(GrowthJob { entity, params: tree.0.clone(), age: age.0 }),
        }
    }

    let build = |job: GrowthJob| {
        let full = generate_skeleton(&job.params);
        let mut data = build_growth_data(&full, &job.params);
        let state = std::mem::take(&mut data.state);
        let (wood, leaves) = data.into_meshes();
        (job.entity, job.age, full, state, wood, leaves)
    };
    build_in_batches(jobs, true, build, |(entity, age, full, state, wood, leaves)| {
        let Ok((_, tree, _, mut render, _)) = trees.get_mut(entity) else { return };
        let params = &tree.0;
        let progress;
        match render.as_deref_mut() {
            Some(render) => {
                let Paint::Growth(paint) = &mut render.paint else { return };
                paint.state.update(&full, params, age);
                write_state(&mut buffers, &paint.nodes, &paint.state);
                progress = paint.state.trunk_progress;
                render.mature_at = full.mature_at(&params.growth);
                render.full = full;
                if let Some(handle) = render.trunk_mesh.as_mut() {
                    replace_mesh(&mut commands, &mut meshes, render.trunk, handle, wood);
                }
                let wood_bark = bark.of(params);
                sync_growth(&mut materials, &paint.trunk, &Surface::wood(params, wood_bark));
                sync_growth(&mut materials, &paint.foliage, &Surface::leaves(params));
                match (&mut render.foliage, leaves) {
                    (Some((child, handle)), Some(leaves)) => {
                        replace_mesh(&mut commands, &mut meshes, *child, handle, leaves);
                    }
                    (None, Some(leaves)) => {
                        let handle = meshes.add(leaves);
                        let child = commands
                            .spawn((
                                Mesh3d(handle.clone()),
                                MeshMaterial3d(paint.foliage.clone()),
                                TreeFoliageMesh,
                                ChildOf(entity),
                            ))
                            .id();
                        render.foliage = Some((child, handle));
                    }
                    (Some((child, _)), None) => {
                        commands.entity(*child).despawn();
                        render.foliage = None;
                    }
                    (None, None) => {}
                }
            }
            None => {
                let mut state = state;
                state.update(&full, params, age);
                progress = state.trunk_progress;
                let nodes = buffers.add(state_buffer(&state));

                let trunk_material = materials.add(growth_material(&nodes, &Surface::wood(params, bark.of(params)), &wind));
                let foliage_material = materials.add(growth_material(&nodes, &Surface::leaves(params), &wind));
                let trunk_mesh = meshes.add(wood);
                let trunk = commands
                    .spawn((
                        Mesh3d(trunk_mesh.clone()),
                        MeshMaterial3d(trunk_material.clone()),
                        TreeTrunkMesh,
                        ChildOf(entity),
                    ))
                    .id();
                let foliage = leaves.map(|leaves| {
                    let handle = meshes.add(leaves);
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
                    trunk_mesh: Some(trunk_mesh),
                    paint: Paint::Growth(GrowthPaint {
                        trunk: trunk_material,
                        foliage: foliage_material,
                        nodes,
                        state,
                    }),
                    foliage,
                    mature_at: full.mature_at(&params.growth),
                    full,
                });
            }
        }

        generated.write(TreeGenerated {
            entity,
            height: params.trunk.height * progress,
            radius: params.trunk.radius,
        });
    });
}

/// One tree for `rebuild_gpu_trees` to build: its parameters and the age to write.
struct GrowthJob {
    entity: Entity,
    params: TreeParams,
    age: f32,
}
