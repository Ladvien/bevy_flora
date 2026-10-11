//! Leaf shading and wind: the material trees are drawn with.
//!
//! [`FloraMaterial`] is a [`StandardMaterial`] with the shaders in this directory:
//!
//! - **Leaves** get an upper side that is darker and glossier than the paler, matte lower
//!   side, a midrib and side veins, a hue of their own and darker interiors of the crown (all
//!   baked into the leaf mesh by [`foliage_mesh`](crate::foliage::foliage_mesh)), and light
//!   that comes through a leaf lit from behind: the wrapped back-side `N·L` of Unreal's
//!   two-sided foliage model times a forward-scattering GGX lobe and the leaf colour, so the
//!   crown glows yellow-green toward the sun and stays dark where other leaves shade it.
//! - **Wind** ([`FloraWind`]) bends a tree as a whole, more toward its top, and flutters each
//!   leaf (Crysis' vegetation animation, GPU Gems 3 chapter 16). A species answers it in its
//!   own way ([`WindResponse`](crate::WindResponse)), and a [`GustFront`] sweeps a band of
//!   stronger wind across the world. It runs in the vertex stage of the main pass and of the
//!   shadow maps alike, so shadows follow the motion, and costs the CPU nothing per frame: only
//!   [`FloraWind`] changing rewrites the materials.
//!
//! The plugin uses it for every tree it draws ([`ProceduralTree`](crate::ProceduralTree),
//! [`GpuGrowth`](crate::GpuGrowth), [`TreePool`](crate::TreePool) and its impostors).

use bevy::app::App;
use bevy::asset::{Asset, AssetApp, AssetServer, Assets, Handle, embedded_asset};
use bevy::color::Color;
use bevy::ecs::prelude::*;
use bevy::log::warn;
use bevy::material::OpaqueRendererMethod;
use bevy::math::{Vec2, Vec4};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
    MaterialPlugin, PbrPlugin, StandardMaterial,
};
use bevy::reflect::Reflect;
use bevy::render::RenderApp;
use bevy::render::render_resource::{
    AsBindGroup, Face, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use bevy::time::Time;

use crate::gpu::GrowthMaterial;
use crate::impostor::ImpostorMaterial;
use crate::mesh::ATTRIBUTE_LEAF_SWAY;
use crate::params::{LeafStyle, TreeParams};

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
/// faster; how far and how fast each species does is [`TreeParams::wind`]. It is global: change
/// it and every flora material is rewritten once (two per [`ProceduralTree`](crate::ProceduralTree),
/// two per [`TreePool`](crate::TreePool), one per impostor variant), with nothing else per frame.
/// A game that animates it every frame therefore pays that many material re-preparations a
/// frame; for a wind that varies, prefer the shaders' own gusts (`gust_strength`) and
/// [`GustFront`]s. Set `strength` to 0 for a still scene (the gallery does, so its captures
/// repeat).
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

/// What the flora shaders read from a material: the wind, the species' response to it and the
/// look of the leaves of the tree it belongs to.
///
/// `wind`, `motion`, `rate`, `front` and `front_shape` are the same for every tree
/// ([`FloraWind`]); `tree` and `response` are the tree's own, and so are `leaf` and `tint`.
#[derive(ShaderType, Reflect, Clone, Copy, Debug, PartialEq, Default)]
pub struct FloraUniform {
    /// (direction x, direction z, strength, speed) of the wind.
    pub wind: Vec4,
    /// (bend, flutter, gust strength, turbulence).
    pub motion: Vec4,
    /// (gust frequency, flutter frequency, 0, 0).
    pub rate: Vec4,
    /// (origin x, origin z, direction x, direction z) of the [`GustFront`]; zero without one.
    pub front: Vec4,
    /// (speed, width, strength, start time) of the [`GustFront`]; zero without one.
    pub front_shape: Vec4,
    /// (1 / trunk height, 0, 0, 0).
    pub tree: Vec4,
    /// (sway, flutter, sway frequency, flutter frequency) of the species: [`TreeParams::wind`].
    pub response: Vec4,
    /// (translucency, roughness of the upper side, veins, underside) of the leaves.
    pub leaf: Vec4,
    /// Linear rgb that light takes on passing through a leaf; w is 0.
    pub tint: Vec4,
}

impl FloraUniform {
    pub(crate) fn new(params: &TreeParams, wind: &FloraWind) -> Self {
        let s = &params.foliage.shading;
        let r = &params.wind;
        // The veins are drawn on leaf-shaped cards; the other styles' UVs are not a leaf.
        let veined = !matches!(params.foliage.style, LeafStyle::NeedleCluster | LeafStyle::ClusterSphere);
        let tint = s.transmission_tint.to_linear();
        let [wind, motion, rate, front, front_shape] = global_wind(wind);
        Self {
            wind,
            motion,
            rate,
            front,
            front_shape,
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
        }
    }

    /// The same wind for a tree's wood, which has no leaf look: the leaf fields are zeroed, so
    /// changing how leaves are shaded does not touch the wood's material. The wood bends like
    /// the species does.
    pub(crate) fn wood(self) -> Self {
        Self { leaf: Vec4::ZERO, tint: Vec4::ZERO, ..self }
    }

    /// Whether [`set_wind`](Self::set_wind) would change anything.
    pub(crate) fn wind_differs(&self, wind: &FloraWind) -> bool {
        [self.wind, self.motion, self.rate, self.front, self.front_shape] != global_wind(wind)
    }

    /// Rewrite the wind, keeping what belongs to the tree.
    pub(crate) fn set_wind(&mut self, wind: &FloraWind) {
        [self.wind, self.motion, self.rate, self.front, self.front_shape] = global_wind(wind);
    }
}

/// The fields of [`FloraUniform`] that every tree shares: `wind`, `motion`, `rate`, `front` and
/// `front_shape`.
fn global_wind(wind: &FloraWind) -> [Vec4; 5] {
    let d = wind.direction.normalize_or_zero();
    // A front with no direction, or none at all, is off: all zero.
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
}

/// The part of [`FloraMaterial`] that bends and shades.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, PartialEq)]
#[bind_group_data(FloraKey)]
pub struct FloraExtension {
    /// The wind and the leaves' look.
    // Binding slots below 100 belong to the base material; 100 is GPU growth's node buffer,
    // which shares this extension's fragment shader.
    #[uniform(101)]
    pub flora: FloraUniform,
    /// Leaves get the leaf surface, the light through the leaf and the flutter (their mesh must
    /// carry the attributes [`foliage_mesh`](crate::foliage::foliage_mesh) bakes); wood only bends.
    pub leaf: bool,
}

impl From<&FloraExtension> for FloraKey {
    fn from(extension: &FloraExtension) -> Self {
        Self { leaf: extension.leaf }
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
        specialize_flora(descriptor, layout, key.bind_group_data.leaf)
    }
}

/// Make the pipeline of a leaf material read the leaf mesh's flutter attribute and turn on the
/// leaf branches of the shaders. Wood needs nothing.
pub(crate) fn specialize_flora(
    descriptor: &mut RenderPipelineDescriptor,
    layout: &MeshVertexBufferLayoutRef,
    leaf: bool,
) -> Result<(), SpecializedMeshPipelineError> {
    if !leaf {
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

pub(crate) fn flora_material(color: Color, flora: FloraUniform, leaf: bool) -> FloraMaterial {
    FloraMaterial { base: flora_base(color), extension: FloraExtension { flora, leaf } }
}

/// Bring `handle`'s colour and uniform up to date, marking it modified only if one differs: a
/// modified material is re-extracted and re-prepared by the renderer, which a growing tree would
/// otherwise cause every frame.
pub(crate) fn sync_flora(
    materials: &mut Assets<FloraMaterial>,
    handle: &Handle<FloraMaterial>,
    color: Color,
    flora: FloraUniform,
) {
    if let Some(mut material) = materials.get_mut(handle)
        && (material.base.base_color != color || material.extension.flora != flora)
    {
        material.base.base_color = color;
        material.extension.flora = flora;
    }
}

/// Whether [`FloraMaterial`] was registered with the renderer, which only happens if
/// [`FloraPlugin`](crate::FloraPlugin) was added after the renderer's plugins. While it is
/// missing, trees are drawn with plain materials. Insert it yourself only where there is no
/// renderer (a headless test) and the asset type was registered by hand.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FloraMaterialSupport;

/// A material type that carries a [`FloraUniform`].
pub(crate) trait HasFlora: Asset {
    fn flora(&self) -> &FloraUniform;
    fn flora_mut(&mut self) -> &mut FloraUniform;
}

impl HasFlora for FloraMaterial {
    fn flora(&self) -> &FloraUniform {
        &self.extension.flora
    }
    fn flora_mut(&mut self) -> &mut FloraUniform {
        &mut self.extension.flora
    }
}

impl HasFlora for GrowthMaterial {
    fn flora(&self) -> &FloraUniform {
        &self.extension.flora
    }
    fn flora_mut(&mut self) -> &mut FloraUniform {
        &mut self.extension.flora
    }
}

impl HasFlora for ImpostorMaterial {
    fn flora(&self) -> &FloraUniform {
        &self.flora
    }
    fn flora_mut(&mut self) -> &mut FloraUniform {
        &mut self.flora
    }
}

/// Write `wind` into every material of `assets` that does not have it yet.
fn rewrite_wind<A: HasFlora>(assets: Option<ResMut<Assets<A>>>, wind: &FloraWind) {
    let Some(mut assets) = assets else { return };
    let stale: Vec<_> = assets
        .iter()
        .filter(|(_, material)| material.flora().wind_differs(wind))
        .map(|(id, _)| id)
        .collect();
    for id in stale {
        if let Some(mut material) = assets.get_mut(id) {
            material.flora_mut().set_wind(wind);
        }
    }
}

/// Push [`FloraWind`] into the materials of every tree. Runs when the wind changed.
pub(crate) fn sync_wind(
    wind: Res<FloraWind>,
    flora: Option<ResMut<Assets<FloraMaterial>>>,
    growth: Option<ResMut<Assets<GrowthMaterial>>>,
    impostors: Option<ResMut<Assets<ImpostorMaterial>>>,
) {
    rewrite_wind(flora, &wind);
    rewrite_wind(growth, &wind);
    rewrite_wind(impostors, &wind);
}

/// Register the wind, and, when the renderer's plugins have already been added, the material
/// and its shaders.
///
/// Plugins may be added in any order. Added ahead of the renderer's (or of the asset plugin),
/// this registers nothing that needs them; [`finish`] covers the asset type, and trees are drawn
/// with plain materials.
pub(crate) fn build(app: &mut App) {
    app.init_resource::<FloraWind>().register_type::<FloraWind>().register_type::<GustFront>();
    if app.get_sub_app(RenderApp).is_some() && app.is_plugin_added::<PbrPlugin>() {
        add_shaders(app);
        app.add_plugins(MaterialPlugin::<FloraMaterial>::default())
            .insert_resource(FloraMaterialSupport);
    } else {
        add_asset_types(app);
    }
}

/// Once every plugin is built, make sure the asset type the systems take exists even if the
/// plugin was added too early to register it.
pub(crate) fn finish(app: &mut App) {
    add_asset_types(app);
    if app.get_sub_app(RenderApp).is_some() && !app.world().contains_resource::<FloraMaterialSupport>() {
        warn!(
            "bevy_flora: FloraPlugin was added before the renderer's plugins, so trees are drawn \
             without leaf shading and wind; add it after DefaultPlugins"
        );
    }
}

/// Register [`FloraMaterial`] as an asset where the asset plugin is present and nobody has yet
/// (the renderer's plugins do it when they are).
fn add_asset_types(app: &mut App) {
    if app.world().contains_resource::<AssetServer>()
        && !app.world().contains_resource::<Assets<FloraMaterial>>()
    {
        app.init_asset::<FloraMaterial>();
    }
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
        let wind = FloraWind::default();
        let veins = |style| {
            let mut params = TreePreset::Oak.params();
            params.foliage.style = style;
            params.foliage.shading.veins = 0.7;
            FloraUniform::new(&params, &wind).leaf.z
        };
        assert_eq!(veins(LeafStyle::CrossedPlanes), 0.7);
        assert_eq!(veins(LeafStyle::SingleQuad), 0.7);
        assert_eq!(veins(LeafStyle::NeedleCluster), 0.0);
        assert_eq!(veins(LeafStyle::ClusterSphere), 0.0);
    }

    #[test]
    fn the_wind_is_rewritten_without_touching_the_tree() {
        let params = TreePreset::Willow.params();
        let calm = FloraUniform::new(&params, &FloraWind { strength: 0.0, ..Default::default() });
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
        assert!(calm.wind_differs(&gale));

        let mut uniform = calm;
        uniform.set_wind(&gale);
        assert!(!uniform.wind_differs(&gale));
        // Direction normalised, a negative speed stopped, strength not capped, the rest clamped.
        assert_eq!(uniform.wind, Vec4::new(0.0, -1.0, 7.0, 0.0));
        assert_eq!(uniform.motion, Vec4::new(3.0, 2.0, 1.0, 0.0));
        assert_eq!(uniform.rate, Vec4::new(0.5, 4.0, 0.0, 0.0));
        // What belongs to the tree stays.
        assert_eq!(
            (uniform.tree, uniform.response, uniform.leaf, uniform.tint),
            (calm.tree, calm.response, calm.leaf, calm.tint)
        );
        assert_eq!(uniform.response, Vec4::new(1.4, 1.6, 0.7, 0.8));
        assert_eq!(uniform, FloraUniform::new(&params, &gale));
    }

    #[test]
    fn gust_front_is_packed_normalised_and_off_when_absent() {
        let params = TreeParams::default();
        let pack = |gust: Option<GustFront>| FloraUniform::new(&params, &FloraWind { gust, ..Default::default() });

        let none = pack(None);
        assert_eq!((none.front, none.front_shape), (Vec4::ZERO, Vec4::ZERO));

        let front = GustFront {
            origin: Vec2::new(4.0, -2.0),
            direction: Vec2::new(0.0, 3.0),
            speed: 12.0,
            width: 0.0,
            strength: 2.0,
            start: 7.5,
        };
        let on = pack(Some(front));
        assert_eq!(on.front, Vec4::new(4.0, -2.0, 0.0, 1.0));
        assert_eq!(on.front_shape, Vec4::new(12.0, 0.01, 2.0, 7.5));

        // No direction, no front.
        let still = pack(Some(GustFront { direction: Vec2::ZERO, ..front }));
        assert_eq!((still.front, still.front_shape), (Vec4::ZERO, Vec4::ZERO));
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
    }
}
