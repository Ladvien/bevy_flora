//! Gliding recolours: [`RecolorPool::glide`](crate::RecolorPool::glide).
//!
//! A look glides on the GPU: every mesh of a pooled tree (and of pooled deadfall) carries its
//! look's [`MeshTag`](bevy::mesh::MeshTag), and while the look glides the shaders multiply its
//! wood and leaf colours by what the shared buffer holds for that tag, the frame's blend over the
//! colours the look's materials hold. A frame of a glide writes a few vectors of that buffer in
//! place and touches no material: a material written every frame would have Bevy re-extract and
//! re-specialize every mesh drawn with it (+60 ms of CPU a frame for 20 000 pooled trees). When the
//! glide arrives the materials take the new colours once, as a recolour at once does, and the tag's
//! multipliers go back to 1 in the same frame. A look without a tag (past [`MAX_GLIDE_LOOKS`], or
//! on a device without flora materials) glides by a recolour at once every frame instead.

use bevy::asset::{AssetId, Assets};
use bevy::color::{Color, LinearRgba, Mix};
use bevy::ecs::message::{MessageCursor, Messages};
use bevy::ecs::prelude::*;
use bevy::platform::collections::HashMap;
use bevy::render::storage::ShaderBuffer;
use bevy::time::Time;

use crate::condition::TreeCondition;
use crate::felling::TreeStump;
use crate::pool::{PoolColors, PoolRecolors, RecolorPool, TreePool};
use crate::push::{PUSH_SLOT, PUSH_VECTORS};
use crate::recolor::{FromPool, RecolorTree};
use crate::shading::{FloraMaterialSupport, FloraWindBuffer};

/// How many looks (of all pools) can glide on the GPU; the ones after glide by a recolour at once
/// every frame.
pub(crate) const MAX_GLIDE_LOOKS: usize = 64;

/// The shared buffer's first glide vector, after the pushers'. `common.wesl`'s `GLIDE` is this
/// number.
pub(crate) const GLIDE_SLOT: usize = PUSH_SLOT + PUSH_VECTORS;

/// The glide vectors: a header (x 1 while a look glides, y 1 while any tree has a `TreeCondition`)
/// and two per tag, what the look's wood and
/// its leaves are multiplied by (linear rgb).
pub(crate) const GLIDE_VECTORS: usize = 1 + 2 * MAX_GLIDE_LOOKS;

/// The glide vectors as the shaders read them; `sync_wind` keeps them when it rewrites the
/// weather.
#[derive(Resource, Clone, Debug, PartialEq)]
pub(crate) struct GlideTints(pub(crate) [[f32; 4]; GLIDE_VECTORS]);

impl Default for GlideTints {
    fn default() -> Self {
        let mut vectors = [[1.0; 4]; GLIDE_VECTORS];
        vectors[0] = [0.0; 4];
        Self(vectors)
    }
}

/// The tag of each look of each pool, given as its trees are first drawn: tag `k` (1 to
/// [`MAX_GLIDE_LOOKS`]) names the glide vectors `2k - 1` and `2k`; 0 is no look.
#[derive(Resource, Default)]
pub(crate) struct GlideSlots(HashMap<(AssetId<TreePool>, u32), u32>);

impl GlideSlots {
    /// The tag of `look` (taken modulo the pool's looks by the caller) of `pool`, given on first
    /// use; 0 once every tag is taken.
    pub(crate) fn tag(&mut self, pool: AssetId<TreePool>, look: u32) -> u32 {
        if let Some(&tag) = self.0.get(&(pool, look)) {
            return tag;
        }
        if self.0.len() >= MAX_GLIDE_LOOKS {
            return 0;
        }
        let tag = self.0.len() as u32 + 1;
        self.0.insert((pool, look), tag);
        tag
    }

    fn get(&self, pool: AssetId<TreePool>, look: u32) -> u32 {
        self.0.get(&(pool, look)).copied().unwrap_or(0)
    }
}

/// A look gliding to new colours.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Glide {
    /// The colours the look's materials hold (a glide on the GPU writes none).
    base: PoolColors,
    from: PoolColors,
    to: PoolColors,
    start: f32,
    seconds: f32,
    /// The look's tag, 0 to glide by recolouring the materials every frame.
    tag: u32,
}

impl Glide {
    /// How far it has come at `now`, 0 to 1.
    fn share(&self, now: f32) -> f32 {
        ((now - self.start) / self.seconds).clamp(0.0, 1.0)
    }

    /// Its colours at `now`: blended in linear RGB, lit or unlit as it started until it arrives,
    /// and exactly the new colours once it has.
    fn at(&self, now: f32) -> PoolColors {
        let t = self.share(now);
        if t >= 1.0 {
            return self.to;
        }
        let mix = |a: Color, b: Color| Color::from(LinearRgba::from(a).mix(&LinearRgba::from(b), t));
        PoolColors { wood: mix(self.from.wood, self.to.wood), leaves: mix(self.from.leaves, self.to.leaves), unlit: self.from.unlit }
    }
}

/// What `base` is multiplied by to show `now`, per linear channel: at most 1000, and 1 where
/// `base` is black (a black material cannot be brightened by a factor).
fn ratio(now: Color, base: Color) -> [f32; 4] {
    let (now, base) = (LinearRgba::from(now), LinearRgba::from(base));
    let channel = |n: f32, b: f32| if b > 1.0e-5 { (n / b).clamp(0.0, 1000.0) } else { 1.0 };
    [channel(now.red, base.red), channel(now.green, base.green), channel(now.blue, base.blue), 1.0]
}

/// Keep every gliding look moving. A [`RecolorPool`] with a glide starts one from the colours the
/// look shows (a glide's in-between included); one without stops the look's glide. On the GPU a
/// frame writes the look's multipliers into [`GlideTints`] and the shared buffer, and its felled
/// wood ([`FromPool`]) its blend; when it arrives the look is recoloured at once to the new
/// colours. Without a tag a frame recolours the look at once to its blend. Every other reader of
/// the message skips the gliding one.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn glide_looks(
    mut messages: ResMut<Messages<RecolorPool>>,
    mut cursor: Local<MessageCursor<RecolorPool>>,
    mut glides: Local<HashMap<(AssetId<TreePool>, u32), Glide>>,
    pools: Res<Assets<TreePool>>,
    recolors: Res<PoolRecolors>,
    slots: Res<GlideSlots>,
    support: Option<Res<FloraMaterialSupport>>,
    mut tints: ResMut<GlideTints>,
    buffer: Option<Res<FloraWindBuffer>>,
    mut buffers: Option<ResMut<Assets<ShaderBuffer>>>,
    mut felled: MessageWriter<RecolorTree>,
    left: Query<(Entity, &FromPool, Option<&TreeStump>)>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    let mut wanted = tints.0;
    for message in cursor.read(&messages) {
        let Some(pool) = pools.get(message.pool) else { continue };
        let look = message.look % pool.looks.len().max(1) as u32;
        let key = (message.pool, look);
        let running = glides.remove(&key);
        if let Some(stopped) = running.filter(|glide| glide.tag > 0) {
            // A recolour at once (or a new glide's first frame) sets the look from here.
            reset(&mut wanted, stopped.tag);
        }
        if message.glide > 0.0 {
            let shown = recolors.colors(message.pool, look, pool);
            let tag = if support.is_some() { slots.get(message.pool, look) } else { 0 };
            let (base, from) = match running {
                // The materials still hold the colours the running glide started over.
                Some(glide) if glide.tag > 0 && tag > 0 => (glide.base, glide.at(now)),
                Some(glide) => (shown, glide.at(now)),
                None => (shown, shown),
            };
            glides.insert(key, Glide { base, from, to: message.colors, start: now, seconds: message.glide, tag });
        }
    }
    let mut writes = Vec::new();
    glides.retain(|&(pool, look), glide| {
        let colors = glide.at(now);
        let arrived = glide.share(now) >= 1.0;
        if glide.tag == 0 || arrived {
            writes.push(RecolorPool { pool, look, colors, glide: 0.0 });
            if glide.tag > 0 {
                reset(&mut wanted, glide.tag);
            }
            return !arrived;
        }
        let k = glide.tag as usize;
        wanted[2 * k - 1] = ratio(colors.wood, glide.base.wood);
        wanted[2 * k] = ratio(colors.leaves, glide.base.leaves);
        for (entity, from, stump) in &left {
            // A stump whose tree is still there is reached through the tree.
            if (from.pool, from.look) == (pool, look) && !stump.and_then(|s| s.tree).is_some_and(|tree| left.contains(tree)) {
                felled.write(RecolorTree { tree: entity, colors });
            }
        }
        true
    });
    wanted[0][0] = if glides.values().any(|glide| glide.tag > 0) { 1.0 } else { 0.0 };
    for write in writes {
        messages.write(write);
    }
    // Its own recolours are not news to it.
    cursor.read(&messages).for_each(drop);
    if wanted == tints.0 {
        return;
    }
    tints.0 = wanted;
    write_tints(&tints, buffer.as_deref(), buffers.as_deref_mut());
}

/// Write the glide vectors into the shared buffer in place, when it exists.
fn write_tints(tints: &GlideTints, buffer: Option<&FloraWindBuffer>, buffers: Option<&mut Assets<ShaderBuffer>>) {
    let (Some(buffer), Some(buffers)) = (buffer, buffers) else { return };
    let Some(mut shader_buffer) = buffers.get_mut(&buffer.0) else { return };
    if let Some(slice) = shader_buffer.cast_slice_mut::<[f32; 4]>()
        && slice.len() >= GLIDE_SLOT + GLIDE_VECTORS
    {
        slice[GLIDE_SLOT..GLIDE_SLOT + GLIDE_VECTORS].copy_from_slice(&tints.0);
    }
}

/// Whether any tree has a [`TreeCondition`], for the shaders: the glide header's second lane is 1
/// while one does (its first is 1 while a look glides). The vertex stages read a mesh's tag only
/// while one of the two is set, so a forest of healthy trees that glides nowhere reads none: the
/// tag is per instance, the header one address for every vertex.
pub(crate) fn sync_condition_flag(
    trees: Query<(), Or<(With<TreeCondition>, With<crate::condition::ConditionShown>)>>,
    mut tints: ResMut<GlideTints>,
    buffer: Option<Res<FloraWindBuffer>>,
    mut buffers: Option<ResMut<Assets<ShaderBuffer>>>,
) {
    let any = if trees.is_empty() { 0.0 } else { 1.0 };
    if tints.0[0][1] == any {
        return;
    }
    tints.0[0][1] = any;
    write_tints(&tints, buffer.as_deref(), buffers.as_deref_mut());
}

/// The multipliers of `tag` back to 1.
fn reset(vectors: &mut [[f32; 4]; GLIDE_VECTORS], tag: u32) {
    let k = tag as usize;
    vectors[2 * k - 1] = [1.0; 4];
    vectors[2 * k] = [1.0; 4];
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::app::App;
    use bevy::asset::{AssetApp, AssetEvent, AssetPlugin, Handle};
    use bevy::mesh::{Mesh, Mesh3d, MeshTag};
    use bevy::pbr::StandardMaterial;
    use bevy::time::TimeUpdateStrategy;

    use super::*;
    use crate::lod::LodSettings;
    use crate::plugin::FloraPlugin;
    use crate::pool::{PoolMaterials, PooledLook, PooledTree, build_pool};
    use crate::presets::TreePreset;
    use crate::shading::FloraMaterial;

    const CRIMSON: PoolColors = PoolColors { wood: Color::srgb(0.30, 0.12, 0.05), leaves: Color::srgb(0.85, 0.25, 0.10), unlit: false };
    const VIOLET: PoolColors = PoolColors { wood: Color::srgb(0.40, 0.10, 0.50), leaves: Color::srgb(0.70, 0.20, 0.90), unlit: false };

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.init_asset::<bevy::image::Image>();
        app.add_plugins(FloraPlugin);
        app.insert_resource(FloraMaterialSupport);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
        app
    }

    /// An oak pool with a crimson look 1, and one tree of it in that look.
    fn planted(app: &mut App) -> (Handle<TreePool>, Entity) {
        let mut oak = TreePreset::Oak.params();
        oak.bark.scan = None;
        oak.foliage.scan = None;
        let mut pool = app.world_mut().resource_scope(|_, mut meshes: Mut<Assets<Mesh>>| {
            build_pool(&oak, 1, f32::INFINITY, &LodSettings::default(), &mut meshes)
        });
        pool.add_look(CRIMSON);
        let pool = app.world_mut().resource_mut::<Assets<TreePool>>().add(pool);
        let tree = app.world_mut().spawn((PooledTree { pool: pool.clone(), variant: 0 }, PooledLook(1))).id();
        (pool, tree)
    }

    /// The flora materials look 1 of `pool` draws with.
    fn drawn(app: &App, pool: &Handle<TreePool>) -> [AssetId<FloraMaterial>; 2] {
        let pools = app.world().resource::<Assets<TreePool>>();
        let Some(PoolMaterials::Flora { wood, leaves }) = pools.get(pool).unwrap().materials.get(1) else { panic!("flora materials") };
        [wood.id(), leaves.id()]
    }

    fn shared(app: &App) -> Vec<[f32; 4]> {
        let handle = &app.world().resource::<FloraWindBuffer>().0;
        let buffers = app.world().resource::<Assets<ShaderBuffer>>();
        buffers.get(handle).unwrap().cast_slice::<[f32; 4]>().unwrap()[GLIDE_SLOT..GLIDE_SLOT + GLIDE_VECTORS].to_vec()
    }

    #[test]
    fn the_shaders_are_told_while_any_tree_has_a_condition_and_not_otherwise() {
        use crate::ProceduralTree;
        let mut app = app();
        let flag = |app: &App| shared(app)[0][1];
        app.update();
        app.update();
        assert_eq!(flag(&app), 0.0, "no tree has a condition");
        let tree = app.world_mut().spawn(ProceduralTree(TreePreset::Oak.params())).id();
        app.update();
        app.update();
        assert_eq!(flag(&app), 0.0, "a tree with none");
        app.world_mut().entity_mut(tree).insert(TreeCondition::Sick);
        app.update();
        assert_eq!(flag(&app), 1.0, "one sick tree");
        assert_eq!(shared(&app)[0][0], 0.0, "and no look glides");
        // The weather rewritten (a season change) keeps it; so does a healthy tree beside the sick one.
        app.world_mut().resource_mut::<crate::FloraSeason>().autumn = 0.5;
        app.world_mut().spawn(ProceduralTree(TreePreset::Oak.params()));
        app.update();
        app.update();
        assert_eq!(flag(&app), 1.0);
        // Healthy is still a condition (the component is there); taken off or despawned, none is.
        app.world_mut().entity_mut(tree).insert(TreeCondition::Healthy);
        app.update();
        assert_eq!(flag(&app), 1.0);
        app.world_mut().entity_mut(tree).despawn();
        app.update();
        assert_eq!(flag(&app), 0.0);
    }

    #[test]
    fn a_glide_on_the_gpu_writes_only_the_shared_buffer_and_arrives_in_the_materials() {
        let mut app = app();
        let (pool, tree) = planted(&mut app);
        // A second pool built alike: until one of them glides, the two draw with the same materials.
        let (twin, _) = planted(&mut app);
        app.update();
        app.update();
        assert_eq!(drawn(&app, &pool), drawn(&app, &twin), "alike, the two share");
        let before = drawn(&app, &pool);
        let tags: Vec<u32> = app.world().get::<Children>(tree).unwrap().iter().filter_map(|c| app.world().get::<MeshTag>(c).map(|t| t.value)).collect();
        let meshes = app.world().get::<Children>(tree).unwrap().iter().filter(|c| app.world().get::<Mesh3d>(*c).is_some()).count();
        let k = tags[0] as usize;
        assert!(k >= 1 && tags.len() == meshes && tags.iter().all(|t| *t as usize == k), "every mesh carries the look's tag: {tags:?}");

        app.world_mut().write_message(RecolorPool { pool: pool.id(), look: 1, colors: VIOLET, glide: 1.0 });
        let mut events = app.world().resource::<Messages<AssetEvent<FloraMaterial>>>().get_cursor_current();
        // Read on the first frame; half of it five frames of 0.1 s later.
        for _ in 0..6 {
            app.update();
        }
        let modified = events
            .read(app.world().resource::<Messages<AssetEvent<FloraMaterial>>>())
            .filter(|e| matches!(e, AssetEvent::Modified { .. }))
            .count();
        assert_eq!(modified, 0, "a glide on the GPU writes no material");
        assert_eq!(drawn(&app, &pool), before);
        let half = |a: Color, b: Color| Color::from(LinearRgba::from(a).mix(&LinearRgba::from(b), 0.5));
        let tints = app.world().resource::<GlideTints>().0;
        assert_eq!(tints[0][0], 1.0, "a look glides");
        for (got, want) in [(tints[2 * k - 1], ratio(half(CRIMSON.wood, VIOLET.wood), CRIMSON.wood)), (tints[2 * k], ratio(half(CRIMSON.leaves, VIOLET.leaves), CRIMSON.leaves))] {
            assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1.0e-4), "halfway: {got:?}, wanted {want:?}");
        }
        assert_eq!(shared(&app), tints.to_vec(), "the shaders read them");
        let other = tints.iter().enumerate().filter(|(i, _)| *i > 0 && *i != 2 * k - 1 && *i != 2 * k).all(|(_, v)| *v == [1.0; 4]);
        assert!(other, "no other look moves");

        // Arrived: the materials take the new colours, the multipliers are 1 again.
        for _ in 0..6 {
            app.update();
        }
        let tints = app.world().resource::<GlideTints>().0;
        assert_eq!(tints, GlideTints::default().0);
        assert_eq!(shared(&app), tints.to_vec());
        assert_ne!(drawn(&app, &pool), before, "the look's materials changed");
        // They are the materials of the new colours: the twin, recoloured at once, takes them.
        app.world_mut().write_message(RecolorPool { pool: twin.id(), look: 1, colors: VIOLET, glide: 0.0 });
        app.update();
        assert_eq!(drawn(&app, &twin), drawn(&app, &pool));
    }
}
