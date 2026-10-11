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
use crate::shading::{FloraKey, FloraUniform, specialize_flora};

/// [`StandardMaterial`] with the vertex stage of a tree that grows on the GPU, and the leaf
/// shading and wind of [`FloraMaterial`](crate::FloraMaterial). The wood and leaves of a tree
/// with [`GpuGrowth`](crate::GpuGrowth) use it.
pub type GrowthMaterial = ExtendedMaterial<StandardMaterial, GrowthExtension>;

/// The part of [`GrowthMaterial`] that moves vertices: it binds the tree's
/// per-node state, which the vertex shader reads (and writes nothing back), and the
/// uniform of the flora shaders.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[bind_group_data(FloraKey)]
pub struct GrowthExtension {
    /// Per-node growth state, rewritten by the plugin every frame the tree's age changes.
    // Binding slots below 100 belong to the base material.
    #[storage(100, read_only)]
    pub nodes: Handle<ShaderBuffer>,
    /// The look of the leaves and the species' response to the wind; see [`FloraUniform`].
    #[uniform(101)]
    pub flora: FloraUniform,
    /// The weather, shared by every flora material: [`FloraWindBuffer`](crate::FloraWindBuffer).
    #[storage(102, read_only)]
    pub wind: Handle<ShaderBuffer>,
    /// Leaves get the leaf surface, the light through the leaf and the flutter; wood only bends.
    /// See [`FloraExtension::leaf`](crate::FloraExtension::leaf).
    pub leaf: bool,
}

impl From<&GrowthExtension> for FloraKey {
    fn from(extension: &GrowthExtension) -> Self {
        Self { leaf: extension.leaf }
    }
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

    fn fragment_shader() -> ShaderRef {
        "embedded://bevy_flora/shading/fragment.wesl".into()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // The base material chose the attributes it needs and their locations
        // (0-7 in both the main pass and the prepass); add ours after them.
        let growth = layout.0.get_layout(&[
            ATTRIBUTE_GROWTH_ID.at_shader_location(8),
            ATTRIBUTE_GROWTH_AXIS.at_shader_location(9),
            ATTRIBUTE_GROWTH_OFFSET.at_shader_location(10),
        ])?;
        descriptor.vertex.buffers[0].attributes.extend(growth.attributes);
        // A leaf's flutter attribute goes after them (location 11).
        specialize_flora(descriptor, layout, key.bind_group_data.leaf)
    }
}

/// Embed the shaders.
pub(crate) fn add_shaders(app: &mut App) {
    bevy::shader::load_shader_library!(app, "growth_common.wesl");
    bevy::asset::embedded_asset!(app, "growth.wesl");
    bevy::asset::embedded_asset!(app, "growth_prepass.wesl");
}
