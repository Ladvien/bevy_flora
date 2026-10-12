//! Leaf shading and wind: the material trees are drawn with.
//!
//! [`FloraMaterial`] is a [`StandardMaterial`] with the shaders in this directory:
//!
//! - **Leaves** get an upper side that is darker and glossier than the paler, matte lower
//!   side, a midrib and side veins, a hue of their own and darker interiors of the crown (all
//!   baked into the leaf mesh by [`foliage_mesh`](crate::foliage::foliage_mesh)), and light
//!   that comes through a leaf lit from behind: the wrapped back-side `N·L` of Unreal's
//!   two-sided foliage model times a forward-scattering GGX lobe and the leaf colour, so the
//!   crown glows yellow-green toward the sun and stays dark where other leaves shade it. A
//!   leaf's gloss fades as the camera turns to face the sun: a flat card stands for a leaf or a
//!   cluster of needles, whose many surfaces would scatter the sun's glint, and reflecting all
//!   of it at once turned treetops white.
//! - **Wind** ([`FloraWind`]) bends a tree as a whole, more toward its top, and flutters each
//!   leaf (Crysis' vegetation animation, GPU Gems 3 chapter 16). A species answers it in its
//!   own way ([`WindResponse`](crate::WindResponse)), and a [`GustFront`] sweeps a band of
//!   stronger wind across the world. It runs in the vertex stage of the main pass and of the
//!   shadow maps alike, so shadows follow the motion, and costs the CPU nothing per frame:
//!   changing [`FloraWind`] writes one small shared buffer ([`FloraWindBuffer`]) and touches no
//!   material.
//!
//! The plugin uses it for every tree it draws ([`ProceduralTree`](crate::ProceduralTree),
//! [`GpuGrowth`](crate::GpuGrowth), [`TreePool`](crate::TreePool) and its impostors).

use bevy::app::App;
use bevy::asset::{Asset, AssetApp, AssetServer, Assets, Handle, embedded_asset};
use bevy::color::Color;
use bevy::ecs::prelude::*;
use bevy::log::warn;
use bevy::material::OpaqueRendererMethod;
use bevy::image::Image;
use bevy::math::{Vec2, Vec4};
use bevy::mesh::{Mesh, MeshVertexBufferLayoutRef};
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
    MaterialPlugin, PbrPlugin, StandardMaterial,
};
use bevy::reflect::Reflect;
use bevy::render::RenderApp;
use bevy::render::render_resource::{
    AsBindGroup, Face, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::render::renderer::RenderDevice;
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;
use bevy::time::Time;

use crate::foliage::uses_scan;
use crate::impostor::ImpostorSupport;
use crate::mesh::ATTRIBUTE_LEAF_SWAY;
use crate::params::{LeafStyle, TreeParams};

mod share;
pub use share::SharedFlora;

mod textures;
pub use textures::{BarkTexture, FloraTextures, LeafTexture, TextureLibrary};

/// A gust that sweeps across the world: a band of stronger wind, `width` metres deep, moving
/// along `direction` at `speed`. Launch it with [`FloraWind::launch_gust`].
///
/// Trees lean along `direction` and flutter harder while the band is over them, most at its
/// centre. The band has no sides: it reaches every tree it passes, however far to the side of
/// `origin`.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
pub struct GustFront {
    /// Where the front is (world x, z) at `start`.
    pub origin: Vec2,
    /// Which way it travels and pushes the trees, world x and z; normalised when used. Zero
    /// turns it off.
    pub direction: Vec2,
    /// Metres per second.
    pub speed: f32,
    /// Depth of the band of strong wind, metres (at least 0.01).
    pub width: f32,
    /// Wind strength added at the front's peak, on the scale of [`FloraWind::strength`].
    pub strength: f32,
    /// Render time ([`Time::elapsed_secs_wrapped`], the shaders' `globals.time`) when the front
    /// was at `origin`.
    pub start: f32,
}

impl Default for GustFront {
    fn default() -> Self {
        Self { origin: Vec2::ZERO, direction: Vec2::X, speed: 15.0, width: 30.0, strength: 1.0, start: 0.0 }
    }
}

/// The wind every tree drawn with [`FloraMaterial`] moves in.
///
/// Trees bend downwind as a whole, slowly and by more toward the top, and their leaves flutter
/// faster; how far and how fast each species does is [`TreeParams::wind`]. It is global: the
/// plugin writes it into one shared GPU buffer ([`FloraWindBuffer`]) whenever it changes, and
/// no material is touched, so a game may animate it every frame at the cost of one small buffer
/// write. Set `strength` to 0 and `gust` to `None` for a still scene (the gallery sets strength 0,
/// so its captures repeat).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct FloraWind {
    /// Where the wind blows toward, on the ground: world x and z. Zero is still air.
    pub direction: Vec2,
    /// 0 is calm, 1 a strong wind; more bends further (the bend keeps every part's distance from
    /// the trunk). Negative is treated as 0.
    pub strength: f32,
    /// Multiplies time: 2 moves everything twice as fast.
    pub speed: f32,
    /// How much leaves flutter, relative to the default of 1.
    pub flutter: f32,
    /// How far trees bend for a given `strength`, relative to the default of 1.
    pub bend: f32,
    /// How much the slow gusts vary the wind, from 0 (steady) to 1 (from calm to full).
    pub gust_strength: f32,
    /// How often gusts come, relative to the default of 1.
    pub gust_frequency: f32,
    /// How fast leaves flutter, relative to the default of 1.
    pub flutter_frequency: f32,
    /// Sideways sway across the wind, as a share of the bend downwind.
    pub turbulence: f32,
    /// A gust travelling across the world, if any: see [`GustFront`] and
    /// [`launch_gust`](Self::launch_gust).
    pub gust: Option<GustFront>,
}

impl Default for FloraWind {
    fn default() -> Self {
        Self {
            direction: Vec2::new(0.8, 0.6),
            strength: 0.3,
            speed: 1.0,
            flutter: 1.0,
            bend: 1.0,
            gust_strength: 0.4,
            gust_frequency: 1.0,
            flutter_frequency: 1.0,
            turbulence: 0.2,
            gust: None,
        }
    }
}

impl FloraWind {
    /// Send `front` across the world, starting now: it was at `front.origin` when this was
    /// called. It replaces any gust still under way; there is one at a time.
    ///
    /// The shaders time the front with the render clock, [`Time::elapsed_secs_wrapped`], which
    /// starts over every `Time::wrap_period` (an hour by default); a front launched less than
    /// its crossing time before that wrap jumps.
    pub fn launch_gust(&mut self, mut front: GustFront, time: &Time) {
        front.start = time.elapsed_secs_wrapped();
        self.gust = Some(front);
    }
}

/// What the flora shaders read from a material: the species' response to the wind and the look
/// of the leaves of the tree it belongs to. The wind itself is not here: it is the same for
/// every tree and lives in [`FloraWindBuffer`].
#[derive(ShaderType, Reflect, Clone, Copy, Debug, PartialEq, Default)]
pub struct FloraUniform {
    /// (1 / trunk height, 0, 0, 0).
    pub tree: Vec4,
    /// (sway, flutter, sway frequency, flutter frequency) of the species: [`TreeParams::wind`].
    pub response: Vec4,
    /// (translucency, roughness of the upper side, veins, underside) of the leaves.
    pub leaf: Vec4,
    /// Linear rgb that light takes on passing through a leaf; w is 0.
    pub tint: Vec4,
    /// (radius below which wood is smooth, radius above which it shows the whole scan, 0, 0) in
    /// metres: [`BarkParams`](crate::BarkParams). Zero for materials without bark.
    pub bark: Vec4,
    /// The bark scan's average colour, linear rgb, 0: what smooth wood is multiplied by, since
    /// the material's base colour is the preset's over it. Zero for materials without bark.
    pub bark_mean: Vec4,
}

impl FloraUniform {
    pub(crate) fn new(params: &TreeParams) -> Self {
        let s = &params.foliage.shading;
        let r = &params.wind;
        // The veins are drawn on leaf-shaped cards; the other styles' UVs are not a leaf.
        let veined = !matches!(params.foliage.style, LeafStyle::NeedleCluster | LeafStyle::ClusterSphere);
        let tint = s.transmission_tint.to_linear();
        Self {
            tree: Vec4::new(1.0 / params.trunk.height.max(0.01), 0.0, 0.0, 0.0),
            response: Vec4::new(
                r.sway.max(0.0),
                r.flutter.max(0.0),
                r.sway_frequency.max(0.0),
                r.flutter_frequency.max(0.0),
            ),
            leaf: Vec4::new(
                s.translucency.max(0.0),
                s.roughness.clamp(0.05, 1.0),
                if veined { s.veins.clamp(0.0, 1.0) } else { 0.0 },
                s.underside.clamp(0.0, 1.0),
            ),
            tint: Vec4::new(tint.red, tint.green, tint.blue, 0.0),
            bark: Vec4::ZERO,
            bark_mean: Vec4::ZERO,
        }
    }

    /// The same uniform for a tree's wood, which has no leaf look: the leaf fields are zeroed,
    /// so changing how leaves are shaded does not touch the wood's material. The wood bends like
    /// the species does.
    pub(crate) fn wood(self) -> Self {
        Self { leaf: Vec4::ZERO, tint: Vec4::ZERO, ..self }
    }
}

/// The five `Vec4`s of [`FloraWind`] the shaders read, in the order of the shared buffer:
/// `wind` (direction x, direction z, strength, speed), `motion` (bend, flutter, gust strength,
/// turbulence), `rate` (gust frequency, flutter frequency, 0, 0), `front` (origin x, origin z,
/// direction x, direction z) and `front_shape` (speed, width, strength, start time); the last
/// two are zero without a [`GustFront`], or with one that has no direction. Directions are
/// normalised and every value clamped to what the shaders can use.
pub(crate) fn wind_vectors(wind: &FloraWind) -> [Vec4; 5] {
    let d = wind.direction.normalize_or_zero();
    let (front, front_shape) = match wind.gust {
        Some(gust) if gust.direction.normalize_or_zero() != Vec2::ZERO => {
            let d = gust.direction.normalize_or_zero();
            (
                Vec4::new(gust.origin.x, gust.origin.y, d.x, d.y),
                Vec4::new(gust.speed.max(0.0), gust.width.max(0.01), gust.strength.max(0.0), gust.start),
            )
        }
        _ => (Vec4::ZERO, Vec4::ZERO),
    };
    [
        Vec4::new(d.x, d.y, wind.strength.max(0.0), wind.speed.max(0.0)),
        Vec4::new(wind.bend.max(0.0), wind.flutter.max(0.0), wind.gust_strength.clamp(0.0, 1.0), wind.turbulence.max(0.0)),
        Vec4::new(wind.gust_frequency.max(0.0), wind.flutter_frequency.max(0.0), 0.0, 0.0),
        front,
        front_shape,
    ]
}

/// What a [`FloraExtension`] changes about the pipeline it is drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FloraKey {
    /// Leaves: see [`FloraExtension::leaf`].
    pub leaf: bool,
    /// Detail textures (bark): see [`FloraExtension::detail_color`]. True when both maps are set.
    pub detail: bool,
    /// Leaf veins from an image: see [`FloraExtension::venation`].
    pub venation: bool,
}

/// The part of [`FloraMaterial`] that bends and shades.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, PartialEq)]
#[bind_group_data(FloraKey)]
pub struct FloraExtension {
    /// The look of the tree's leaves and its response to the wind.
    // Binding slots below 100 belong to the base material; 100 is GPU growth's node buffer,
    // which shares this extension's fragment shader.
    #[uniform(101)]
    pub flora: FloraUniform,
    /// The weather, shared by every flora material: [`FloraWindBuffer`].
    #[storage(102, read_only)]
    pub wind: Handle<ShaderBuffer>,
    /// Leaves get the leaf surface, the light through the leaf and the flutter (their mesh must
    /// carry the attributes [`foliage_mesh`](crate::foliage::foliage_mesh) bakes); wood only bends.
    pub leaf: bool,
    /// The colour map of the surface's detail (bark), multiplied into the base colour, or `None`
    /// for none: see [`BarkParams`](crate::BarkParams). Bound with the normal map and
    /// [`FloraUniform::bark`].
    #[texture(103)]
    #[sampler(105)]
    pub detail_color: Option<Handle<Image>>,
    /// The normal map of the surface's detail (tangent space, OpenGL convention).
    #[texture(104)]
    pub detail_normal: Option<Handle<Image>>,
    /// The image of leaf veins (`R8`, the card's UVs address it): the veins of a leaf without a
    /// scan, drawn by the leaf shader, or `None` for none. See
    /// [`FloraTextures::venation`].
    #[texture(106)]
    #[sampler(107)]
    pub venation: Option<Handle<Image>>,
}

impl From<&FloraExtension> for FloraKey {
    fn from(extension: &FloraExtension) -> Self {
        Self {
            leaf: extension.leaf,
            detail: extension.detail_color.is_some() && extension.detail_normal.is_some(),
            venation: extension.venation.is_some(),
        }
    }
}

/// [`StandardMaterial`] with leaf shading and wind. The wood and leaves of trees use it.
pub type FloraMaterial = ExtendedMaterial<StandardMaterial, FloraExtension>;

impl MaterialExtension for FloraExtension {
    fn vertex_shader() -> ShaderRef {
        "embedded://bevy_flora/shading/vertex.wesl".into()
    }

    // Shadow maps run through the prepass pipeline, so this is what makes shadows move too.
    fn prepass_vertex_shader() -> ShaderRef {
        "embedded://bevy_flora/shading/prepass.wesl".into()
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
        specialize_flora(descriptor, layout, key.bind_group_data)
    }
}

/// Make the pipeline of a leaf material read the leaf mesh's flutter attribute and turn on the
/// leaf branches of the shaders, and that of a material with detail textures sample them. Plain
/// wood needs nothing. Detail needs the mesh's two texture coordinate sets (bark is laid out in
/// the first, fades in with the second); a mesh without them is drawn without it.
pub(crate) fn specialize_flora(
    descriptor: &mut RenderPipelineDescriptor,
    layout: &MeshVertexBufferLayoutRef,
    key: FloraKey,
) -> Result<(), SpecializedMeshPipelineError> {
    if key.venation
        && layout.0.contains(Mesh::ATTRIBUTE_UV_0.id)
        && let Some(fragment) = descriptor.fragment.as_mut()
    {
        fragment.shader_defs.push("FLORA_VENATION".into());
    }
    if key.detail
        && layout.0.contains(Mesh::ATTRIBUTE_UV_0.id)
        && layout.0.contains(Mesh::ATTRIBUTE_UV_1.id)
        && let Some(fragment) = descriptor.fragment.as_mut()
    {
        fragment.shader_defs.push("FLORA_DETAIL".into());
    }
    if !key.leaf {
        return Ok(());
    }
    descriptor.vertex.shader_defs.push("FLORA_LEAF".into());
    if let Some(fragment) = descriptor.fragment.as_mut() {
        fragment.shader_defs.push("FLORA_LEAF".into());
    }
    // The base material chose its attributes and their locations (0-7); growth's are 8-10.
    let sway = layout.0.get_layout(&[ATTRIBUTE_LEAF_SWAY.at_shader_location(11)])?;
    descriptor.vertex.buffers[0].attributes.extend(sway.attributes);
    Ok(())
}

/// The base of a flora material: the leaf shader lights the surface itself, which only the
/// forward pass can do.
pub(crate) fn flora_base(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 1.0,
        cull_mode: Some(Face::Back),
        opaque_render_method: OpaqueRendererMethod::Forward,
        ..Default::default()
    }
}

/// What a flora material is made of, besides the wind: its base colour, the uniform the shaders
/// read, whether it is for leaves, and the textures it wears (bark, a leaf scan's atlas, or the
/// image of leaf veins). Two trees with equal surfaces share a material ([`SharedFlora`]).
#[derive(Clone, Debug)]
pub(crate) struct Surface {
    /// The material's base colour: for scanned wood or leaves the preset colour over the
    /// scan's average.
    pub color: Color,
    /// What the flora shaders read.
    pub uniform: FloraUniform,
    /// Leaves rather than wood.
    pub leaf: bool,
    /// The bark scan wood wears, with its images.
    pub bark: Option<BarkTexture>,
    /// The leaf scan leaves wear, with its atlas.
    pub leaf_scan: Option<LeafTexture>,
    /// The image of leaf veins a leaf without a scan wears.
    pub venation: Option<Handle<Image>>,
}

impl Surface {
    /// The wood of a tree with `params`, wearing `bark` when it is given.
    pub(crate) fn wood(params: &TreeParams, bark: Option<BarkTexture>) -> Self {
        let mut uniform = FloraUniform::new(params).wood();
        if let Some(bark) = &bark {
            uniform.bark = Vec4::new(params.bark.smooth_below, params.bark.fissured_above, 0.0, 0.0);
            uniform.bark_mean = Vec4::new(bark.mean.red, bark.mean.green, bark.mean.blue, 0.0);
        }
        let color = bark.as_ref().map_or(params.trunk_color, |bark| bark.base_color(params.trunk_color));
        Self { color, uniform, leaf: false, bark, leaf_scan: None, venation: None }
    }

    /// The leaves of a tree with `params`, wearing `leaf_scan`'s atlas, or `venation`'s veins.
    /// Cards cut to a scan's outline have the real leaf's veins in the atlas, so the uniform
    /// draws none of its own on them, whether or not the atlas could be loaded.
    pub(crate) fn leaves(
        params: &TreeParams,
        leaf_scan: Option<LeafTexture>,
        venation: Option<Handle<Image>>,
    ) -> Self {
        let mut uniform = FloraUniform::new(params);
        if uses_scan(&params.foliage) {
            uniform.leaf.z = 0.0;
        }
        let color = leaf_scan.as_ref().map_or(params.foliage.color, |scan| scan.base_color(params.foliage.color));
        Self { color, uniform, leaf: true, bark: None, leaf_scan, venation }
    }

    /// The colour and normal maps the material binds, if it has them.
    pub(crate) fn detail(&self) -> (Option<Handle<Image>>, Option<Handle<Image>>) {
        if let Some(bark) = &self.bark {
            (Some(bark.color.clone()), Some(bark.normal.clone()))
        } else if let Some(scan) = &self.leaf_scan {
            (Some(scan.color.clone()), Some(scan.normal.clone()))
        } else {
            (None, None)
        }
    }
}

pub(crate) fn flora_material(surface: &Surface, wind: &FloraWindBuffer) -> FloraMaterial {
    let (detail_color, detail_normal) = surface.detail();
    FloraMaterial {
        base: flora_base(surface.color),
        extension: FloraExtension {
            flora: surface.uniform,
            wind: wind.0.clone(),
            leaf: surface.leaf,
            detail_color,
            detail_normal,
            venation: surface.venation.clone(),
        },
    }
}

/// Whether [`FloraMaterial`] can be drawn: the plugin was added after the renderer's plugins
/// (which registers the material) and the device has storage buffers in the vertex stage (the
/// wind lives in one; everything but WebGL2 does). Decided at runtime, once the renderer
/// exists; until then no tree is built. Without it, trees are drawn with plain materials and
/// pools without impostors. Insert it yourself only where there is no renderer (a headless
/// test) and the asset type was registered by hand.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FloraMaterialSupport;

/// The wind the flora shaders read: one storage buffer of five `Vec4`s (direction and strength,
/// motion, rates and the [`GustFront`], packed from [`FloraWind`]), shared by every flora material
/// (binding 102), so changing [`FloraWind`]
/// writes this buffer in place and touches no material. Hand it to the `wind` field of a
/// [`FloraExtension`], [`GrowthExtension`](crate::GrowthExtension) or
/// [`ImpostorMaterial`](crate::ImpostorMaterial) you build yourself.
#[derive(Resource, Clone, Debug)]
pub struct FloraWindBuffer(pub Handle<ShaderBuffer>);

impl FloraWindBuffer {
    fn new(buffers: &mut Assets<ShaderBuffer>, wind: &FloraWind) -> Self {
        Self(buffers.add(ShaderBuffer::from(wind_vectors(wind).map(|v| v.to_array()).to_vec())))
    }
}

/// The renderer's plugins came before this one, so [`FloraMaterial`] is registered, but whether
/// the device can draw it is not known until it exists.
#[derive(Resource)]
pub(crate) struct FloraSupportPending;

/// Run condition: whether it is known how trees are drawn.
pub(crate) fn flora_decided(pending: Option<Res<FloraSupportPending>>) -> bool {
    pending.is_none()
}

/// Once the renderer's device exists, decide whether flora materials can be drawn on it; if
/// not, trees are drawn with plain materials and pools without impostors.
pub(crate) fn detect_flora_support(
    mut commands: Commands,
    device: Option<Res<RenderDevice>>,
    pending: Option<Res<FloraSupportPending>>,
) {
    let (Some(device), Some(_)) = (device, pending) else { return };
    commands.remove_resource::<FloraSupportPending>();
    if bevy::render::storage_buffers_are_unsupported(&device.limits()) {
        warn!(
            "bevy_flora: this device has no storage buffers in the vertex stage (WebGL2?), so trees \
             are drawn without leaf shading and wind, and pools without impostors"
        );
        commands.remove_resource::<ImpostorSupport>();
    } else {
        commands.insert_resource(FloraMaterialSupport);
    }
}

/// Write [`FloraWind`] into the shared buffer, in place. Runs when the wind changed.
pub(crate) fn sync_wind(
    wind: Res<FloraWind>,
    buffer: Res<FloraWindBuffer>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    let packed = wind_vectors(&wind).map(|v| v.to_array());
    // Look before borrowing mutably: a mutable borrow marks the buffer changed.
    if buffers.get(&buffer.0).and_then(|buffer| buffer.cast_slice::<[f32; 4]>()) == Some(&packed[..]) {
        return;
    }
    let Some(mut shader_buffer) = buffers.get_mut(&buffer.0) else { return };
    match shader_buffer.cast_slice_mut::<[f32; 4]>() {
        Some(slice) if slice.len() == packed.len() => slice.copy_from_slice(&packed),
        _ => *shader_buffer = ShaderBuffer::from(packed.to_vec()),
    }
}

/// Register the wind, and, when the renderer's plugins have already been added, the material
/// and its shaders.
///
/// Plugins may be added in any order. Added ahead of the renderer's (or of the asset plugin),
/// this registers nothing that needs them; [`finish`] covers the asset types, and trees are drawn
/// with plain materials.
pub(crate) fn build(app: &mut App) {
    app.init_resource::<FloraWind>()
        .init_resource::<SharedFlora>()
        .init_resource::<FloraTextures>()
        .register_type::<FloraWind>()
        .register_type::<GustFront>();
    if app.get_sub_app(RenderApp).is_some() && app.is_plugin_added::<PbrPlugin>() {
        add_shaders(app);
        app.add_plugins(MaterialPlugin::<FloraMaterial>::default()).insert_resource(FloraSupportPending);
    } else {
        add_asset_types(app);
    }
    add_wind_buffer(app);
}

/// Once every plugin is built, make sure the asset types the systems take exist even if the
/// plugin was added too early to register them.
pub(crate) fn finish(app: &mut App) {
    add_asset_types(app);
    add_wind_buffer(app);
    if app.get_sub_app(RenderApp).is_some() && !app.world().contains_resource::<FloraSupportPending>() {
        warn!(
            "bevy_flora: FloraPlugin was added before the renderer's plugins, so trees are drawn \
             without leaf shading and wind; add it after DefaultPlugins"
        );
    }
}

/// Register [`FloraMaterial`] and [`ShaderBuffer`] as assets where the asset plugin is present
/// and nobody has yet (the renderer's plugins do it when they are).
fn add_asset_types(app: &mut App) {
    if app.world().contains_resource::<AssetServer>() {
        if !app.world().contains_resource::<Assets<FloraMaterial>>() {
            app.init_asset::<FloraMaterial>();
        }
        if !app.world().contains_resource::<Assets<ShaderBuffer>>() {
            app.init_asset::<ShaderBuffer>();
        }
    }
}

/// Make the shared wind buffer, once there are buffer assets to put it in.
fn add_wind_buffer(app: &mut App) {
    if app.world().contains_resource::<FloraWindBuffer>() || !app.world().contains_resource::<Assets<ShaderBuffer>>() {
        return;
    }
    let wind = *app.world().resource::<FloraWind>();
    let buffer = FloraWindBuffer::new(&mut app.world_mut().resource_mut::<Assets<ShaderBuffer>>(), &wind);
    app.insert_resource(buffer);
}

/// Embed the shaders.
fn add_shaders(app: &mut App) {
    bevy::shader::load_shader_library!(app, "common.wesl");
    embedded_asset!(app, "vertex.wesl");
    embedded_asset!(app, "prepass.wesl");
    embedded_asset!(app, "fragment.wesl");
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::presets::TreePreset;

    #[test]
    fn veins_are_drawn_only_on_leaf_shaped_cards() {
        let veins = |style| {
            let mut params = TreePreset::Oak.params();
            params.foliage.style = style;
            params.foliage.shading.veins = 0.7;
            FloraUniform::new(&params).leaf.z
        };
        assert_eq!(veins(LeafStyle::CrossedPlanes), 0.7);
        assert_eq!(veins(LeafStyle::SingleQuad), 0.7);
        assert_eq!(veins(LeafStyle::NeedleCluster), 0.0);
        assert_eq!(veins(LeafStyle::ClusterSphere), 0.0);
    }

    #[test]
    fn the_uniform_holds_the_tree_and_nothing_of_the_weather() {
        let uniform = FloraUniform::new(&TreePreset::Willow.params());
        assert_eq!(uniform.response, Vec4::new(1.4, 1.6, 0.7, 0.8));
        let wood = uniform.wood();
        assert_eq!((wood.tree, wood.response), (uniform.tree, uniform.response));
        assert_eq!((wood.leaf, wood.tint), (Vec4::ZERO, Vec4::ZERO));
    }

    #[test]
    fn the_wind_is_packed_normalised_and_clamped() {
        let gale = FloraWind {
            direction: Vec2::new(0.0, -5.0),
            strength: 7.0,
            speed: -1.0,
            flutter: 2.0,
            bend: 3.0,
            gust_strength: 2.0,
            gust_frequency: 0.5,
            flutter_frequency: 4.0,
            turbulence: -1.0,
            gust: None,
        };
        let [wind, motion, rate, front, front_shape] = wind_vectors(&gale);
        // Direction normalised, a negative speed stopped, strength not capped, the rest clamped.
        assert_eq!(wind, Vec4::new(0.0, -1.0, 7.0, 0.0));
        assert_eq!(motion, Vec4::new(3.0, 2.0, 1.0, 0.0));
        assert_eq!(rate, Vec4::new(0.5, 4.0, 0.0, 0.0));
        assert_eq!((front, front_shape), (Vec4::ZERO, Vec4::ZERO));
        assert_ne!(wind_vectors(&FloraWind { strength: 0.0, ..gale }), wind_vectors(&gale));
    }

    #[test]
    fn gust_front_is_packed_normalised_and_off_when_absent() {
        let pack = |gust: Option<GustFront>| {
            let [.., front, front_shape] = wind_vectors(&FloraWind { gust, ..Default::default() });
            (front, front_shape)
        };

        assert_eq!(pack(None), (Vec4::ZERO, Vec4::ZERO));

        let front = GustFront {
            origin: Vec2::new(4.0, -2.0),
            direction: Vec2::new(0.0, 3.0),
            speed: 12.0,
            width: 0.0,
            strength: 2.0,
            start: 7.5,
        };
        assert_eq!(pack(Some(front)), (Vec4::new(4.0, -2.0, 0.0, 1.0), Vec4::new(12.0, 0.01, 2.0, 7.5)));

        // No direction, no front.
        assert_eq!(pack(Some(GustFront { direction: Vec2::ZERO, ..front })), (Vec4::ZERO, Vec4::ZERO));
    }

    /// The surfaces of `params`'s wood and leaves, resolved through the library in a bare world
    /// that has image assets.
    fn surfaces(params: &TreeParams, images: bool) -> (Surface, Surface) {
        use bevy::ecs::system::SystemState;
        let mut world = World::new();
        world.init_resource::<FloraTextures>();
        if images {
            world.init_resource::<Assets<Image>>();
        }
        let mut state = SystemState::<TextureLibrary>::new(&mut world);
        let mut library = state.get_mut(&mut world).expect("the library's resources exist");
        (library.wood(params), library.leaves(params))
    }

    #[test]
    fn leaves_wear_a_scan_or_the_vein_image_never_both_and_veins_only_where_a_card_is_a_leaf() {
        // The oak's cards are cut to a scanned leaf: its real veins are in the atlas, so none
        // are drawn, whether or not the atlas loaded (without the `textures` feature the card
        // is procedural again and draws its veins).
        let oak = TreePreset::Oak.params();
        let (_, leaves) = surfaces(&oak, true);
        if cfg!(feature = "textures") {
            assert!(leaves.leaf_scan.is_some() && leaves.venation.is_none());
            assert_eq!(leaves.uniform.leaf.z, 0.0);
            assert_ne!(leaves.color, oak.foliage.color, "the scan's mean is divided out of the colour");
            assert!(leaves.detail().0.is_some() && leaves.detail().1.is_some());
        } else {
            assert!(leaves.leaf_scan.is_none() && leaves.venation.is_some());
            assert!(leaves.uniform.leaf.z > 0.0);
        }

        // A leaf card without a scan has veins grown by space colonisation, in one image all
        // trees share.
        let dragon = TreePreset::DragonTree.params();
        assert_eq!(dragon.foliage.scan, None);
        let (_, a) = surfaces(&dragon, true);
        assert!(a.leaf_scan.is_none() && a.venation.is_some() && a.uniform.leaf.z > 0.0);
        assert_eq!(a.color, dragon.foliage.color);

        // Needles and cluster spheres are not leaf-shaped: no veins, no image.
        let (_, needles) = surfaces(&TreePreset::Pine.params(), true);
        assert!(needles.venation.is_none() && needles.leaf_scan.is_none() && needles.uniform.leaf.z == 0.0);

        // Without veins to draw there is no image either.
        let mut plain = dragon.clone();
        plain.foliage.shading.veins = 0.0;
        assert!(surfaces(&plain, true).1.venation.is_none());

        // Nothing is made without image assets.
        let (wood, leaves) = surfaces(&oak, false);
        assert!(wood.bark.is_none() && leaves.leaf_scan.is_none() && leaves.venation.is_none());
        assert_eq!(wood.color, oak.trunk_color);
    }

    #[test]
    fn trees_of_other_seeds_share_their_materials_and_other_scans_do_not() {
        use crate::shading::SharedFlora;
        let mut shared = SharedFlora::default();
        let mut materials = Assets::<FloraMaterial>::default();
        let wind = FloraWindBuffer(Handle::default());
        let mut resolve = |surface: &Surface| shared.resolve(&mut materials, surface, &wind);
        let (mut oak, other) = (TreePreset::Oak.params(), TreePreset::Maple.params());
        let (wood, leaves) = surfaces(&oak, true);
        oak.seed += 1;
        let (wood_again, leaves_again) = surfaces(&oak, true);
        // A different seed is the same look.
        assert_eq!(resolve(&wood), resolve(&wood_again));
        assert_eq!(resolve(&leaves), resolve(&leaves_again));
        // A different species is not.
        let (other_wood, other_leaves) = surfaces(&other, true);
        assert_ne!(resolve(&wood), resolve(&other_wood));
        assert_ne!(resolve(&leaves), resolve(&other_leaves));
    }

    #[test]
    fn launching_a_gust_stamps_the_render_clock() {
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(5.0));
        let mut wind = FloraWind::default();
        wind.launch_gust(GustFront { start: -1.0, ..Default::default() }, &time);
        let front = wind.gust.expect("the gust was launched");
        assert_eq!(front.start, time.elapsed_secs_wrapped());
        assert_eq!(front.start, 5.0);
        let [.., front_shape] = wind_vectors(&wind);
        assert_eq!(front_shape.w, 5.0);
    }
}
