//! Leaves that fall from the trees: knocked off by a shake ([`TreeShakes`]), shed by a felled tree
//! as it lands ([`TreeLanded`]), or dropped on demand ([`DropLeaves`]).
//!
//! A drop copies some of the tree's own leaf cards into a burst ([`LeafBurst`]): one entity with
//! one small mesh, drawn with a copy of the tree's leaf material ([`FloraExtension::falling`]),
//! whose vertex shader moves every card on the GPU. A leaf hangs where it grew until its moment
//! (the leaves of a drop let go over half a second), falls, picking up speed and tumbling, swings
//! from side to side, drifts with the [`FloraWind`], turns flat as it reaches the ground
//! ([`FloraGround`], or the height the tree stands at), lies there for about
//! [`FallingLeaves::rest`] seconds and shrinks away. Nothing moves on the CPU; the burst is
//! despawned when its last leaf is gone. The leaves
//! look like those still on the tree (the scan, the tint, the season's colour, snow) because they
//! are drawn by the same fragment shader. The tree keeps its own leaves: a few falling copies are
//! not missed from a crown of thousands.
//!
//! [`FallingLeaves`] sets how many fall and how long they lie. Trees drawn on the CPU
//! ([`ProceduralTree`]) and from a pool ([`PooledTree`]) drop leaves; a tree grown on the GPU
//! ([`GpuGrowth`](crate::GpuGrowth)), one drawn without [`FloraMaterial`]s and a leafless one
//! drop none, and a deciduous tree drops none of those it has shed this season.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy_flora::prelude::*;
//!
//! // A bird lands in the oak.
//! fn bird_lands(mut drops: MessageWriter<DropLeaves>, oak: Single<Entity, With<ProceduralTree>>) {
//!     drops.write(DropLeaves { tree: *oak, count: 6 });
//! }
//! ```

use std::ops::Range;
use std::sync::RwLock;

use bevy::asset::{AssetEvent, AssetId, Assets, Handle, RenderAssetUsages};
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::Visibility;
use bevy::ecs::prelude::*;
use bevy::math::{Vec2, Vec3, Vec3Swizzles};
use bevy::mesh::{
    Indices, Mesh, Mesh3d, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology, VertexAttributeValues,
    VertexFormat,
};
use bevy::pbr::MeshMaterial3d;
use bevy::platform::collections::HashMap;
use bevy::reflect::Reflect;
use bevy::render::render_resource::{RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::shader::{Shader, ShaderDefVal};
use bevy::time::Time;
use bevy::transform::components::{GlobalTransform, Transform};

use crate::felling::{Felled, TreeLanded};
use crate::ground::FloraGround;
use crate::mesh::ATTRIBUTE_LEAF_SWAY;
use crate::plugin::{Paint, ProceduralTree, TreeRender};
use crate::pool::{PoolMaterials, PooledTree, TreePool};
use crate::rng::SeededRng;
use crate::shading::{FloraExtension, FloraMaterial, FloraSeason, FloraWind, TreeShake, TreeShakes};

/// How many leaves fall and how long they lie. A resource; the plugin adds the default.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct FallingLeaves {
    /// Leaves a shake of strength 1 knocks off a tree standing where it happens ([`TreeShakes`]);
    /// fewer the further the tree stands from it, none at its radius. 0 knocks none off.
    pub per_shake: f32,
    /// Leaves a felled tree sheds as it lands ([`TreeLanded`]). 0 sheds none.
    pub per_landing: u32,
    /// Seconds a fallen leaf lies on the ground before it shrinks away, each a quarter more or less.
    pub rest: f32,
    /// Most leaves falling or lying at once; a drop past it drops fewer, or none.
    pub max_leaves: u32,
}

impl Default for FallingLeaves {
    fn default() -> Self {
        Self { per_shake: 12.0, per_landing: 80, rest: 6.0, max_leaves: 5000 }
    }
}

/// Make `count` of `tree`'s leaves fall now, chosen at random (fewer if it has fewer, or if
/// [`FallingLeaves::max_leaves`] would be passed). Write it for a bird that lands, a branch that is
/// hit, a gust; the plugin writes it for shakes and landings. See the [module documentation](self).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropLeaves {
    /// The tree: a [`ProceduralTree`] or a [`PooledTree`].
    pub tree: Entity,
    /// How many leaves.
    pub count: u32,
}

/// A drop of falling leaves: one entity with one mesh, drawn with a copy of its tree's leaf material
/// that moves them ([`FloraExtension::falling`]), standing where its tree stood when they let go,
/// and despawned when the last of them is gone.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct LeafBurst {
    /// The tree they fell from.
    pub tree: Entity,
    /// How many leaves.
    pub leaves: u32,
    /// When the last of them is gone, in [`Time::elapsed_secs`].
    pub gone_at: f32,
}

/// Per falling-leaf vertex: where its card's centre hangs in the burst's space (xyz) and the render
/// time it lets go (w). See `falling_common.wesl`.
pub(crate) const ATTRIBUTE_FALL_ANCHOR: MeshVertexAttribute =
    MeshVertexAttribute::new("FallAnchor", 0x666C_6F72_6120, VertexFormat::Float32x4);
/// Per falling-leaf vertex: the ground's height under where it lands (burst space), its seed, how
/// long it lies on the ground and how fast it falls.
pub(crate) const ATTRIBUTE_FALL_PATH: MeshVertexAttribute =
    MeshVertexAttribute::new("FallPath", 0x666C_6F72_6121, VertexFormat::Float32x4);
/// Per falling-leaf vertex: its card's face normal in the burst's space (xyz; w unused).
pub(crate) const ATTRIBUTE_FALL_FACE: MeshVertexAttribute =
    MeshVertexAttribute::new("FallFace", 0x666C_6F72_6122, VertexFormat::Float32x4);

/// The vertex shaders of falling leaves, main pass and prepass (`falling.wesl`,
/// `falling_prepass.wesl`), kept once the shaders are embedded.
static SHADERS: RwLock<Option<(Handle<Shader>, Handle<Shader>)>> = RwLock::new(None);

/// Keep the handles of the falling leaves' vertex shaders for [`specialize_falling`].
pub(crate) fn set_shaders(main: Handle<Shader>, prepass: Handle<Shader>) {
    if let Ok(mut slot) = SHADERS.write() {
        *slot = Some((main, prepass));
    }
}

/// The pipeline of a falling-leaf material ([`FloraExtension::falling`]): its vertex stage moves the
/// cards of a burst (`falling.wesl`, or `falling_prepass.wesl` in a prepass or shadow pipeline, so
/// their shadows fall with them) and reads the fall's attributes after the leaf's sway (11).
pub(crate) fn specialize_falling(
    descriptor: &mut RenderPipelineDescriptor,
    layout: &MeshVertexBufferLayoutRef,
) -> Result<(), SpecializedMeshPipelineError> {
    let Some((main, prepass)) = SHADERS.read().ok().and_then(|shaders| shaders.clone()) else { return Ok(()) };
    let in_prepass = descriptor.vertex.shader_defs.contains(&ShaderDefVal::from("PREPASS_PIPELINE"));
    descriptor.vertex.shader = if in_prepass { prepass } else { main };
    let fall = layout.0.get_layout(&[
        ATTRIBUTE_FALL_ANCHOR.at_shader_location(12),
        ATTRIBUTE_FALL_PATH.at_shader_location(13),
        ATTRIBUTE_FALL_FACE.at_shader_location(14),
    ])?;
    descriptor.vertex.buffers[0].attributes.extend(fall.attributes);
    Ok(())
}

// The fall's timing, as in `falling_common.wesl`.
/// Seconds a leaf takes to pick up most of its speed.
const PICK_UP: f32 = 0.35;
/// How fast the wind carries a leaf across, metres per second at strength 1.
const DRIFT: f32 = 1.5;
/// Seconds a leaf takes to shrink away.
const SHRINK: f32 = 1.0;
/// Most a leaf swings to one side, metres.
const SWING: f32 = 0.4;

/// Seconds over which the leaves of one drop let go.
const LET_GO: f32 = 0.5;
/// How fast a leaf falls once it has picked up speed, metres per second: each its own in this range.
const SPEED: Range<f32> = 0.9..1.5;
/// A leaf that hangs less than this above the ground (a felled tree's crown lies on it) stays put.
const LOWEST: f32 = 0.05;
/// Leaf meshes whose cards [`Caches`] keeps, at most: a pool's few variants, the trees that drop
/// leaves often.
const CACHED_MESHES: usize = 64;

/// What `drop_leaves` keeps between drops: the falling-leaf material made from each leaf material,
/// and the cards of each leaf mesh it has read.
#[derive(Default)]
pub(crate) struct Caches {
    materials: HashMap<AssetId<FloraMaterial>, Handle<FloraMaterial>>,
    cards: HashMap<AssetId<Mesh>, MeshCards>,
}

/// The cards of a leaf mesh, and its vertex and index counts when they were read.
struct MeshCards {
    size: (usize, usize),
    cards: Vec<Card>,
}

/// Seconds a leaf that lets go `height` metres above the ground takes to reach it.
pub(crate) fn landing_time(height: f32, speed: f32) -> f32 {
    if height <= 0.0 {
        return 0.0;
    }
    let mut t = height / speed + PICK_UP;
    for _ in 0..4 {
        let e = (-t / PICK_UP).exp();
        let f = speed * (t - PICK_UP * (1.0 - e)) - height;
        t -= f / (speed * (1.0 - e)).max(1.0e-3);
    }
    t.max(0.0)
}

/// `leaf_season_moment` in `common.wesl`: a leaf's moment in the season, from its phase.
fn leaf_season_moment(phase: f32) -> f32 {
    let x = phase * 7.31 + 0.137;
    x - x.floor()
}

/// One card of a tree's leaf mesh: its vertices and its triangles' indices (runs of the mesh's
/// buffers), its moment in the season and whether it is a flower.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Card {
    pub vertices: Range<usize>,
    pub indices: Range<usize>,
    /// `leaf_season_moment` of its phase: a deciduous tree sheds it once the leaf fall passes it.
    pub moment: f32,
    /// A flower (vertex alpha 0), not a leaf.
    pub flower: bool,
}

impl Card {
    /// Whether it is a leaf still on the tree, given the season's leaf fall of a deciduous tree.
    pub fn on_tree(&self, shed: Option<f32>) -> bool {
        !self.flower && shed.is_none_or(|shed| self.moment >= shed)
    }
}

/// The cards of `mesh`, a tree's leaf mesh, each with its triangles. The foliage mesher keeps each
/// leaf's vertices and triangles together and gives the leaf one phase (`sway.w`), which is what
/// tells one card from the next.
pub(crate) fn leaf_cards(mesh: &Mesh) -> Vec<Card> {
    let Some(VertexAttributeValues::Float32x4(sway)) = mesh.attribute(ATTRIBUTE_LEAF_SWAY) else {
        return Vec::new();
    };
    let Some(indices) = mesh.indices() else { return Vec::new() };
    let colors = match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(VertexAttributeValues::Float32x4(colors)) => Some(colors),
        _ => None,
    };
    // Runs of vertices with one phase.
    let mut cards: Vec<Card> = Vec::new();
    let mut card_of = Vec::with_capacity(sway.len());
    for (v, s) in sway.iter().enumerate() {
        match cards.last_mut() {
            Some(card) if sway[card.vertices.start][3].to_bits() == s[3].to_bits() => card.vertices.end = v + 1,
            _ => cards.push(Card {
                vertices: v..v + 1,
                indices: 0..0,
                moment: leaf_season_moment(s[3]),
                flower: colors.is_some_and(|colors| colors[v][3] < 0.5),
            }),
        }
        card_of.push(cards.len() - 1);
    }
    // Each card's triangles follow one another, in the order of the cards.
    let indices: Vec<usize> = indices.iter().collect();
    for (k, triangle) in indices.as_chunks::<3>().0.iter().enumerate() {
        let card = &mut cards[card_of[triangle[0]]];
        if card.indices.is_empty() {
            card.indices = 3 * k..3 * k;
        }
        card.indices.end = 3 * k + 3;
    }
    cards.retain(|card| !card.indices.is_empty());
    cards
}

/// How a drop falls: when it begins, how long its leaves lie and how the wind carries them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fall {
    /// The render time ([`Time::elapsed_secs_wrapped`]) the first leaf lets go.
    pub now: f32,
    /// [`FallingLeaves::rest`].
    pub rest: f32,
    /// The wind's drift, metres per second on the ground.
    pub drift: Vec2,
}

/// What a drop makes: the burst's mesh and bounds in the space of the burst standing at the
/// tree's position, how many leaves it holds, and how many seconds until the last is gone.
pub(crate) struct Burst {
    pub mesh: Mesh,
    pub aabb: Aabb,
    pub leaves: u32,
    pub life: f32,
}

/// The burst of `cards` of `source` (a tree's leaf mesh) for the tree placed at `tree`: every
/// vertex in world orientation and scale, relative to the tree's position, with the attributes
/// `falling_common.wesl` reads. `ground` is the ground's height at a world (x, z). `None` when no
/// card hangs above the ground.
pub(crate) fn build_burst(
    source: &Mesh,
    cards: &[&Card],
    tree: &GlobalTransform,
    ground: impl Fn(Vec2) -> f32,
    fall: Fall,
    rng: &mut SeededRng,
) -> Option<Burst> {
    let floats3 = |id| match source.attribute(id) {
        Some(VertexAttributeValues::Float32x3(v)) => Some(v),
        _ => None,
    };
    let floats4 = |id| match source.attribute(id) {
        Some(VertexAttributeValues::Float32x4(v)) => Some(v),
        _ => None,
    };
    let floats2 = |id| match source.attribute(id) {
        Some(VertexAttributeValues::Float32x2(v)) => Some(v),
        _ => None,
    };
    let positions = floats3(Mesh::ATTRIBUTE_POSITION)?;
    let normals = floats3(Mesh::ATTRIBUTE_NORMAL)?;
    let uv0 = floats2(Mesh::ATTRIBUTE_UV_0)?;
    let uv1 = floats2(Mesh::ATTRIBUTE_UV_1);
    let colors = floats4(Mesh::ATTRIBUTE_COLOR);
    let sway = floats4(ATTRIBUTE_LEAF_SWAY)?;
    let source_indices: Vec<usize> = source.indices()?.iter().collect();

    let origin = tree.translation();
    let affine = tree.affine();
    let mut out = BurstBuffers::default();
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    let mut life: f32 = 0.0;
    let mut leaves = 0;
    for card in cards {
        let world: Vec<Vec3> =
            card.vertices.clone().map(|v| affine.transform_point3(Vec3::from_array(positions[v])) - origin).collect();
        let anchor = world.iter().sum::<Vec3>() / world.len() as f32;
        let reach = world.iter().map(|p| p.distance(anchor)).fold(0.0, f32::max);
        // The ground under where it will land: under where it hangs, then where the wind takes it.
        let speed = rng.range(SPEED.start, SPEED.end);
        let at = anchor + origin;
        let mut landing = at.xz();
        let mut floor = ground(landing);
        for _ in 0..2 {
            landing = at.xz() + fall.drift * landing_time(at.y - floor, speed);
            floor = ground(landing);
        }
        let height = at.y - floor;
        if height < LOWEST {
            continue;
        }
        let land = landing_time(height, speed);
        let delay = rng.range(0.0, LET_GO);
        let rest = fall.rest.max(0.0) * rng.range(0.75, 1.25);
        let seed = rng.next_f32();
        life = life.max(delay + land + rest + SHRINK);
        leaves += 1;

        // The card's face, from its first triangle.
        let first = &source_indices[card.indices.start..card.indices.start + 3];
        let corner = |k: usize| affine.transform_point3(Vec3::from_array(positions[first[k]]));
        let face = (corner(1) - corner(0)).cross(corner(2) - corner(0)).normalize_or(Vec3::Y);

        let base = out.positions.len() as u32;
        let local_floor = floor - origin.y;
        for (k, v) in card.vertices.clone().enumerate() {
            out.positions.push(world[k].to_array());
            out.normals.push(affine.transform_vector3(Vec3::from_array(normals[v])).normalize_or(Vec3::Y).to_array());
            out.uv0.push(uv0[v]);
            if let Some(uv1) = uv1 {
                out.uv1.push(uv1[v]);
            }
            if let Some(colors) = colors {
                out.colors.push(colors[v]);
            }
            out.sway.push(sway[v]);
            out.anchor.push(anchor.extend(fall.now + delay).to_array());
            out.path.push([local_floor, seed, rest, speed]);
            out.face.push(face.extend(0.0).to_array());
        }
        let start = card.vertices.start;
        out.indices.extend(source_indices[card.indices.clone()].iter().map(|&i| base + (i - start) as u32));

        // Bounds: where it hangs and where it lands, each with the card's reach and the swing.
        let landed = Vec3::new(landing.x - origin.x, local_floor, landing.y - origin.z);
        let margin = Vec3::splat(reach + SWING);
        lo = lo.min(anchor - margin).min(landed - margin);
        hi = hi.max(anchor + margin).max(landed + margin);
    }
    if leaves == 0 {
        return None;
    }
    Some(Burst { mesh: out.into_mesh(), aabb: Aabb::from_min_max(lo, hi), leaves, life })
}

/// The vertex buffers of a burst under construction.
#[derive(Default)]
struct BurstBuffers {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uv0: Vec<[f32; 2]>,
    uv1: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    sway: Vec<[f32; 4]>,
    anchor: Vec<[f32; 4]>,
    path: Vec<[f32; 4]>,
    face: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl BurstBuffers {
    fn into_mesh(self) -> Mesh {
        // Only the GPU reads it.
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv0)
            .with_inserted_attribute(ATTRIBUTE_LEAF_SWAY, self.sway)
            .with_inserted_attribute(ATTRIBUTE_FALL_ANCHOR, self.anchor)
            .with_inserted_attribute(ATTRIBUTE_FALL_PATH, self.path)
            .with_inserted_attribute(ATTRIBUTE_FALL_FACE, self.face)
            .with_inserted_indices(Indices::U32(self.indices));
        if !self.uv1.is_empty() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, self.uv1);
        }
        if !self.colors.is_empty() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        }
        mesh
    }
}

/// `smoothstep(0, edge, x)`, as in WGSL.
fn smoothstep(edge: f32, x: f32) -> f32 {
    let t = (x / edge.max(1.0e-4)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Turn each new shake into drops from the standing trees it reaches, as many as it shakes them
/// (`shake_at` in `common.wesl`: all of its strength where it happens, none at its radius).
#[allow(clippy::type_complexity)]
pub(crate) fn shake_leaves_loose(
    shakes: Res<TreeShakes>,
    settings: Res<FallingLeaves>,
    mut seen: Local<Vec<TreeShake>>,
    trees: Query<(Entity, &GlobalTransform), (Or<(With<TreeRender>, With<PooledTree>)>, Without<Felled>)>,
    mut drops: MessageWriter<DropLeaves>,
) {
    if !shakes.is_changed() {
        return;
    }
    let new: Vec<TreeShake> = shakes.iter().filter(|shake| !seen.contains(shake)).copied().collect();
    seen.clear();
    seen.extend(shakes.iter().copied());
    if settings.per_shake <= 0.0 {
        return;
    }
    for shake in new {
        let radius = shake.radius.max(0.01);
        for (tree, at) in &trees {
            let near = 1.0 - smoothstep(radius, at.translation().xz().distance(shake.at));
            let count = (settings.per_shake * shake.strength.max(0.0) * near).round() as u32;
            if count > 0 {
                drops.write(DropLeaves { tree, count });
            }
        }
    }
}

/// A felled tree sheds leaves as it lands.
pub(crate) fn shed_landed_leaves(
    mut landed: MessageReader<TreeLanded>,
    settings: Res<FallingLeaves>,
    mut drops: MessageWriter<DropLeaves>,
) {
    for landing in landed.read() {
        if settings.per_landing > 0 {
            drops.write(DropLeaves { tree: landing.tree, count: settings.per_landing });
        }
    }
}

/// Make the bursts the drops ask for.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn drop_leaves(
    mut commands: Commands,
    mut drops: MessageReader<DropLeaves>,
    settings: Res<FallingLeaves>,
    time: Res<Time>,
    wind: Res<FloraWind>,
    season: Res<FloraSeason>,
    ground: Option<Res<FloraGround>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut flora: ResMut<Assets<FloraMaterial>>,
    pools: Option<Res<Assets<TreePool>>>,
    trees: Query<(&GlobalTransform, Option<&TreeRender>, Option<&ProceduralTree>, Option<&PooledTree>, Option<&Felled>)>,
    stumps: Query<&GlobalTransform>,
    bursts: Query<&LeafBurst>,
    mut caches: Local<Caches>,
    mut changed: MessageReader<AssetEvent<Mesh>>,
) {
    // A leaf mesh that changed (a growing tree's, every frame) is read again when it next drops.
    for event in changed.read() {
        if let AssetEvent::Modified { id } | AssetEvent::Removed { id } | AssetEvent::Unused { id } = event {
            caches.cards.remove(id);
        }
    }
    let mut live: u32 = bursts.iter().map(|burst| burst.leaves).sum();
    let fall = Fall {
        now: time.elapsed_secs_wrapped(),
        rest: settings.rest,
        drift: wind.direction.normalize_or_zero() * (wind.strength.max(0.0) * DRIFT),
    };
    for (k, drop) in drops.read().enumerate() {
        let budget = settings.max_leaves.saturating_sub(live).min(drop.count) as usize;
        if budget == 0 {
            continue;
        }
        let Ok((at, render, tree, pooled, felled)) = trees.get(drop.tree) else { continue };
        // The tree's leaf mesh and material, and whether it sheds its leaves in autumn.
        let (mesh, material, deciduous) = match (render, tree, pooled) {
            (Some(render), Some(tree), _) => {
                let (Paint::Flora { foliage, .. }, Some((_, mesh))) = (&render.paint, &render.foliage) else { continue };
                (mesh.clone(), foliage.clone(), tree.0.foliage.autumn.is_some())
            }
            (_, _, Some(pooled)) => {
                let Some(pool) = pools.as_deref().and_then(|pools| pools.get(&pooled.pool)) else { continue };
                let Some(PoolMaterials::Flora { leaves, .. }) = &pool.materials else { continue };
                let variant = &pool.variants[pooled.variant as usize % pool.variants.len()];
                let Some(mesh) = &variant.leaves[0] else { continue };
                (mesh.clone(), leaves.clone(), pool.tree_params(pooled.variant).foliage.autumn.is_some())
            }
            _ => continue,
        };
        let Some(leaf_material) = flora.get(&material) else { continue };
        let look = FloraMaterial {
            base: leaf_material.base.clone(),
            extension: FloraExtension { falling: true, ..leaf_material.extension.clone() },
        };
        let Some(source) = meshes.get(&mesh) else { continue };
        if caches.cards.len() >= CACHED_MESHES && !caches.cards.contains_key(&mesh.id()) {
            caches.cards.clear();
        }
        // Read again if the mesh is not the one read last time (its change not seen yet).
        let size = (source.count_vertices(), source.indices().map_or(0, Indices::len));
        let read = caches.cards.entry(mesh.id()).or_insert_with(|| MeshCards { size, cards: leaf_cards(source) });
        if read.size != size {
            *read = MeshCards { size, cards: leaf_cards(source) };
        }
        let shed = deciduous.then_some(season.leaf_fall);
        let mut cards: Vec<&Card> = read.cards.iter().filter(|card| card.on_tree(shed)).collect();
        if cards.is_empty() {
            continue;
        }
        // Which leaves: a few at random, by a partial shuffle.
        let mut rng = SeededRng::from_parts(drop.tree.to_bits() as i32 ^ fall.now.to_bits() as i32, k as u32);
        let (chosen, last) = (budget.min(cards.len()), cards.len() - 1);
        for i in 0..chosen {
            let j = i + (rng.next_f32() * (cards.len() - i) as f32) as usize;
            cards.swap(i, j.min(last));
        }
        let chosen = &cards[..chosen];
        // The ground: the terrain, or the height the tree stands at (a felled tree's: its stump's).
        let stood = felled.and_then(|felled| stumps.get(felled.stump).ok()).unwrap_or(at).translation().y;
        let height = |p: Vec2| ground.as_deref().map_or(stood, |ground| ground.height(p));
        let Some(burst) = build_burst(source, chosen, at, height, fall, &mut rng) else { continue };

        // The tree's leaf material, made to move falling leaves: one per look, made again when the
        // look changes.
        let handle = match caches.materials.get(&material.id()) {
            Some(handle) if flora.get(handle).is_some_and(|made| made.extension == look.extension) => handle.clone(),
            _ => {
                let handle = flora.add(look);
                caches.materials.insert(material.id(), handle.clone());
                handle
            }
        };
        live += burst.leaves;
        commands.spawn((
            Mesh3d(meshes.add(burst.mesh)),
            MeshMaterial3d(handle),
            // Spawned after this frame's transform propagation: its global transform is set here.
            Transform::from_translation(at.translation()),
            GlobalTransform::from_translation(at.translation()),
            Visibility::default(),
            burst.aabb,
            LeafBurst { tree: drop.tree, leaves: burst.leaves, gone_at: time.elapsed_secs() + burst.life },
        ));
    }
}

/// Despawn the bursts whose last leaf is gone.
pub(crate) fn expire_bursts(mut commands: Commands, time: Res<Time>, bursts: Query<(Entity, &LeafBurst)>) {
    let now = time.elapsed_secs();
    for (entity, burst) in &bursts {
        if now >= burst.gone_at {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::TreeAge;
    use bevy::pbr::StandardMaterial;
    use crate::pipeline::build_tree;
    use crate::presets::TreePreset;

    fn leaf_mesh(preset: TreePreset) -> Mesh {
        build_tree(&preset.params(), f32::INFINITY).foliage.into_bevy_mesh()
    }

    /// How far a leaf has fallen `t` seconds after letting go, at full speed `speed` (`fallen` in
    /// `falling_common.wesl`).
    fn fallen(t: f32, speed: f32) -> f32 {
        speed * (t - PICK_UP * (1.0 - (-t / PICK_UP).exp()))
    }

    #[test]
    fn a_leaf_lands_when_it_has_fallen_its_height() {
        for (height, speed) in [(0.3, 0.9), (2.0, 1.2), (8.0, 1.5), (25.0, 1.0)] {
            let t = landing_time(height, speed);
            assert!((fallen(t, speed) - height).abs() < 1.0e-3, "{height} m at {speed} m/s: {t} s");
        }
        assert_eq!(landing_time(-1.0, 1.0), 0.0);
    }

    #[test]
    fn every_leaf_is_one_card_and_a_card_holds_only_its_own_triangles() {
        for preset in [TreePreset::Oak, TreePreset::Pine, TreePreset::Maple, TreePreset::Willow] {
            let params = preset.params();
            let built = build_tree(&params, f32::INFINITY);
            let points = crate::foliage::collect_leaf_points(
                &built.skeleton,
                &params.foliage,
                &params.growth,
                &params.trunk,
                params.seed,
            );
            let mesh = built.foliage.into_bevy_mesh();
            let cards = leaf_cards(&mesh);
            assert_eq!(cards.len(), points.len(), "{preset:?}");
            let indices: Vec<usize> = mesh.indices().unwrap().iter().collect();
            for card in &cards {
                assert!(indices[card.indices.clone()].iter().all(|i| card.vertices.contains(i)), "{preset:?}");
            }
        }
    }

    #[test]
    fn a_tree_drops_neither_flowers_nor_the_leaves_it_has_shed() {
        // A pool's leaf mesh holds the flowers after the leaves.
        let built = build_tree(&TreePreset::Cherry.params(), f32::INFINITY);
        let leaves = on_tree(&built.foliage.clone().into_bevy_mesh(), None);
        let mut merged = built.foliage;
        merged.extend(&built.flowers);
        assert_eq!(on_tree(&merged.into_bevy_mesh(), None), leaves);

        let oak = leaf_mesh(TreePreset::Oak);
        let all = on_tree(&oak, None);
        let half = on_tree(&oak, Some(0.5));
        assert!((all as f32 * 0.4..all as f32 * 0.6).contains(&(half as f32)), "{half} of {all} left at half the leaf fall");
        assert_eq!(on_tree(&oak, Some(1.0)), 0);
    }

    /// How many of the cards of `mesh` are leaves on the tree at the season's leaf fall `shed`.
    fn on_tree(mesh: &Mesh, shed: Option<f32>) -> usize {
        leaf_cards(mesh).iter().filter(|card| card.on_tree(shed)).count()
    }

    #[test]
    fn a_burst_holds_the_chosen_cards_falling_to_the_ground_from_where_they_hang() {
        let oak = leaf_mesh(TreePreset::Oak);
        let cards = leaf_cards(&oak);
        let chosen: Vec<&Card> = cards.iter().step_by(97).collect();
        let tree = GlobalTransform::from(Transform::from_xyz(4.0, 1.0, -3.0).with_scale(Vec3::splat(1.5)));
        let fall = Fall { now: 10.0, rest: 6.0, drift: Vec2::new(0.5, 0.0) };
        let mut rng = SeededRng::new(3);
        let burst = build_burst(&oak, &chosen, &tree, |_| 1.0, fall, &mut rng).expect("leaves above the ground");
        assert_eq!(burst.leaves as usize, chosen.len());
        let vertices: usize = chosen.iter().map(|card| card.vertices.len()).sum();
        assert_eq!(burst.mesh.count_vertices(), vertices);
        let Some(VertexAttributeValues::Float32x4(anchor)) = burst.mesh.attribute(ATTRIBUTE_FALL_ANCHOR) else {
            panic!("no anchors")
        };
        let Some(VertexAttributeValues::Float32x4(path)) = burst.mesh.attribute(ATTRIBUTE_FALL_PATH) else {
            panic!("no paths")
        };
        let Some(VertexAttributeValues::Float32x3(positions)) = burst.mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
            panic!("no positions")
        };
        // Each card is where the tree holds it, relative to the tree; it lets go within the drop's
        // half second, lands on the ground (1 m, the tree's own height in burst space: 0) and stays
        // inside the bounds whether it hangs or lies there.
        let first = Vec3::from_array(positions[0]);
        let source = match oak.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(p)) => Vec3::from_array(p[chosen[0].vertices.start]),
            _ => unreachable!(),
        };
        assert!((first - (tree.transform_point(source) - tree.translation())).length() < 1.0e-4);
        for (a, p) in anchor.iter().zip(path) {
            assert!((10.0..10.0 + LET_GO).contains(&a[3]), "let go at {}", a[3]);
            assert_eq!(p[0], 0.0);
            assert!(SPEED.contains(&p[3]));
            let hangs = Vec3::new(a[0], a[1], a[2]);
            assert!(burst.aabb.min().cmple(hangs.into()).all() && burst.aabb.max().cmpge(hangs.into()).all());
        }
        assert!(burst.aabb.min().y <= 0.0, "the bounds reach the ground");
        let tallest = anchor.iter().map(|a| a[1]).fold(0.0, f32::max);
        assert!(burst.life > landing_time(tallest, SPEED.end) + 6.0 * 0.75);
        assert!(burst.life < LET_GO + landing_time(tallest, SPEED.start) + 6.0 * 1.25 + SHRINK + 1.0e-3);
    }

    fn app() -> bevy::app::App {
        use bevy::asset::{AssetApp, AssetPlugin};
        use bevy::time::TimeUpdateStrategy;
        let mut app = bevy::app::App::new();
        // `MinimalPlugins` leaves out transform propagation, which says where the trees stand.
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default(), bevy::transform::TransformPlugin));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(crate::FloraPlugin);
        app.insert_resource(crate::FloraMaterialSupport);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
        app
    }

    fn bursts(app: &mut bevy::app::App) -> Vec<LeafBurst> {
        app.world_mut().query::<&LeafBurst>().iter(app.world()).copied().collect()
    }

    #[test]
    fn a_shake_or_a_drop_makes_a_burst_from_the_trees_it_reaches_until_its_leaves_are_gone() {
        let mut app = app();
        let oak = TreePreset::Oak.params();
        let near = app.world_mut().spawn((ProceduralTree(oak.clone()), Transform::default())).id();
        let far = app.world_mut().spawn((ProceduralTree(oak), TreeAge(10.0), Transform::from_xyz(30.0, 0.0, 0.0))).id();
        app.update();
        app.update();
        assert!(bursts(&mut app).is_empty());

        // An axe on the near oak: the default dozen leaves, from it alone, and once.
        let time = *app.world().resource::<Time>();
        app.world_mut().resource_mut::<TreeShakes>().shake(TreeShake { at: Vec2::ZERO, ..Default::default() }, &time);
        app.update();
        app.update();
        let shaken = bursts(&mut app);
        assert_eq!(shaken.len(), 1);
        assert_eq!((shaken[0].tree, shaken[0].leaves), (near, 12));

        // A drop takes what is left under the cap.
        app.world_mut().resource_mut::<FallingLeaves>().max_leaves = 20;
        app.world_mut().resource_mut::<Messages<DropLeaves>>().write(DropLeaves { tree: far, count: 50 });
        app.update();
        let dropped: Vec<LeafBurst> = bursts(&mut app).into_iter().filter(|burst| burst.tree == far).collect();
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].leaves, 8);

        // A tree whose leaf mesh changed in the frame it drops (it is younger now: fewer leaves)
        // drops from its new leaves, not from the cards of the old mesh.
        app.world_mut().resource_mut::<FallingLeaves>().max_leaves = 5000;
        app.world_mut().get_mut::<TreeAge>(far).unwrap().0 = 5.0;
        app.world_mut().resource_mut::<Messages<DropLeaves>>().write(DropLeaves { tree: far, count: 100_000 });
        app.update();
        let young = {
            let world = app.world();
            let render = world.get::<TreeRender>(far).unwrap();
            let mesh = world.resource::<Assets<Mesh>>().get(&render.foliage.as_ref().unwrap().1).unwrap();
            on_tree(mesh, None)
        };
        let regrown: Vec<LeafBurst> = bursts(&mut app).into_iter().filter(|burst| burst.tree == far).collect();
        assert_eq!(regrown.len(), 2);
        assert_eq!(regrown.iter().map(|burst| burst.leaves).max(), Some(young as u32));

        // The bursts go once their leaves have fallen, lain and shrunk away.
        let gone_at = bursts(&mut app).iter().map(|burst| burst.gone_at).fold(0.0, f32::max);
        let now = app.world().resource::<Time>().elapsed_secs();
        assert!(gone_at - now > 6.0 * 0.75, "gone {:.1} s after the drop", gone_at - now);
        while app.world().resource::<Time>().elapsed_secs() < gone_at + 0.2 {
            app.update();
        }
        assert!(bursts(&mut app).is_empty());
    }
}
