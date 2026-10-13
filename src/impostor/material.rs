//! The material of an impostor, and the one its pictures are baked with.

use bevy::app::App;
use bevy::asset::{Asset, Handle};
use bevy::color::LinearRgba;
use bevy::image::Image;
use bevy::material::AlphaMode;
use bevy::math::Vec4;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::reflect::TypePath;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use bevy::render::storage::ShaderBuffer;

use crate::params::RenderStyle;
use crate::shading::{FloraUniform, FloraWindBuffer};

/// Draws a far tree as a quad that turns about the trunk to face the camera and
/// shows the baked view (see [`ImpostorAtlas`](super::ImpostorAtlas)) closest to
/// the direction the camera looks from.
///
/// The pictures hold the tree's colour and its surface (normal, crown visibility,
/// translucency), not its lighting: the fragment stage lights every pixel with the scene's
/// own lights, shadows, ambient light and fog, and lets light through backlit leaves like
/// [`FloraMaterial`](crate::FloraMaterial). The quad bends with the wind.
///
/// Drawn with alpha to coverage: with MSAA a pixel draws as many of its samples as the share of
/// it the baked tree covers (the atlas's mipmaps average that share), so a far crown keeps the
/// gaps between its leaves the way its meshes do; without MSAA it is alpha-masked at one half.
/// No shadows and no prepasses; it cross-fades with a
/// [`VisibilityRange`](bevy::camera::visibility::VisibilityRange) like any mesh.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
#[bind_group_data(ImpostorKey)]
pub struct ImpostorMaterial {
    /// The colour atlas: views side by side, with mipmaps; alpha is coverage.
    #[texture(0)]
    #[sampler(1)]
    pub albedo: Handle<Image>,
    /// The surface atlas, laid out like `albedo`: the normal in the tree's own frame,
    /// octahedral-encoded, in `rg`, the share of the sky the leaf sees in `b` and how much
    /// light it lets through in `a`. Linear.
    #[texture(2)]
    pub surface: Handle<Image>,
    /// `x`: the number of views in the atlas; `y`: the [`RenderStyle`] (0 natural, 1 painterly, 2 toon).
    #[uniform(3)]
    pub params: Vec4,
    /// The look of the leaves and the species' response to the wind.
    #[uniform(101)]
    pub flora: FloraUniform,
    /// The weather, shared by every flora material: [`FloraWindBuffer`](crate::FloraWindBuffer).
    #[storage(102, read_only)]
    pub wind: Handle<ShaderBuffer>,
    /// Whether the see-through fade ([`SeeThroughTrees`](crate::SeeThroughTrees)) cuts this
    /// impostor; the plugin sets it while a camera has one.
    pub see_through: bool,
}

impl ImpostorMaterial {
    /// A material for the atlases `albedo` and `surface` with `views` views, lit in `style`
    /// ([`RenderStyle::Toon`] bands the light), bending in the wind of `wind`.
    pub fn new(
        albedo: Handle<Image>,
        surface: Handle<Image>,
        views: u32,
        style: RenderStyle,
        flora: FloraUniform,
        wind: &FloraWindBuffer,
    ) -> Self {
        Self {
            albedo,
            surface,
            params: Vec4::new(views as f32, style as u32 as f32, 0.0, 0.0),
            flora,
            wind: wind.0.clone(),
            see_through: false,
        }
    }
}

/// What an [`ImpostorMaterial`] changes about the pipeline it is drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImpostorKey {
    /// The see-through fade: see [`ImpostorMaterial::see_through`].
    pub see_through: bool,
}

impl From<&ImpostorMaterial> for ImpostorKey {
    fn from(material: &ImpostorMaterial) -> Self {
        Self { see_through: material.see_through }
    }
}

impl Material for ImpostorMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://bevy_flora/impostor/impostor.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://bevy_flora/impostor/impostor.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::AlphaToCoverage
    }

    // The vertex stage places the quad itself, so the default prepass and
    // shadow shaders would draw it somewhere else; impostors are far away and
    // cast no shadows.
    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // The quad turns to face the camera whatever its winding looks like
        // under the tree's own transform (a mirrored tree flips it).
        descriptor.primitive.cull_mode = None;
        if key.bind_group_data.see_through
            && let Some(fragment) = descriptor.fragment.as_mut()
        {
            fragment.shader_defs.push("FLORA_SEE_THROUGH".into());
        }
        Ok(())
    }
}

/// What a tree's meshes are drawn with to bake one of the two pictures of an impostor: the
/// material's colour times the mesh's vertex colours (the colour pass), or the mesh's normal,
/// crown visibility and translucency (the surface pass). Unlit; nothing else is drawn.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub(crate) struct ImpostorBakeMaterial {
    /// Linear colour of the wood or the leaves.
    #[uniform(0)]
    pub color: LinearRgba,
    /// `x`: 0 for the colour pass, 1 for the surface pass; `y`: translucency (1 for leaves,
    /// 0 for wood); `z`: crown visibility of a mesh without a second UV set.
    #[uniform(1)]
    pub params: Vec4,
}

impl ImpostorBakeMaterial {
    pub(crate) fn colour(color: LinearRgba) -> Self {
        Self { color, params: Vec4::new(0.0, 0.0, 1.0, 0.0) }
    }

    pub(crate) fn surface(color: LinearRgba, translucency: f32) -> Self {
        Self { color, params: Vec4::new(1.0, translucency, 1.0, 0.0) }
    }
}

impl Material for ImpostorBakeMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://bevy_flora/impostor/impostor_bake.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://bevy_flora/impostor/impostor_bake.wesl".into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// Embed the shaders.
pub(crate) fn add_shaders(app: &mut App) {
    bevy::asset::embedded_asset!(app, "impostor.wesl");
    bevy::asset::embedded_asset!(app, "impostor_bake.wesl");
}
