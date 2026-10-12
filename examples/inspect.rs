//! Photographs one tree under controlled conditions, to look at how readable it is: from a
//! distance, from every side, at several zooms and in a row of frames.
//!
//! ```text
//! cargo run --release --example inspect -- --out <dir> [--preset <name>] [--mode pool|tree]
//!     [--age <a>] [--seed <n>] [--frames <n>] [--distance <m>[,<m>]] [--constant-size [<share>]]
//!     [--fov <deg>] [--yaw <deg>] [--eye <m>] [--look <share>] [--sun <x,y,z>] [--wind <s>]
//!     [--size <w>x<h>] [--lod <a,b,c>] [--no-impostors] [--level <0|1|2|3>] [--empty] [--no-msaa]
//! ```
//!
//! The tree stands at the origin; the camera stands `--distance` metres away on the bearing
//! `--yaw` (0 is +z), `--eye` metres above the ground, and looks at `--look` (a share of the
//! tree's height, 0.5 by default) on the trunk's axis. With `--frames n` and two distances the
//! camera backs off from the first to the second in `n` steps, spaced geometrically, so the
//! tree passes through every level of detail and into the impostor. Each frame is written to
//! `<dir>/frame_<nnnn>.png` and the camera's distance and field of view to `<dir>/frames.tsv`.
//!
//! `--mode pool` (the default) draws the tree from a [`TreePool`](bevy_flora::TreePool), as a
//! forest would, so the level of detail follows the distance (`--lod` moves the switches,
//! `--no-impostors` leaves the impostor out). `--mode tree` draws a `ProceduralTree` at `--age`
//! (a growth age; mature by default), which has one level of detail.
//! `--level n` draws the pool's level `n` at every distance (3 is the impostor), so two levels
//! can be compared at one distance; `--empty` photographs the same scene with no tree, so the
//! tree's pixels are the ones that differ from it. `--no-msaa` turns the camera's multisampling
//! off (it is on, 4 samples, by default).
//!
//! `--constant-size` zooms as the camera backs off, so the tree keeps filling `<share>` of the
//! frame's height (0.6 by default): consecutive frames then differ only by what the distance
//! changes, and a switch of level of detail shows as a jump in the picture. Without it the field
//! of view stays `--fov` and the tree shrinks as it would in a game.
//!
//! The clock steps a fixed 1/60 s per frame, the wind is off unless `--wind` says otherwise, and
//! nothing depends on the wall clock, so two runs write the same frames.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use bevy::app::AppExit;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::time::TimeUpdateStrategy;
use bevy_flora::{
    FloraPlugin, FloraWind, LodDistances, LodSettings, PooledTree, ProceduralTree, TreeAge, TreePool, TreePreset,
    build_pool, build_tree,
};

/// Frames rendered before the first photograph so the pool's impostor is baked and shadows exist.
const WARMUP_FRAMES: u32 = 30;
/// Frames between moving the camera and photographing (visibility ranges, shadows).
const SETTLE_FRAMES: u32 = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Pool,
    Tree,
}

struct Args {
    out: PathBuf,
    preset: TreePreset,
    mode: Mode,
    age: f32,
    seed: i32,
    frames: u32,
    distance: [f32; 2],
    /// `Some(share)`: zoom so the tree fills this share of the frame's height.
    constant_size: Option<f32>,
    fov: f32,
    yaw: f32,
    eye: f32,
    look: f32,
    sun: Vec3,
    wind: f32,
    size: UVec2,
    lod: Option<[f32; 3]>,
    impostors: bool,
    /// Draw only this level of detail of the pool (3: the impostor).
    level: Option<u32>,
    /// No tree: the background the tree's pixels are measured against.
    empty: bool,
    msaa: bool,
}

fn usage() -> ! {
    eprintln!(
        "usage: inspect --out <dir> [--preset <name>] [--mode pool|tree] [--age <a>] [--seed <n>] \
         [--frames <n>] [--distance <m>[,<m>]] [--constant-size [<share>]] [--fov <deg>] [--yaw <deg>] \
         [--eye <m>] [--look <share>] [--sun <x,y,z>] [--wind <s>] [--size <w>x<h>] [--lod <a,b,c>] \
         [--no-impostors] [--level <0|1|2|3>] [--empty] [--no-msaa]"
    );
    std::process::exit(2);
}

fn floats(list: &str) -> Vec<f32> {
    list.split(',').map(|p| p.parse().unwrap_or_else(|_| usage())).collect()
}

fn parse_args() -> Args {
    let mut args = Args {
        out: PathBuf::new(),
        preset: TreePreset::Oak,
        mode: Mode::Pool,
        age: f32::INFINITY,
        seed: 1,
        frames: 1,
        distance: [30.0, 30.0],
        constant_size: None,
        fov: 45.0,
        yaw: 45.0,
        eye: 1.7,
        look: 0.5,
        sun: Vec3::new(-20.0, 25.0, 20.0),
        wind: 0.0,
        size: UVec2::new(960, 540),
        lod: None,
        impostors: true,
        level: None,
        empty: false,
        msaa: true,
    };
    let mut have_out = false;
    let mut it = std::env::args().skip(1).peekable();
    fn value<T: std::str::FromStr>(it: &mut impl Iterator<Item = String>) -> T {
        it.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage())
    }
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out" => {
                args.out = it.next().unwrap_or_else(|| usage()).into();
                have_out = true;
            }
            "--preset" => {
                let name = it.next().unwrap_or_else(|| usage());
                args.preset = TreePreset::ALL.into_iter().find(|p| p.name() == name).unwrap_or_else(|| usage());
            }
            "--mode" => {
                args.mode = match it.next().as_deref() {
                    Some("pool") => Mode::Pool,
                    Some("tree") => Mode::Tree,
                    _ => usage(),
                }
            }
            "--age" => args.age = value(&mut it),
            "--seed" => args.seed = value(&mut it),
            "--frames" => args.frames = value(&mut it),
            "--distance" => {
                let d = floats(&it.next().unwrap_or_else(|| usage()));
                args.distance = match d[..] {
                    [a] => [a, a],
                    [a, b] => [a, b],
                    _ => usage(),
                };
            }
            "--constant-size" => {
                let share = it.peek().and_then(|v| v.parse::<f32>().ok());
                if share.is_some() {
                    it.next();
                }
                args.constant_size = Some(share.unwrap_or(0.6));
            }
            "--fov" => args.fov = value(&mut it),
            "--yaw" => args.yaw = value(&mut it),
            "--eye" => args.eye = value(&mut it),
            "--look" => args.look = value(&mut it),
            "--sun" => {
                let p = floats(&it.next().unwrap_or_else(|| usage()));
                let [x, y, z]: [f32; 3] = p.try_into().unwrap_or_else(|_| usage());
                args.sun = Vec3::new(x, y, z);
            }
            "--wind" => args.wind = value(&mut it),
            "--size" => {
                let s = it.next().unwrap_or_else(|| usage());
                let (w, h) = s.split_once('x').unwrap_or_else(|| usage());
                args.size = UVec2::new(
                    w.parse().unwrap_or_else(|_| usage()),
                    h.parse().unwrap_or_else(|_| usage()),
                );
            }
            "--lod" => {
                let p = floats(&it.next().unwrap_or_else(|| usage()));
                args.lod = Some(p.try_into().unwrap_or_else(|_| usage()));
            }
            "--no-impostors" => args.impostors = false,
            "--level" => args.level = Some(value(&mut it)),
            "--empty" => args.empty = true,
            "--no-msaa" => args.msaa = false,
            _ => usage(),
        }
    }
    if !have_out || args.frames == 0 || args.distance.iter().any(|d| *d <= 0.0) {
        usage();
    }
    args
}

#[derive(Resource)]
struct Shoot {
    args: Args,
    target: Handle<Image>,
    /// Height of the tree, for the camera's aim and zoom.
    height: f32,
    frame: u32,
    wait: u32,
    in_flight: bool,
    /// Whether the scene (the pool's impostor, the tree's children) is ready.
    ready: bool,
    log: String,
}

#[derive(Component)]
struct Photographer;

fn main() {
    let args = parse_args();
    fs::create_dir_all(&args.out).expect("create the output directory");
    let wind = args.wind;
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: None,
            exit_condition: bevy::window::ExitCondition::DontExit,
            ..default()
        }),
        FloraPlugin,
    ))
    .insert_resource(ClearColor(Color::srgb(0.55, 0.72, 0.92)))
    .insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.6, 0.6, 0.6),
        brightness: 300.0,
        ..default()
    })
    .insert_resource(FloraWind { strength: wind, ..default() })
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)))
    .insert_resource(Shoot {
        args,
        target: Handle::default(),
        height: 1.0,
        frame: 0,
        wait: WARMUP_FRAMES,
        in_flight: false,
        ready: false,
        log: String::from("frame\tdistance\tfov\n"),
    })
    .add_systems(Startup, setup)
    .add_systems(Update, drive);
    app.run();
}

fn setup(
    mut commands: Commands,
    mut shoot: ResMut<Shoot>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut pools: ResMut<Assets<TreePool>>,
) {
    let size = shoot.args.size;
    shoot.target = images.add(Image::new_target_texture(
        size.x,
        size.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 10_000.0,
            ..default()
        },
        Transform::from_translation(shoot.args.sun).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(4000.0, 4000.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.45, 0.30),
            perceptual_roughness: 1.0,
            ..default()
        })),
    ));
    let msaa = if shoot.args.msaa { Msaa::Sample4 } else { Msaa::Off };
    commands.spawn((
        Camera3d::default(),
        msaa,
        RenderTarget::Image(shoot.target.clone().into()),
        Transform::default(),
        Photographer,
    ));

    let mut params = shoot.args.preset.params();
    params.seed = shoot.args.seed;
    let mature = build_tree(&params, shoot.args.age);
    shoot.height = mature
        .trunk
        .positions
        .iter()
        .chain(&mature.foliage.positions)
        .fold(0.0f32, |h, p| h.max(p.y))
        .max(0.5);
    if shoot.args.empty {
        return;
    }
    match shoot.args.mode {
        Mode::Pool => {
            let mut lod = LodSettings::default();
            if let Some([lod1, lod2, impostor]) = shoot.args.lod {
                lod.distances = LodDistances { lod1, lod2, impostor };
            }
            // One level everywhere: every other level's band lies below the camera's nearest
            // position or beyond its farthest.
            if let Some(level) = shoot.args.level {
                let (near, far) = (0.001, 1.0e6);
                lod.distances = match level {
                    0 => LodDistances { lod1: far, lod2: far + 1.0, impostor: far + 2.0 },
                    1 => LodDistances { lod1: near, lod2: far, impostor: far + 1.0 },
                    2 => LodDistances { lod1: near, lod2: near * 2.0, impostor: far },
                    _ => LodDistances { lod1: near, lod2: near * 2.0, impostor: near * 3.0 },
                };
            }
            if !shoot.args.impostors {
                lod.distances.impostor = f32::INFINITY;
            }
            let pool = build_pool(&params, 1, shoot.args.age, &lod, &mut meshes);
            let pool = pools.add(pool);
            commands.spawn((PooledTree { pool, variant: 0 }, Transform::default()));
        }
        Mode::Tree => {
            commands.spawn((ProceduralTree(params), TreeAge(shoot.args.age), Transform::default()));
        }
    }
}

/// The camera's distance and field of view (degrees) at frame `frame`.
fn pose(args: &Args, height: f32, frame: u32) -> (f32, f32) {
    let t = if args.frames > 1 { frame as f32 / (args.frames - 1) as f32 } else { 0.0 };
    let distance = args.distance[0] * (args.distance[1] / args.distance[0]).powf(t);
    let fov = match args.constant_size {
        Some(share) => (2.0 * (height / (2.0 * share * distance)).atan()).to_degrees().clamp(0.5, 120.0),
        None => args.fov,
    };
    (distance, fov)
}

/// A tree that has no render children yet.
type Unfinished = (Or<(With<PooledTree>, With<ProceduralTree>)>, Without<Children>);

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    mut shoot: ResMut<Shoot>,
    mut camera: Single<(&mut Transform, &mut Projection), With<Photographer>>,
    screenshots: Query<(), With<Screenshot>>,
    pools: Res<Assets<TreePool>>,
    trees: Query<(), Unfinished>,
    mut exit: MessageWriter<AppExit>,
) {
    if !shoot.ready {
        let baked = pools.iter().all(|(_, pool)| {
            !shoot.args.impostors || pool.variants.iter().all(|v| v.impostor.is_some())
        });
        if shoot.wait > 0 {
            shoot.wait -= 1;
            return;
        }
        if !baked || !trees.is_empty() {
            return;
        }
        shoot.ready = true;
        shoot.wait = SETTLE_FRAMES;
    }
    if shoot.in_flight {
        if !screenshots.is_empty() {
            return;
        }
        shoot.in_flight = false;
        shoot.frame += 1;
        shoot.wait = SETTLE_FRAMES;
        if shoot.frame == shoot.args.frames {
            let path = shoot.args.out.join("frames.tsv");
            fs::write(&path, &shoot.log).expect("write frames.tsv");
            println!("inspect: wrote {} frames to {}", shoot.args.frames, shoot.args.out.display());
            exit.write(AppExit::Success);
            return;
        }
    }

    let (distance, fov) = pose(&shoot.args, shoot.height, shoot.frame);
    let yaw = shoot.args.yaw.to_radians();
    let eye = Vec3::new(yaw.sin() * distance, shoot.args.eye, yaw.cos() * distance);
    let aim = Vec3::new(0.0, shoot.height * shoot.args.look, 0.0);
    *camera.0 = Transform::from_translation(eye).looking_at(aim, Vec3::Y);
    *camera.1 = Projection::Perspective(PerspectiveProjection {
        fov: fov.to_radians(),
        near: 0.1,
        far: 4000.0,
        ..default()
    });

    if shoot.wait > 0 {
        shoot.wait -= 1;
        return;
    }
    let path = shoot.args.out.join(format!("frame_{:04}.png", shoot.frame));
    let frame = shoot.frame;
    let _ = writeln!(shoot.log, "{frame}\t{distance:.3}\t{fov:.3}");
    commands.spawn(Screenshot::image(shoot.target.clone())).observe(save_to_disk(path));
    shoot.in_flight = true;
}
