//! The material of a tree that grows in the vertex shader.

use bevy::app::App;
use bevy::asset::{Asset, Handle};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline, StandardMaterial};
use bevy::reflect::Reflect;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;

use super::{ATTRIBUTE_GROWTH_AXIS, ATTRIBUTE_GROWTH_ID, ATTRIBUTE_GROWTH_OFFSET};

/// [`StandardMaterial`] with the vertex stage of a tree that grows on the GPU.
/// The wood and leaves of a tree with [`GpuGrowth`](crate::GpuGrowth) use it.
pub type GrowthMaterial = ExtendedMaterial<StandardMaterial, GrowthExtension>;

/// The part of [`GrowthMaterial`] that moves vertices: it binds the tree's
/// per-node state, which the vertex shader reads (and writes nothing back).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct GrowthExtension {
    /// Per-node growth state, rewritten by the plugin every frame the tree's age changes.
    // Binding slots below 100 belong to the base material.
    #[storage(100, read_only)]
    pub nodes: Handle<ShaderBuffer>,
}

impl MaterialExtension for GrowthExtension {
    fn vertex_shader() -> ShaderRef {
        "embedded://bevy_flora/gpu/growth.wesl".into()
    }

    // Shadow maps run through the prepass pipeline, so this is also what makes a
    // tree's shadow follow its growth.
    fn prepass_vertex_shader() -> ShaderRef {
        "embedded://bevy_flora/gpu/growth_prepass.wesl".into()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // The base material chose the attributes it needs and their locations
        // (0-7 in both the main pass and the prepass); add ours after them.
        let growth = layout.0.get_layout(&[
            ATTRIBUTE_GROWTH_ID.at_shader_location(8),
            ATTRIBUTE_GROWTH_AXIS.at_shader_location(9),
            ATTRIBUTE_GROWTH_OFFSET.at_shader_location(10),
        ])?;
        descriptor.vertex.buffers[0].attributes.extend(growth.attributes);
        Ok(())
    }
}

/// Embed the shaders.
pub(crate) fn add_shaders(app: &mut App) {
    bevy::shader::load_shader_library!(app, "growth_common.wesl");
    bevy::asset::embedded_asset!(app, "growth.wesl");
    bevy::asset::embedded_asset!(app, "growth_prepass.wesl");
}
