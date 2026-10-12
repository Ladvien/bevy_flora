//! Felling: a cut tree falls, lands with a bounce and leaves a stump.
//!
//! Insert [`FellTree`] on a tree ([`ProceduralTree`], or a [`PooledTree`], which first becomes
//! the `ProceduralTree` of its variant). Once the tree is built, the plugin cuts its wood at
//! `cut_height`: the part below becomes a [`TreeStump`] entity, the part above falls with the
//! tree, and both get a cut face (an end-grain disc). The tree then tips over toward `direction`
//! as a rod hinged at the front of the cut would, comes to rest where its limbs first touch the
//! ground (twigs and leaves give way), and bounces a little; [`TreeLanded`] is sent the moment
//! it hits, and [`Felled`] says how far it has tipped and when it lies still. The ground is flat
//! at the height the tree stands at, unless a [`FloraGround`] resource says otherwise: then the
//! cut is measured from the uphill side of the trunk and the tree lands on the terrain, down a
//! slope or against it.
//!
//! Once it lands a felled tree stops swaying in the wind (its leaves still flutter). It is no
//! longer rebuilt when its [`ProceduralTree`] or [`TreeAge`] changes, and keeps its entity:
//! despawn it, or swap it for logs, once it has landed. Its [`TreeShape`](crate::TreeShape)
//! turns with it but is not cut.

use bevy::asset::{Assets, Handle};
use bevy::camera::visibility::Visibility;
use bevy::ecs::prelude::*;
use bevy::image::Image;
use bevy::math::{Quat, Vec2, Vec3, Vec3Swizzles};
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::reflect::Reflect;
use bevy::time::Time;
use bevy::transform::components::Transform;

use crate::gpu::GpuGrowth;
use crate::ground::FloraGround;
use crate::mesh::MeshData;
use crate::mesh::cut::cut;
use crate::pipeline::build_aged;
use crate::plugin::{Growing, Paint, ProceduralTree, TreeAge, TreeRender, paint_child};
use crate::pool::{PooledShapeFilled, PooledTree, PooledTreeSpawned, TreePool};
use crate::shading::{FloraMaterial, FloraWindBuffer, SharedFlora, TextureLibrary, TreeShake, TreeShakes};
use crate::params::TreeParams;
use crate::skeleton::{NodeKind, Skeleton, trunk_wobble};

/// Fell this tree: insert it on a [`ProceduralTree`] or a [`PooledTree`]. See the
/// [module docs](self).
#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct FellTree {
    /// Which way the tree falls, world x and z; normalised when used. Zero falls along +x.
    pub direction: Vec2,
    /// Where the saw goes in, in metres above the tree's base (at least 5 cm, at most half the
    /// trunk); with a [`FloraGround`], above the ground on the uphill side of the trunk.
    pub cut_height: f32,
    /// How fast it falls: 1 as a real tree of its height would (a 15 m tree takes about four
    /// seconds from the first lean to the ground), 2 twice as fast.
    pub speed: f32,
}

impl Default for FellTree {
    fn default() -> Self {
        Self { direction: Vec2::X, cut_height: 0.5, speed: 1.0 }
    }
}

/// A tree that has been cut, added by the plugin in place of [`FellTree`]: falling, then lying
/// on the ground.
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct Felled {
    /// The stump it left.
    pub stump: Entity,
    /// How far it has tipped from upright, in radians.
    pub angle: f32,
    /// True once it lies still.
    pub at_rest: bool,
    #[reflect(ignore)]
    fall: Fall,
}

/// The stump a felled tree left: the wood below the cut and its cut face, where the tree stood.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct TreeStump {
    /// The tree that was cut from it.
    pub tree: Entity,
}

/// Sent the frame a felled tree first hits the ground: the moment for the crash, the dust and
/// the camera shake.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct TreeLanded {
    /// The tree.
    pub tree: Entity,
    /// Its stump.
    pub stump: Entity,
    /// Where it first touched the ground, in world space.
    pub at: Vec3,
}

/// How a felled tree moves, in its own (local) space.
#[derive(Clone, Debug, Default)]
struct Fall {
    /// Its transform before it was cut.
    base: Transform,
    /// The front edge of the cut, which it turns about.
    hinge: Vec3,
    /// The axis it turns about (through `hinge`).
    axis: Vec3,
    /// The angle at which it first touches the ground, and the point that touches.
    rest: f32,
    contact: Vec3,
    /// `3 g / 2 L` times the square of the speed: a rod of length `L` hinged at its foot.
    pull: f32,
    /// Angular speed, radians a second.
    spin: f32,
    landed: bool,
    /// How far the crown reaches from the trunk, for the shake it gives its neighbours.
    crown_radius: f32,
    /// Its wood and leaves, and the materials they take once it lands (they do not sway).
    still: Vec<(Entity, Handle<FloraMaterial>)>,
}

/// How much of its speed a tree keeps when it bounces off the ground.
const RESTITUTION: f32 = 0.25;
/// A bounce that would lift the tree less than this, in radians, is over: it lies still.
const SETTLE_RISE: f32 = 0.004;
/// The push the last stroke of the saw gives a cut tree, radians a second: from upright it
/// leans slowly, then gravity takes it.
const START_SPIN: f32 = 0.08;
/// Integration step, seconds.
const STEP: f32 = 1.0 / 240.0;
const GRAVITY: f32 = 9.81;
/// Limbs at least this share of the trunk's radius at the cut, and at least [`CONTACT_RADIUS`]
/// (metres), stop a falling tree; thinner branches, twigs and the leaves give way and may end
/// below the ground, as a crown crushed into it. A conifer's whorls of thin limbs do not prop it
/// up at a slant.
const CONTACT_SHARE: f32 = 0.15;
const CONTACT_RADIUS: f32 = 0.04;
/// The furthest a felled tree turns, from upright: on flat ground its trunk meets the ground
/// before 105°; down a steep slope it can lie along it, well past 90°.
const REST_LIMIT: f32 = 150.0 * std::f32::consts::PI / 180.0;

/// Start felling every tree with a [`FellTree`] that can be cut: turn a pooled tree into its
/// `ProceduralTree`, move a tree off the GPU growth path, then cut, spawn the stump and set up
/// the fall.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn start_felling(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut flora: ResMut<Assets<FloraMaterial>>,
    mut shared: ResMut<SharedFlora>,
    mut textures: TextureLibrary,
    wind: Option<Res<FloraWindBuffer>>,
    pools: Option<Res<Assets<TreePool>>>,
    ground: Option<Res<FloraGround>>,
    mut cut_face: Local<Option<Handle<StandardMaterial>>>,
    trees: Query<(
        Entity,
        &FellTree,
        &Transform,
        Option<&ProceduralTree>,
        Option<&TreeAge>,
        Option<&TreeRender>,
        Option<&PooledTree>,
        Has<GpuGrowth>,
        Has<Growing>,
    )>,
) {
    for (entity, fell, transform, tree, age, render, pooled, gpu, growing) in &trees {
        if let Some(pooled) = pooled {
            let Some(pool) = pools.as_deref().and_then(|pools| pools.get(&pooled.pool)) else { continue };
            let params = pool.tree_params(pooled.variant);
            commands
                .entity(entity)
                .despawn_related::<Children>()
                .remove::<(PooledTree, PooledTreeSpawned, PooledShapeFilled)>()
                .insert((ProceduralTree(params), TreeAge(pool.age)));
            continue;
        }
        let Some(tree) = tree else { continue };
        if growing {
            commands.entity(entity).remove::<Growing>();
        }
        if gpu {
            commands.entity(entity).remove::<GpuGrowth>();
            continue;
        }
        // Wait for the tree's meshes, rebuilt on the CPU.
        let Some(render) = render else { continue };
        let Some(paint) = render.paint.cloned_cpu() else { continue };
        let age = age.map_or(f32::INFINITY, |age| age.0);
        let built = build_aged(&render.full, &tree.0, age);
        if built.trunk.is_empty() {
            commands.entity(entity).remove::<FellTree>();
            continue;
        }

        let scale = transform.scale.y.max(1.0e-4);
        // On a slope the saw goes in above the uphill side of the trunk.
        let uphill = ground.as_deref().map_or(0.0, |ground| {
            let top = ground.top(transform.translation.xz(), tree.0.base_radius() * scale);
            (top - transform.translation.y).max(0.0) / scale
        });
        let cut_y = (uphill + fell.cut_height / scale).clamp(0.05 / scale, tree.0.trunk.height * 0.5);
        let above = cut(&built.trunk, cut_y, true);
        let below = cut(&built.trunk, cut_y, false);
        let face = cut_face
            .get_or_insert_with(|| {
                let grain = textures.end_grain();
                materials.add(StandardMaterial {
                    base_color: if grain.is_some() { bevy::color::Color::WHITE } else { END_GRAIN_COLOR },
                    base_color_texture: grain,
                    perceptual_roughness: 0.85,
                    ..Default::default()
                })
            })
            .clone();

        // The stump: the wood below the cut and its face, where the tree stands.
        let stump = commands.spawn((*transform, Visibility::Inherited, TreeStump { tree: entity })).id();
        let mut wood = commands.spawn((Mesh3d(meshes.add(below.kept.into_bevy_mesh())), ChildOf(stump)));
        paint_child(&mut wood, &paint, false);
        if !below.faces.is_empty() {
            commands.spawn((Mesh3d(meshes.add(below.faces.into_bevy_mesh())), MeshMaterial3d(face.clone()), ChildOf(stump)));
        }

        // How it will fall: about the front edge of the cut, until its limbs reach the ground.
        let local_dir = fall_direction(transform, fell.direction);
        let (hinge, trunk_reach) = hinge_point(&above.outlines, cut_y, local_dir);
        let axis = Vec3::Y.cross(local_dir).normalize();
        let contact_set = contact_points(&above.kept, &built.skeleton, &tree.0, cut_y, trunk_reach);
        // How far a point of the tree, in its own space, stands above the ground.
        let clearance = |p: Vec3| match ground.as_deref() {
            None => p.y,
            Some(ground) => {
                let world = transform.transform_point(p);
                (world.y - ground.height(world.xz())) / scale
            }
        };
        let (rest, contact) = rest_angle(&contact_set, hinge, axis, clearance);
        let length = above.kept.positions.iter().map(|p| (*p - hinge).length()).fold(trunk_reach, f32::max) * scale;
        let crown_radius =
            built.foliage.positions.iter().map(|p| Vec2::new(p.x, p.z).length()).fold(0.0, f32::max) * scale;

        // The tree keeps the wood above the cut and gains its face.
        commands.entity(render.trunk).insert((Mesh3d(meshes.add(above.kept.into_bevy_mesh())), Visibility::Inherited));
        if !above.faces.is_empty() {
            commands.spawn((Mesh3d(meshes.add(above.faces.into_bevy_mesh())), MeshMaterial3d(face), ChildOf(entity)));
        }
        // Materials that do not sway, for when it lies on the ground: while it falls the wind
        // still moves it as it did (a change there would show).
        let mut still = Vec::new();
        if let (Paint::Flora { .. }, Some(wind)) = (&paint, wind.as_deref()) {
            let calm = TreeParams { wind: crate::WindResponse { sway: 0.0, ..tree.0.wind.clone() }, ..tree.0.clone() };
            still.push((render.trunk, shared.resolve(&mut flora, &textures.wood(&calm), wind)));
            if let Some((leaves_child, _)) = &render.foliage {
                still.push((*leaves_child, shared.resolve(&mut flora, &textures.leaves(&calm), wind)));
            }
        }
        let speed = fell.speed.max(0.01);
        commands.entity(entity).remove::<FellTree>().insert(Felled {
            stump,
            angle: 0.0,
            at_rest: false,
            fall: Fall {
                base: *transform,
                hinge,
                axis,
                rest,
                contact,
                pull: 1.5 * GRAVITY / length.max(0.5) * speed * speed,
                spin: START_SPIN * speed,
                landed: false,
                crown_radius,
                still,
            },
        });
    }
}

/// Tip every falling tree over, a few small steps a frame, and settle it where it lands.
pub(crate) fn drive_felling(
    mut commands: Commands,
    time: Res<Time>,
    mut shakes: ResMut<TreeShakes>,
    mut landed: MessageWriter<TreeLanded>,
    mut trees: Query<(Entity, &mut Felled, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut felled, mut transform) in &mut trees {
        if felled.at_rest {
            continue;
        }
        let felled = &mut *felled;
        let fall = &mut felled.fall;
        let mut left = dt;
        while left > 0.0 && !felled.at_rest {
            let h = left.min(STEP);
            left -= h;
            fall.spin += fall.pull * felled.angle.sin() * h;
            felled.angle += fall.spin * h;
            if felled.angle >= fall.rest && fall.spin > 0.0 {
                felled.angle = fall.rest;
                if !fall.landed {
                    fall.landed = true;
                    let resting = fall.base.mul_transform(pivot(fall, fall.rest));
                    let at = resting.transform_point(fall.contact);
                    landed.write(TreeLanded { tree: entity, stump: felled.stump, at });
                    // On the ground it stops swaying with the wind; the crash hides the change.
                    for (child, material) in fall.still.drain(..) {
                        commands.entity(child).insert(MeshMaterial3d(material));
                    }
                    // Its own leaves shake on the ground (it no longer sways), and the crowns it
                    // crashed through swing.
                    let own = TreeShake { at: resting.translation.xz(), radius: 0.5, strength: 2.0, duration: 2.0, ..Default::default() };
                    shakes.shake(own, &time);
                    let crash = TreeShake {
                        at: at.xz(),
                        radius: fall.crown_radius.max(2.0),
                        strength: 1.2,
                        duration: 2.5,
                        ..Default::default()
                    };
                    shakes.shake(crash, &time);
                }
                fall.spin *= -RESTITUTION;
                // How far the bounce would lift it back against gravity; a small or quick tree
                // falls back within a step, so its speed alone never gets small.
                let rise = fall.spin * fall.spin / (2.0 * fall.pull * fall.rest.sin().max(0.05));
                if rise < SETTLE_RISE {
                    fall.spin = 0.0;
                    felled.at_rest = true;
                }
            }
        }
        *transform = fall.base.mul_transform(pivot(fall, felled.angle));
    }
}

/// Turning by `angle` about the fall's hinge, in the tree's own space.
fn pivot(fall: &Fall, angle: f32) -> Transform {
    let turn = Quat::from_axis_angle(fall.axis, angle);
    Transform::from_translation(fall.hinge - turn * fall.hinge).with_rotation(turn)
}

/// `direction` (world x, z) in the tree's own horizontal plane, unit length.
fn fall_direction(transform: &Transform, direction: Vec2) -> Vec3 {
    let world = Vec3::new(direction.x, 0.0, direction.y).normalize_or(Vec3::X);
    let local = transform.rotation.inverse() * world;
    Vec3::new(local.x, 0.0, local.z).normalize_or(Vec3::X)
}

/// The front edge of the cut toward `direction`, which a falling tree turns about (the hinge
/// wood of a real felling cut), and how far it is from the trunk's centre.
fn hinge_point(outlines: &[Vec<Vec3>], cut_y: f32, direction: Vec3) -> (Vec3, f32) {
    let trunk = outlines.iter().max_by_key(|o| o.len()).map(Vec::as_slice).unwrap_or(&[]);
    if trunk.is_empty() {
        return (Vec3::new(0.0, cut_y, 0.0), 0.0);
    }
    let centre = trunk.iter().copied().sum::<Vec3>() / trunk.len() as f32;
    let reach = trunk.iter().map(|p| (*p - centre).dot(direction)).fold(0.0, f32::max);
    (Vec3::new(centre.x, cut_y, centre.z) + direction * reach, reach)
}

/// The points of the wood above the cut that can hold a falling tree up: the trunk and leader
/// all along (sampled from `skeleton`), and the vertices of limbs at least [`CONTACT_SHARE`] as
/// thick as the trunk at the cut and [`CONTACT_RADIUS`] (`uv1.x` is a wood vertex's radius,
/// `trunk_radius` the trunk's at the cut, both in the tree's own units).
fn contact_points(wood: &MeshData, skeleton: &Skeleton, params: &TreeParams, cut_y: f32, trunk_radius: f32) -> Vec<Vec3> {
    let least = (trunk_radius * CONTACT_SHARE).max(CONTACT_RADIUS);
    let mut points: Vec<Vec3> =
        wood.positions.iter().zip(&wood.uv1).filter(|(_, uv1)| uv1.x >= least).map(|(p, _)| *p).collect();
    for node in skeleton.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Trunk | NodeKind::Leader)) {
        let (side, up) = node.direction.any_orthonormal_pair();
        for i in 0..=48 {
            let t = i as f32 / 48.0;
            let mut centre = node.start + node.direction * (node.length * t);
            if node.kind == NodeKind::Trunk {
                // The trunk's mesh follows its wobble; the node's axis does not.
                let (x, z) = trunk_wobble(&params.trunk, params.seed, centre.y / params.trunk.height.max(1.0e-4));
                centre += Vec3::new(x, 0.0, z);
            }
            let radius = node.base_radius + (node.tip_radius - node.base_radius) * t;
            for k in 0..12 {
                let (sin, cos) = (k as f32 * std::f32::consts::TAU / 12.0).sin_cos();
                let p = centre + (side * cos + up * sin) * radius;
                if p.y >= cut_y {
                    points.push(p);
                }
            }
        }
    }
    if points.is_empty() { wood.positions.clone() } else { points }
}

/// The smallest angle, turning about `axis` through `hinge`, at which a point of `points` first
/// reaches the ground, and that point; [`REST_LIMIT`] if none does. `clearance(p)` is how far
/// `p` (in the tree's space) stands above the ground; points already below it before the fall
/// (a limb low on the uphill side of a slope) are left out, as they would plough through.
fn rest_angle(points: &[Vec3], hinge: Vec3, axis: Vec3, clearance: impl Fn(Vec3) -> f32) -> (f32, Vec3) {
    let points: Vec<Vec3> = points.iter().copied().filter(|p| clearance(*p) >= 0.0).collect();
    let lowest = |angle: f32, stride: usize| {
        let turn = Quat::from_axis_angle(axis, angle);
        points
            .iter()
            .step_by(stride)
            .map(|p| (*p, clearance(turn * (*p - hinge) + hinge)))
            .fold((hinge, f32::INFINITY), |best, (p, y)| if y < best.1 { (p, y) } else { best })
    };
    let stride = (points.len() / 6000).max(1);
    let step = 0.5f32.to_radians();
    let mut below = None;
    let mut angle = 0.0;
    while angle < REST_LIMIT {
        angle += step;
        if lowest(angle, stride).1 < 0.0 {
            below = Some(angle);
            break;
        }
    }
    let Some(mut high) = below else { return (REST_LIMIT, lowest(REST_LIMIT, 1).0) };
    let mut low = (high - step).max(0.0);
    for _ in 0..14 {
        let mid = 0.5 * (low + high);
        if lowest(mid, 1).1 < 0.0 {
            high = mid;
        } else {
            low = mid;
        }
    }
    (low, lowest(low, 1).0)
}

/// The colour of fresh-cut wood, for a cut face without the end-grain picture.
pub(crate) const END_GRAIN_COLOR: bevy::color::Color = bevy::color::Color::srgb(0.80, 0.66, 0.46);

/// The end-grain picture a cut face wears: pale sapwood, warmer heartwood, growth rings that
/// wander a little, a dark pith and a few checks, RGBA8 sRGB, `size` square.
fn end_grain_pixels(size: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    let half = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 + 0.5 - half) / half;
            let dz = (y as f32 + 0.5 - half) / half;
            let r = (dx * dx + dz * dz).sqrt();
            let theta = dz.atan2(dx);
            let wander = 0.012 * (3.0 * theta + 1.3).sin() + 0.008 * (7.0 * theta + 0.4).sin();
            let ring = ((r + wander) * 17.0).fract();
            let late = smoothstep(0.78, 0.95, ring) * (1.0 - smoothstep(0.95, 1.0, ring));
            let sapwood = Vec3::new(225.0, 199.0, 152.0);
            let heartwood = Vec3::new(201.0, 156.0, 104.0);
            let mut colour = heartwood.lerp(sapwood, smoothstep(0.45, 0.7, r));
            colour *= 1.0 - 0.18 * late;
            // The pith, and a few checks running out from it.
            colour *= 0.55 + 0.45 * smoothstep(0.02, 0.06, r);
            let check = [0.4f32, 2.3, 4.4]
                .iter()
                .map(|&a| {
                    let off = (theta - a).sin().abs() * r;
                    let facing = if (theta - a).cos() > 0.0 { 1.0 } else { 0.0 };
                    facing * (1.0 - smoothstep(0.0, 0.012, off)) * (1.0 - smoothstep(0.25, 0.55, r))
                })
                .fold(0.0, f32::max);
            colour *= 1.0 - 0.55 * check;
            // The cambium, darker, where the bark begins.
            colour *= 1.0 - 0.35 * smoothstep(0.93, 1.0, r);
            data.extend([colour.x, colour.y, colour.z].map(|c| c.clamp(0.0, 255.0) as u8));
            data.push(255);
        }
    }
    data
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The end-grain image with its mip chain, for [`TextureLibrary::end_grain`].
pub(crate) fn end_grain_image() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    const SIZE: u32 = 256;
    let base = end_grain_pixels(SIZE);
    // `Image::new` takes the base level alone (debug builds check its length); the chain
    // replaces the data afterwards.
    let mut image = Image::new(
        Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
        TextureDimension::D2,
        base.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(crate::impostor::with_mip_chain(&base, SIZE, SIZE));
    image.texture_descriptor.mip_level_count = crate::impostor::MIP_LEVELS;
    image
}

/// Messages and reflection; the systems are in the plugin's chain.
pub(crate) fn build(app: &mut bevy::app::App) {
    app.add_message::<TreeLanded>()
        .register_type::<FellTree>()
        .register_type::<Felled>()
        .register_type::<TreeStump>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TreePreset, build_tree};

    #[test]
    fn a_felled_tree_rests_on_its_limbs_without_sinking_into_the_ground() {
        for preset in [
            TreePreset::Oak,
            TreePreset::Pine,
            TreePreset::Willow,
            TreePreset::Birch,
            TreePreset::Maple,
            TreePreset::Spruce,
            TreePreset::Redwood,
        ] {
            let params = preset.params();
            let tree = build_tree(&params, f32::INFINITY);
            let above = cut(&tree.trunk, 0.5, true);
            let direction = Vec3::X;
            let (hinge, reach) = hinge_point(&above.outlines, 0.5, direction);
            let points = contact_points(&above.kept, &tree.skeleton, &params, 0.5, reach);
            let axis = Vec3::Y.cross(direction).normalize();
            let (rest, contact) = rest_angle(&points, hinge, axis, |p| p.y);
            assert!(
                rest > 45f32.to_radians() && rest < 105f32.to_radians(),
                "{preset:?}: rests at {}°",
                rest.to_degrees()
            );
            let turn = Quat::from_axis_angle(axis, rest);
            let lowest = points.iter().map(|p| (turn * (*p - hinge) + hinge).y).fold(f32::INFINITY, f32::min);
            assert!(lowest > -0.005, "{preset:?}: its limbs sink {lowest} m into the ground at rest");
            assert!(lowest < 0.05, "{preset:?}: floats {lowest} m above the ground at rest");
            assert!((turn * (contact - hinge) + hinge).y < 0.05, "{preset:?}: the contact point is not on the ground");
            // It falls toward +x: the top of the trunk ends on that side.
            let top = points.iter().copied().max_by(|a, b| a.y.total_cmp(&b.y)).unwrap();
            assert!((turn * (top - hinge) + hinge).x > hinge.x, "{preset:?}: fell the wrong way");
        }
    }

    fn app() -> bevy::app::App {
        use bevy::asset::{AssetApp, AssetPlugin};
        use bevy::time::TimeUpdateStrategy;
        let mut app = bevy::app::App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(crate::FloraPlugin);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 30.0)));
        app
    }

    /// Run updates until every felled tree lies still (at most `frames`), counting landings.
    fn settle(app: &mut bevy::app::App, frames: u32) -> usize {
        let mut cursor = app.world().resource::<bevy::ecs::message::Messages<TreeLanded>>().get_cursor_current();
        let mut landings = 0;
        for _ in 0..frames {
            app.update();
            landings += cursor.read(app.world().resource::<bevy::ecs::message::Messages<TreeLanded>>()).count();
            let mut query = app.world_mut().query::<&Felled>();
            let felled: Vec<bool> = query.iter(app.world()).map(|f| f.at_rest).collect();
            if !felled.is_empty() && felled.iter().all(|&rest| rest) {
                break;
            }
        }
        landings
    }

    #[test]
    fn a_felled_tree_falls_the_way_it_was_sent_lands_once_and_leaves_a_stump() {
        let mut app = app();
        let tree = app
            .world_mut()
            .spawn((ProceduralTree(TreePreset::Oak.params()), Transform::from_xyz(3.0, 0.0, -2.0)))
            .id();
        app.update();
        let fell = FellTree { direction: Vec2::new(0.0, 1.0), speed: 3.0, ..Default::default() };
        app.world_mut().entity_mut(tree).insert(fell);
        assert_eq!(settle(&mut app, 600), 1, "the tree did not land exactly once");

        let felled = app.world().get::<Felled>(tree).expect("the tree was not felled").clone();
        assert!(felled.at_rest, "the tree is still moving");
        let up = app.world().get::<Transform>(tree).unwrap().rotation * Vec3::Y;
        assert!(up.z > 0.6 && up.y < 0.6, "it fell along {up}, not toward +z");
        let stump = app.world().get::<TreeStump>(felled.stump).expect("no stump");
        assert_eq!(stump.tree, tree);
        assert_eq!(app.world().get::<Transform>(felled.stump).unwrap().translation, Vec3::new(3.0, 0.0, -2.0));
        let pieces = app.world().get::<Children>(felled.stump).map_or(0, |c| c.len());
        assert_eq!(pieces, 2, "the stump should be its wood and its cut face");

        // It is cut for good: ageing it rebuilds nothing.
        let mut generated =
            app.world().resource::<bevy::ecs::message::Messages<crate::TreeGenerated>>().get_cursor_current();
        app.world_mut().get_mut::<TreeAge>(tree).unwrap().0 = 3.0;
        app.update();
        app.update();
        let rebuilt = generated.read(app.world().resource::<bevy::ecs::message::Messages<crate::TreeGenerated>>());
        assert_eq!(rebuilt.filter(|g| g.entity == tree).count(), 0, "a felled tree was rebuilt");
    }

    #[test]
    fn a_pooled_tree_is_felled_as_the_tree_of_its_variant() {
        let mut app = app();
        let pool = crate::build_pool(
            &TreePreset::Birch.params(),
            3,
            f32::INFINITY,
            &crate::LodSettings::default(),
            &mut app.world_mut().resource_mut::<Assets<Mesh>>(),
        );
        let expected = pool.tree_params(4);
        let pool = app.world_mut().resource_mut::<Assets<TreePool>>().add(pool);
        let tree = app.world_mut().spawn((PooledTree { pool, variant: 4 }, Transform::default())).id();
        app.update();
        app.world_mut().entity_mut(tree).insert(FellTree { speed: 4.0, ..Default::default() });
        assert_eq!(settle(&mut app, 600), 1);
        assert!(app.world().get::<PooledTree>(tree).is_none(), "it is still pooled");
        assert_eq!(app.world().get::<ProceduralTree>(tree), Some(&ProceduralTree(expected)));
        assert!(app.world().get::<Felled>(tree).is_some_and(|f| f.at_rest));
    }

    #[test]
    fn on_a_slope_a_tree_is_cut_above_its_uphill_side_and_lands_on_the_terrain() {
        // The ground falls 0.4 m a metre toward +x, 22°.
        let slope = FloraGround::new(|at: Vec2| 3.0 - 0.4 * at.x);
        let params = TreePreset::Oak.params();
        let at = Vec2::new(2.0, 1.0);
        let fell = |ground: Option<&FloraGround>, direction: Vec2| {
            let mut app = app();
            let y = ground.map_or(0.0, |ground| ground.footing(at, params.base_radius()));
            if let Some(ground) = ground {
                app.insert_resource(ground.clone());
            }
            let base = Transform::from_xyz(at.x, y, at.y);
            let tree = app.world_mut().spawn((ProceduralTree(params.clone()), base)).id();
            app.update();
            app.world_mut().entity_mut(tree).insert(FellTree { direction, speed: 4.0, ..Default::default() });
            assert_eq!(settle(&mut app, 900), 1, "it did not land exactly once");
            let felled = app.world().get::<Felled>(tree).unwrap().clone();
            assert!(felled.at_rest, "it is still moving");
            (base, *app.world().get::<Transform>(tree).unwrap(), felled)
        };
        let (_, _, flat) = fell(None, Vec2::X);
        for (direction, downhill) in [(Vec2::X, true), (Vec2::NEG_X, false)] {
            let (base, lying, felled) = fell(Some(&slope), direction);
            // The saw went in half a metre above the ground on the trunk's uphill side.
            let cut = base.translation.y + felled.fall.hinge.y;
            let uphill = slope.top(at, params.base_radius());
            assert!((cut - uphill - 0.5).abs() < 1.0e-3, "cut at {cut}, the uphill ground at {uphill}");
            // Down the slope it turns further than on flat ground, up it less.
            let (angle, on_flat) = (felled.angle.to_degrees(), flat.angle.to_degrees());
            let turned = if downhill { angle > on_flat + 8.0 } else { angle < on_flat - 8.0 };
            assert!(turned, "downhill {downhill}: rests at {angle}°, {on_flat}° on flat ground");
            // And it lies on the terrain.
            let contact = lying.transform_point(felled.fall.contact);
            let gap = contact.y - slope.height(contact.xz());
            assert!(gap.abs() < 0.03, "downhill {downhill}: it touches the ground {gap} m off it");
        }
    }

    #[test]
    fn every_preset_felled_on_a_slope_lands_once_and_lies_on_it() {
        // 27°, falling toward +x.
        let slope = FloraGround::new(|at: Vec2| -0.5 * at.x);
        for preset in TreePreset::ALL {
            let params = preset.params();
            for direction in [Vec2::X, Vec2::NEG_X, Vec2::Y] {
                let mut app = app();
                app.insert_resource(slope.clone());
                let base = Transform::from_xyz(0.0, slope.footing(Vec2::ZERO, params.base_radius()), 0.0);
                let tree = app.world_mut().spawn((ProceduralTree(params.clone()), base)).id();
                app.update();
                app.world_mut().entity_mut(tree).insert(FellTree { direction, speed: 4.0, ..Default::default() });
                let landings = settle(&mut app, 900);
                let felled = app.world().get::<Felled>(tree).unwrap().clone();
                let lying = *app.world().get::<Transform>(tree).unwrap();
                let contact = lying.transform_point(felled.fall.contact);
                let gap = contact.y - slope.height(contact.xz());
                assert!(
                    felled.at_rest && landings == 1 && gap.abs() < 0.05,
                    "{preset:?} toward {direction}: landed {landings} times, at rest {}, at {}°, {gap} m off the ground",
                    felled.at_rest,
                    felled.angle.to_degrees()
                );
            }
        }
    }

    #[test]
    fn every_preset_falls_and_lies_still() {
        for preset in TreePreset::ALL {
            for seed in [42, 1, 2] {
                let mut app = app();
                let tree = app.world_mut().spawn(ProceduralTree(TreeParams { seed, ..preset.params() })).id();
                app.update();
                app.world_mut().entity_mut(tree).insert(FellTree { speed: 4.0, ..Default::default() });
                let landings = settle(&mut app, 900);
                let felled = app.world().get::<Felled>(tree).unwrap_or_else(|| panic!("{preset:?} was not felled"));
                assert!(
                    felled.at_rest && landings == 1,
                    "{preset:?} seed {seed}: landed {landings} times, at rest: {}, angle {}° of {}°, spin {}",
                    felled.at_rest,
                    felled.angle.to_degrees(),
                    felled.fall.rest.to_degrees(),
                    felled.fall.spin
                );
            }
        }
    }
}
