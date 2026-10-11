//! The systems that put a [`GpuGrowth`] tree on screen.

use std::collections::HashMap;

use bevy::asset::{AssetId, Assets, Handle};
use bevy::color::{Color, ColorToComponents};
use bevy::ecs::prelude::*;
use bevy::math::Vec4;
use bevy::mesh::{Mesh, Mesh3d, MeshTag};
use bevy::pbr::MeshMaterial3d;
use bevy::render::renderer::RenderDevice;
use bevy::render::storage::ShaderBuffer;

use super::material::{GrowthExtension, GrowthMaterial};
use super::mesh::build_growth_data;
use super::state::GrowthState;
use super::store::{GrowthSlice, GrowthStore};
use super::{GpuGrowth, GpuGrowthSupport, GrowthMaterialAdded};
use crate::plugin::{
    Paint, ProceduralTree, TreeAge, TreeFoliageMesh, TreeGenerated, TreeRender, TreeTrunkMesh, replace_mesh,
};
use crate::shading::{FloraUniform, FloraWind, flora_base};
use crate::skeleton::generate_skeleton;

/// What a tree that grows on the GPU draws with.
pub(crate) struct GrowthPaint {
    pub trunk: Handle<GrowthMaterial>,
    pub foliage: Handle<GrowthMaterial>,
    /// The tree's part of the node buffer both materials bind.
    pub slice: GrowthSlice,
    /// What was last written to `slice`.
    pub state: GrowthState,
    /// The age `state` was written for, at most the tree's `mature_at`: a tree that has
    /// reached it is not written again.
    pub written_age: f32,
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

/// The materials trees with equal looks share: one per (chunk of the node buffer, colour,
/// flora uniform without the wind, wood or leaf).
///
/// Entries are weak: a material lives while some tree draws with it, and entries of dead
/// ones are pruned whenever a new material is made.
#[derive(Resource, Default)]
pub(crate) struct GrowthMaterials(HashMap<Look, AssetId<GrowthMaterial>>);

/// What makes two [`GrowthMaterial`]s interchangeable. The global wind is not part of it: every
/// flora material is brought to the same wind whenever it changes.
#[derive(PartialEq, Eq, Hash)]
struct Look {
    nodes: AssetId<ShaderBuffer>,
    leaf: bool,
    color: [u32; 4],
    tree: [u32; 4],
    response: [u32; 4],
    leaf_look: [u32; 4],
    tint: [u32; 4],
}

fn bits(v: Vec4) -> [u32; 4] {
    v.to_array().map(f32::to_bits)
}

impl GrowthMaterials {
    /// The shared material for a tree whose state is in `nodes`: the wood's when `leaf` is
    /// false (then `flora` is already [`FloraUniform::wood`]'d), the leaves' otherwise.
    fn get(
        &mut self,
        materials: &mut Assets<GrowthMaterial>,
        nodes: &Handle<ShaderBuffer>,
        color: Color,
        flora: FloraUniform,
        leaf: bool,
    ) -> Handle<GrowthMaterial> {
        let look = Look {
            nodes: nodes.id(),
            leaf,
            color: color.to_linear().to_f32_array().map(f32::to_bits),
            tree: bits(flora.tree),
            response: bits(flora.response),
            leaf_look: bits(flora.leaf),
            tint: bits(flora.tint),
        };
        if let Some(handle) = self.0.get(&look).and_then(|&id| materials.get_strong_handle(id)) {
            return handle;
        }
        self.0.retain(|_, id| materials.contains(*id));
        let handle = materials.add(GrowthMaterial {
            base: flora_base(color),
            extension: GrowthExtension { nodes: nodes.clone(), flora, leaf },
        });
        self.0.insert(look, handle.id());
        handle
    }
}

/// Build (or bring up to date) the render state of trees that grow on the GPU.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn rebuild_gpu_trees(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GrowthMaterial>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut shared: ResMut<GrowthMaterials>,
    store: Res<GrowthStore>,
    support: Res<GpuGrowthSupport>,
    wind: Res<FloraWind>,
    mut generated: MessageWriter<TreeGenerated>,
    mut trees: Query<
        (Entity, Ref<ProceduralTree>, &TreeAge, Option<&mut TreeRender>, Has<GpuGrowth>),
        Or<(Changed<ProceduralTree>, Changed<TreeAge>, Without<TreeRender>)>,
    >,
) {
    for (entity, tree, age, mut render, gpu) in &mut trees {
        if !(gpu && support.0) {
            continue;
        }
        let params = &tree.0;
        let progress;
        match render.as_deref_mut() {
            // Only the age changed: age the kept skeleton and write the node state, unless
            // the tree was already as grown as it gets.
            Some(render) if !tree.is_changed() => {
                let TreeRender {
                    full,
                    paint: Paint::Growth(paint),
                    mature_at,
                    ..
                } = render
                else {
                    continue;
                };
                let reached = age.0.min(*mature_at);
                if reached != paint.written_age {
                    paint.state.update(full, params, age.0);
                    paint.slice.write(&paint.state);
                    paint.written_age = reached;
                }
                progress = paint.state.trunk_progress;
            }
            // The parameters changed: rebuild the mature meshes in place.
            Some(render) => {
                let Paint::Growth(paint) = &mut render.paint else { continue };
                let full = generate_skeleton(params);
                let data = build_growth_data(&full, params);
                paint.state.update(&full, params, age.0);
                // The state can have another size, which needs another slice, and with it the
                // meshes need the new start.
                let moved = paint.state.vec4_len() != paint.slice.len();
                if moved {
                    paint.slice = store.allocate(paint.state.vec4_len(), &mut buffers);
                }
                paint.slice.write(&paint.state);
                progress = paint.state.trunk_progress;
                render.mature_at = full.mature_at(&params.growth);
                paint.written_age = age.0.min(render.mature_at);
                render.full = full;
                let (wood, leaves) = data.into_meshes();
                if let Some(handle) = render.trunk_mesh.as_mut() {
                    replace_mesh(&mut commands, &mut meshes, render.trunk, handle, wood);
                }
                // The materials are shared, so a new look is another material, not an edit of this one.
                let uniform = FloraUniform::new(params, &wind);
                let nodes = paint.slice.buffer();
                let trunk = shared.get(&mut materials, nodes, params.trunk_color, uniform.wood(), false);
                let foliage = shared.get(&mut materials, nodes, params.foliage.color, uniform, true);
                if trunk != paint.trunk {
                    commands.entity(render.trunk).insert(MeshMaterial3d(trunk.clone()));
                    paint.trunk = trunk;
                }
                if moved {
                    commands.entity(render.trunk).insert(MeshTag::new(paint.slice.base()));
                }
                if foliage != paint.foliage {
                    if let Some((child, _)) = &render.foliage {
                        commands.entity(*child).insert(MeshMaterial3d(foliage.clone()));
                    }
                    paint.foliage = foliage;
                }
                match (&mut render.foliage, leaves) {
                    (Some((child, handle)), Some(leaves)) => {
                        replace_mesh(&mut commands, &mut meshes, *child, handle, leaves);
                        if moved {
                            commands.entity(*child).insert(MeshTag::new(paint.slice.base()));
                        }
                    }
                    (None, Some(leaves)) => {
                        let handle = meshes.add(leaves);
                        let child = commands
                            .spawn((
                                Mesh3d(handle.clone()),
                                MeshMaterial3d(paint.foliage.clone()),
                                MeshTag::new(paint.slice.base()),
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
                let full = generate_skeleton(params);
                let data = build_growth_data(&full, params);
                let mut state = data.state.clone();
                state.update(&full, params, age.0);
                progress = state.trunk_progress;
                let slice = store.allocate(state.vec4_len(), &mut buffers);
                slice.write(&state);
                let (wood, leaves) = data.into_meshes();

                let uniform = FloraUniform::new(params, &wind);
                let nodes = slice.buffer();
                let trunk_material = shared.get(&mut materials, nodes, params.trunk_color, uniform.wood(), false);
                let foliage_material = shared.get(&mut materials, nodes, params.foliage.color, uniform, true);
                let tag = MeshTag::new(slice.base());
                let trunk_mesh = meshes.add(wood);
                let trunk = commands
                    .spawn((
                        Mesh3d(trunk_mesh.clone()),
                        MeshMaterial3d(trunk_material.clone()),
                        tag.clone(),
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
                            tag,
                            TreeFoliageMesh,
                            ChildOf(entity),
                        ))
                        .id();
                    (child, handle)
                });
                let mature_at = full.mature_at(&params.growth);
                commands.entity(entity).insert(TreeRender {
                    trunk,
                    trunk_mesh: Some(trunk_mesh),
                    paint: Paint::Growth(GrowthPaint {
                        trunk: trunk_material,
                        foliage: foliage_material,
                        slice,
                        state,
                        written_age: age.0.min(mature_at),
                    }),
                    foliage,
                    mature_at,
                    full,
                });
            }
        }

        generated.write(TreeGenerated {
            entity,
            height: params.trunk.height * progress,
            radius: params.trunk.radius,
        });
    }
}
