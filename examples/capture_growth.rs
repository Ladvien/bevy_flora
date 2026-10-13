//! Records the README clips, rendered offscreen to numbered PNG frames: one tree growing from a
//! seedling to maturity while the camera orbits it (`docs/growth.gif`), or, with `--grove`, a
//! grove of mixed species growing in turn while a gust front crosses it (`docs/grove.gif`).
//!
//! ```text
//! cargo run --release --example capture_growth -- [--out <dir>] [--preset <name>] [--gpu]
//! ffmpeg -y -framerate 24 -i <dir>/frame_%04d.png -vf \
//!   "fps=24,scale=480:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" \
//!   docs/growth.gif
//!
//! cargo run --release --example capture_growth -- --grove [--out <dir>] [--gpu]
//! ffmpeg -y -framerate 24 -i <dir>/frame_%04d.png -vf \
//!   "fps=20,scale=560:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" \
//!   docs/grove.gif
//! ```
//!
//! `--seasons` records the grove fully grown through a year (`docs/seasons.gif`): the deciduous
//! trees turn their autumn colours and shed their leaves, snow settles on every tree and melts,
//! the cherry blossoms on bare branches, and the leaves come out again, while the pine and the
//! cypress stay green. The blossom is too few pixels to win a pink in the grove's palette (it
//! comes out white), so the GIF takes 80 colours from the whole clip and 16 from the pink pixels
//! of the cherry's crown while it blossoms:
//!
//! ```text
//! cargo run --release --example capture_growth -- --seasons [--out <dir>]
//! ffmpeg -y -framerate 24 -i <dir>/frame_%04d.png -vf \
//!   "fps=20,scale=560:-1:flags=lanczos,palettegen=max_colors=80:stats_mode=diff" -update 1 clip.png
//! ffmpeg -y -framerate 24 -start_number 136 -i <dir>/frame_%04d.png -vf \
//!   "trim=end_frame=28,fps=20,scale=560:-1:flags=lanczos,crop=200:150:320:150,format=gbrp,geq=r='if(gt(r(X\,Y)-g(X\,Y)\,18)*gt(r(X\,Y)\,150)\,r(X\,Y)\,0)':g='if(gt(r(X\,Y)-g(X\,Y)\,18)*gt(r(X\,Y)\,150)\,g(X\,Y)\,0)':b='if(gt(r(X\,Y)-g(X\,Y)\,18)*gt(r(X\,Y)\,150)\,b(X\,Y)\,0)',format=rgb24,palettegen=max_colors=16:stats_mode=full" \
//!   -update 1 pink.png
//! ffmpeg -y -i clip.png -i pink.png -filter_complex \
//!   "[0]crop=16:5:0:0[a];[1]crop=16:1:0:0[b];[a][b]vstack,pad=16:16:0:0:black" -update 1 palette.png
//! ffmpeg -y -framerate 24 -i <dir>/frame_%04d.png -i palette.png -filter_complex \
//!   "[0]fps=20,scale=560:-1:flags=lanczos[x];[x][1]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" \
//!   docs/seasons.gif
//! ```
//!
//! `--shake` records it fully grown from a still camera, knocked with `TreeShakes`
//! (`docs/shake.gif`, encoded like the grove): an axe on the oak, one on the birch, then a blast
//! that swings every tree. `--fell` records a few grown trees from a still camera while the oak in
//! front is felled (`FellTree`), crashes to the ground and leaves its stump (`docs/fell.gif`,
//! encoded like the grove).
//!
//! Each clip is rendered as 192 frames, eight seconds at 24 fps; the grove's GIF keeps 160 of
//! them (20 fps) because its camera orbits and every leaf moves, which makes a large GIF. The
//! age of every tree and the clock are set explicitly per frame: time stands still except for
//! one 1/24 s step per captured frame, so the wind sways smoothly, a gust crosses the grove at a
//! steady pace, and two runs write the same frames. `--out` defaults to `target/capture/growth`
//! (`target/capture/grove`), `--preset` to `maple`; `--gpu` grows the trees in the vertex shader
//! instead of rebuilding their meshes (compare the two runs' frames with the `compare_gallery`
//! example).

use std::f32::consts::{PI, TAU};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use bevy::app::AppExit;
use bevy::camera::RenderTarget;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::time::TimeUpdateStrategy;
use bevy_flora::{
    FellTree, FloraPlugin, FloraSeason, FloraWind, GpuGrowth, GustFront, ProceduralTree, RenderStyle, TreeAge,
    TreeParams, TreePreset, TreeShake, TreeShakes, build_tree,
};

/// Frames per second of the clips, and so the clock's step per captured frame.
const FPS: f32 = 24.0;
/// Frames in a clip: eight seconds.
const TOTAL_FRAMES: u32 = 192;
/// The shake clip's knocks: (frame, the grove tree hit, or `None` for a blast at the grove's
/// middle, radius, strength, duration): an axe on the oak (stiffer than the birch, sway 0.8
/// against 1.4, so a harder knock), one on the birch, then a blast that swings every tree.
const SHAKES: [(u32, Option<usize>, f32, f32, f32); 3] =
    [(12, Some(0), 1.0, 3.0, 3.0), (64, Some(2), 1.0, 2.5, 2.5), (112, None, 30.0, 3.0, 3.5)];
/// The fell clip's trees: species, seed and place (in the grove's spacing), the oak in front
/// with open ground on both sides of it, the others behind.
const FELL_GROVE: [(TreePreset, i32, [f32; 2]); 5] = [
    (TreePreset::Oak, 42, [0.0, 0.0]),
    (TreePreset::Maple, 5, [-1.6, -0.6]),
    (TreePreset::Pine, 1, [-1.9, 0.8]),
    (TreePreset::Birch, 8, [0.3, -1.4]),
    (TreePreset::Cypress, 2, [-2.3, 1.9]),
];
/// The fell clip's cut: (frame, the tree in `FELL_GROVE`, bearing it falls toward in degrees
/// from +x toward +z). The camera looks from +x, so the oak falls to the left across open
/// ground, square to the view.
const FELLS: [(u32, usize, f32); 1] = [(24, 0, 90.0)];
/// Frames rendered before the first capture so meshes and shadows exist.
const WARMUP_FRAMES: u32 = 10;
/// Frames between setting an age and capturing it (mesh upload, shadows).
const SETTLE_FRAMES: u32 = 2;
/// Camera orbit over the whole clip, in turns.
const ORBIT_TURNS: f32 = 0.35;
/// The share of its way to maturity a tree has when it first appears, so no clip starts on
/// bare ground: a small one for the single tree, a bigger one for the grove, where most of
/// the trees are not yet there at the start.
const TREE_SEEDLING: f32 = 0.03;
const GROVE_SEEDLING: f32 = 0.10;
/// Camera field of view, Bevy's default.
const FOV: f32 = PI / 4.0;

/// One tree of a clip: how its age follows the frame number.
#[derive(Component)]
struct Growth {
    /// First frame the tree exists; before it the tree has age 0 and nothing is drawn.
    start: u32,
    /// Frames from the seedling to maturity.
    frames: u32,
    /// The share of the way to maturity at `start`.
    seedling: f32,
    mature_at: f32,
}

impl Growth {
    /// The age at `frame`: eased out, so the early, fast-changing years get more frames.
    fn age(&self, frame: u32) -> f32 {
        if frame < self.start {
            return 0.0;
        }
        let raw = ((frame - self.start) as f32 / (self.frames - 1) as f32).min(1.0);
        let progress = self.seedling + (1.0 - self.seedling) * raw;
        self.mature_at * (1.0 - (1.0 - progress).powi(2))
    }
}

/// What a clip shows.
#[derive(Clone)]
enum Scene {
    /// One tree growing for three quarters of the clip, then held.
    Tree(Box<TreeParams>),
    /// A grove of mixed species growing in turn, and a gust front crossing it.
    Grove,
    /// The grove, fully grown, through autumn, leaf fall, winter and spring (`FloraSeason`).
    Seasons,
    /// The grove, fully grown, in a light wind, knocked: one tree, another, then all of it.
    Shake,
    /// A few grown trees in a light wind; the oak in front is felled and crashes to the ground.
    Fell,
}

/// A tree of the grove: species, seed, where it stands (in units of the grove's spacing, x and
/// z), the frame it starts growing and how many frames it takes. The camera starts on the +z
/// side and swings round to the +x side, so the tall and broad trees stand toward -x and -z,
/// behind the oak whichever way it looks, and the small ones in front.
const GROVE: [(TreePreset, i32, [f32; 2], u32, u32); 8] = [
    (TreePreset::Oak, 42, [0.0, 0.0], 0, 130),
    (TreePreset::Maple, 5, [-1.7, -1.0], 0, 124),
    (TreePreset::Birch, 2, [1.4, 0.3], 0, 118),
    (TreePreset::Pine, 1, [-0.2, -1.9], 8, 128),
    (TreePreset::Cypress, 2, [-2.3, 0.1], 12, 126),
    (TreePreset::Willow, 4, [-1.2, 1.4], 16, 130),
    (TreePreset::JapaneseMaple, 6, [0.7, 1.4], 20, 120),
    (TreePreset::Cherry, 3, [1.1, -1.0], 24, 120),
];
/// Frame the gust front is launched, and its shape.
const GUST_FRAME: u32 = 110;
const GUST_SPEED: f32 = 12.0;
const GUST_WIDTH: f32 = 7.0;
const GUST_STRENGTH: f32 = 1.4;
/// The part of a year the seasons clip shows, as (share of the clip, `FloraSeason::of_year`'s
/// year) keys joined by straight lines: late summer to the last leaves falling in the first half,
/// the snowy weeks (0.88 to 0.98 of the year, a tenth of it) in the next fifth, so the snow is on
/// screen long enough to see, the melt and the cherry's blossom (0.98 to 0.07, the flowers out on
/// bare branches from the year's turn) in the fifth after that, and the leaves of summer in the rest.
const SEASON_KEYS: [(f32, f32); 5] = [(0.0, 0.47), (0.5, 0.88), (0.72, 0.98), (0.92, 1.07), (1.0, 1.13)];

/// The year (for `FloraSeason::of_year`) at share `t` of the seasons clip.
fn season_year(t: f32) -> f32 {
    SEASON_KEYS
        .windows(2)
        .find(|pair| t <= pair[1].0)
        .map_or(SEASON_KEYS[SEASON_KEYS.len() - 1].1, |pair| {
            let ((t0, y0), (t1, y1)) = (pair[0], pair[1]);
            y0 + (y1 - y0) * ((t - t0) / (t1 - t0)).clamp(0.0, 1.0)
        })
}

/// The ground's colour under `snow` (0..1): grass, or snow over it.
fn ground_colour(snow: f32) -> Color {
    let (grass, snowed) = (Vec3::new(0.35, 0.45, 0.30), Vec3::new(0.86, 0.88, 0.92));
    let c = grass.lerp(snowed, snow);
    Color::srgb(c.x, c.y, c.z)
}

#[derive(Resource)]
struct Capture {
    scene: Scene,
    size: UVec2,
    out: PathBuf,
    target: Handle<Image>,
    /// The ground's material: it whitens with the snow in the seasons clip.
    ground: Handle<StandardMaterial>,
    /// Orbit radius and look-at point, framed on the mature trees.
    distance: f32,
    focus: Vec3,
    /// The wind blows toward this (world x and z): across the view, so the sway shows.
    wind: Vec2,
    frame: u32,
    wait: u32,
    in_flight: bool,
    gusted: bool,
    /// Where the trees stand, in spawn order (the grove's order).
    trees: Vec<Vec3>,
    /// The grove's trees in its order; `None` for one the clip leaves out.
    entities: Vec<Option<Entity>>,
    /// How many of `SHAKES` have been sent.
    shaken: usize,
    /// How many of `FELLS` have been cut.
    felled: usize,
}

#[derive(Component)]
struct OrbitCam;

fn usage() -> ! {
    eprintln!(
        "usage: capture_growth [--out <dir>] [--preset <name> [--style <natural|painterly|toon>] | --grove | --seasons | --shake | --fell] [--gpu]"
    );
    std::process::exit(2);
}

fn main() {
    let mut out = None;
    let mut preset = None;
    // The grove clips: "grove", "seasons", "shake" or "fell"; none is the single growing tree.
    let mut clip: Option<&'static str> = None;
    let mut gpu = false;
    let mut style = RenderStyle::Natural;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--gpu" => gpu = true,
            "--grove" | "--seasons" | "--shake" | "--fell" => {
                if clip.is_some() {
                    usage();
                }
                clip = Some(match arg.as_str() {
                    "--grove" => "grove",
                    "--seasons" => "seasons",
                    "--shake" => "shake",
                    _ => "fell",
                });
            }
            "--preset" => {
                let name = args.next().unwrap_or_else(|| usage());
                preset = Some(
                    TreePreset::ALL
                        .into_iter()
                        .find(|p| p.name() == name)
                        .unwrap_or_else(|| usage()),
                );
            }
            "--style" => {
                style = match args.next().as_deref() {
                    Some("natural") => RenderStyle::Natural,
                    Some("painterly") => RenderStyle::Painterly,
                    Some("toon") => RenderStyle::Toon,
                    _ => usage(),
                }
            }
            _ => usage(),
        }
    }
    if clip.is_some() && (preset.is_some() || style != RenderStyle::Natural) {
        usage();
    }
    let out = out.unwrap_or_else(|| PathBuf::from(format!("target/capture/{}", clip.unwrap_or("growth"))));
    fs::create_dir_all(&out).expect("create output directory");

    let scene = match clip {
        Some("grove") => Scene::Grove,
        Some("seasons") => Scene::Seasons,
        Some("shake") => Scene::Shake,
        Some(_) => Scene::Fell,
        None => {
            let mut params = preset.unwrap_or(TreePreset::Maple).params();
            params.style = style;
            Scene::Tree(Box::new(params))
        }
    };
    let size = if matches!(scene, Scene::Tree(_)) { UVec2::new(800, 600) } else { UVec2::new(1280, 720) };
    let (distance, focus) = frame_scene(&scene, size);
    // Across the view: the camera looks along (-1, -1), the wind blows toward (-1, 1).
    let wind = Vec2::new(-1.0, 1.0).normalize();

    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "bevy_flora capture".to_string(),
                    ..default()
                }),
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
        // A light wind in the shake and fell clips, so the knocks and the falls are what moves.
        .insert_resource(FloraWind {
            strength: if matches!(scene, Scene::Shake | Scene::Fell) { 0.15 } else { 0.5 },
            direction: wind,
            speed: 1.4,
            ..default()
        })
        // Time stands still until `drive` steps it, once per captured frame.
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
        .insert_resource(Capture {
            scene,
            size,
            out,
            target: Handle::default(),
            ground: Handle::default(),
            distance,
            focus,
            wind,
            frame: 0,
            wait: WARMUP_FRAMES,
            in_flight: false,
            gusted: false,
            trees: Vec::new(),
            entities: Vec::new(),
            shaken: 0,
            felled: 0,
        })
        .insert_resource(UseGpu(gpu))
        .add_systems(Startup, setup)
        .add_systems(Update, drive)
        .run();
}

#[derive(Resource)]
struct UseGpu(bool);

/// The mature tree of `params` standing at `at`: the box around its wood and leaves.
fn bounds(params: &TreeParams, at: Vec3) -> (Vec3, Vec3) {
    let tree = build_tree(params, f32::INFINITY);
    tree.trunk
        .positions
        .iter()
        .chain(&tree.foliage.positions)
        .chain(&tree.flowers.positions)
        .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), &p| (lo.min(p + at), hi.max(p + at)))
}

/// The distance the camera orbits at and the point it looks at, so every mature tree fits.
fn frame_scene(scene: &Scene, size: UVec2) -> (f32, Vec3) {
    match scene {
        Scene::Tree(params) => {
            let (min, max) = bounds(params, Vec3::ZERO);
            let radius = (0.5 * (max - min).length()).max(0.5);
            (radius / (FOV * 0.5).sin() * 0.8, Vec3::new(0.0, 0.5 * (min.y + max.y), 0.0))
        }
        Scene::Grove | Scene::Seasons | Scene::Shake => {
            let placed = grove(Vec3::ZERO);
            let (min, max) = placed
                .iter()
                .map(|(params, at, ..)| bounds(params, *at))
                .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), (a, b)| (lo.min(a), hi.max(b)));
            // The orbit brings any tree close, so the grove is as wide as its diagonal and the
            // camera backs off by part of that.
            let half_width = 0.5 * Vec2::new(max.x - min.x, max.z - min.z).length();
            let focus = Vec3::new(0.5 * (min.x + max.x), 0.45 * max.y, 0.5 * (min.z + max.z));
            let half_height = (max.y - focus.y).max(focus.y);
            let aspect = size.x as f32 / size.y as f32;
            let half_h_fov = ((FOV * 0.5).tan() * aspect).atan();
            let fit = (half_width / half_h_fov.tan()).max(half_height / (FOV * 0.5).tan());
            (fit * 0.9 + half_width * 0.35, focus)
        }
        // Seen from +x: the standing trees and the ground the felled ones will lie on.
        Scene::Fell => {
            let placed = fell_grove();
            let (mut min, mut max) = placed
                .iter()
                .map(|(params, at, ..)| bounds(params, *at))
                .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), (a, b)| (lo.min(a), hi.max(b)));
            for &(_, tree, bearing) in &FELLS {
                let (params, at, ..) = &placed[tree];
                let (sin, cos) = bearing.to_radians().sin_cos();
                let (lo, hi) = bounds(params, Vec3::ZERO);
                let crown = 0.5 * (hi.x - lo.x).max(hi.z - lo.z);
                let lying = *at + Vec3::new(cos, 0.0, sin) * (hi.y + 0.5 * crown);
                (min, max) = (min.min(lying), max.max(lying));
            }
            let half_width = 0.55 * (max.z - min.z);
            let focus = Vec3::new(0.5 * (min.x + max.x), 0.42 * max.y, 0.5 * (min.z + max.z));
            let half_height = (max.y - focus.y).max(focus.y);
            let aspect = size.x as f32 / size.y as f32;
            let half_h_fov = ((FOV * 0.5).tan() * aspect).atan();
            let fit = (half_width / half_h_fov.tan()).max(half_height / (FOV * 0.5).tan());
            (fit * 0.9 + 0.25 * (max.x - min.x), focus)
        }
    }
}

/// The grove's trees (parameters, position, start frame, growth frames), centred on `centre`,
/// spaced by the mean crown width of the species.
fn grove(centre: Vec3) -> Vec<(TreeParams, Vec3, u32, u32)> {
    let spacing = grove_spacing();
    GROVE
        .iter()
        .map(|&(preset, seed, [x, z], start, frames)| {
            (TreeParams { seed, ..preset.params() }, centre + Vec3::new(x, 0.0, z) * spacing, start, frames)
        })
        .collect()
}

/// Half the mean crown width of the grove's species: the grid its positions are given in.
fn grove_spacing() -> f32 {
    let width = |p: &TreeParams| {
        let (min, max) = bounds(p, Vec3::ZERO);
        Vec2::new(max.x - min.x, max.z - min.z).length()
    };
    let widths: Vec<f32> =
        GROVE.iter().map(|&(preset, seed, ..)| width(&TreeParams { seed, ..preset.params() })).collect();
    0.5 * widths.iter().sum::<f32>() / widths.len() as f32
}

/// The fell clip's trees, mature, on the grove's spacing.
fn fell_grove() -> Vec<(TreeParams, Vec3, u32, u32)> {
    let spacing = grove_spacing();
    FELL_GROVE
        .iter()
        .map(|&(preset, seed, [x, z])| (TreeParams { seed, ..preset.params() }, Vec3::new(x, 0.0, z) * spacing, 0, 1))
        .collect()
}

fn setup(
    mut commands: Commands,
    mut capture: ResMut<Capture>,
    gpu: Res<UseGpu>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    capture.target = images.add(Image::new_target_texture(
        capture.size.x,
        capture.size.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 10_000.0,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            first_cascade_far_bound: capture.distance * 0.5,
            maximum_distance: capture.distance * 2.5,
            ..default()
        }
        .build(),
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    capture.ground = materials.add(StandardMaterial {
        base_color: ground_colour(0.0),
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(400.0, 400.0))),
        MeshMaterial3d(capture.ground.clone()),
    ));
    let (seedling, trees): (f32, Vec<(TreeParams, Vec3, u32, u32)>) = match &capture.scene {
        Scene::Tree(params) => (
            TREE_SEEDLING,
            vec![(TreeParams::clone(params), Vec3::ZERO, 0, TOTAL_FRAMES * 3 / 4)],
        ),
        Scene::Grove => (GROVE_SEEDLING, grove(Vec3::ZERO)),
        // Every tree mature from the first frame: the seasons are the clip.
        Scene::Seasons | Scene::Shake => {
            (1.0, grove(Vec3::ZERO).into_iter().map(|(p, at, _, frames)| (p, at, 0, frames)).collect())
        }
        Scene::Fell => (1.0, fell_grove()),
    };
    capture.trees = trees.iter().map(|&(_, at, ..)| at).collect();
    for (params, at, start, frames) in trees {
        let mature_at = build_tree(&params, f32::INFINITY).mature_at;
        let tree = commands
            .spawn((
                ProceduralTree(params),
                TreeAge(0.0),
                Growth { start, frames, seedling, mature_at },
                Transform::from_translation(at),
            ))
            .id();
        capture.entities.push(Some(tree));
        if gpu.0 {
            commands.entity(tree).insert(GpuGrowth);
        }
    }
    commands.spawn((
        Camera3d::default(),
        RenderTarget::Image(capture.target.clone().into()),
        camera_at(&capture, 0),
        OrbitCam,
    ));
}

/// Camera transform at fraction `t` (0..1) of the clip.
fn orbit(capture: &Capture, t: f32) -> Transform {
    // The sun is at +X+Z (yaw 1/8 turn); the orbit is centred on it so the
    // trees stay front-lit.
    let yaw = (0.125 - 0.5 * ORBIT_TURNS + t * ORBIT_TURNS) * TAU;
    let pitch = 12f32.to_radians();
    let dir = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos());
    Transform::from_translation(capture.focus + dir * capture.distance).looking_at(capture.focus, Vec3::Y)
}

/// The camera at `frame`: still for the shake and fell clips, so the trees are what moves,
/// orbiting for the others.
fn camera_at(capture: &Capture, frame: u32) -> Transform {
    match capture.scene {
        Scene::Shake => orbit(capture, 0.5),
        // From +x, square to both falls; the sun stands behind the camera's left shoulder.
        Scene::Fell => {
            let pitch = 14f32.to_radians();
            let dir = Vec3::new(pitch.cos(), pitch.sin(), 0.0);
            Transform::from_translation(capture.focus + dir * capture.distance).looking_at(capture.focus, Vec3::Y)
        }
        _ => orbit(capture, frame as f32 / (TOTAL_FRAMES - 1) as f32),
    }
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    mut capture: ResMut<Capture>,
    mut trees: Query<(&Growth, &mut TreeAge)>,
    mut camera: Single<&mut Transform, With<OrbitCam>>,
    screenshots: Query<(), With<Screenshot>>,
    mut strategy: ResMut<TimeUpdateStrategy>,
    mut wind: ResMut<FloraWind>,
    mut season: ResMut<FloraSeason>,
    time: Res<Time>,
    mut exit: MessageWriter<AppExit>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shakes: ResMut<TreeShakes>,
) {
    // The clock stands still, except for the one step the frame below asks for.
    *strategy = TimeUpdateStrategy::ManualDuration(Duration::ZERO);
    if capture.in_flight {
        if !screenshots.is_empty() {
            return;
        }
        capture.in_flight = false;
        capture.frame += 1;
        if capture.frame == TOTAL_FRAMES {
            println!("capture_growth: wrote {TOTAL_FRAMES} frames to {}", capture.out.display());
            exit.write(AppExit::Success);
            return;
        }
        capture.wait = SETTLE_FRAMES;
        // The next update advances the clock to this frame's time.
        *strategy = TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / FPS));
    }

    let frame = capture.frame;
    for (growth, mut age) in &mut trees {
        let next = growth.age(frame);
        if age.0 != next {
            age.0 = next;
        }
    }
    **camera = camera_at(&capture, frame);
    if matches!(capture.scene, Scene::Seasons) {
        // From late summer through autumn, leaf fall and the snow into the next summer.
        let t = frame as f32 / (TOTAL_FRAMES - 1) as f32;
        let next = FloraSeason::of_year(season_year(t));
        if *season != next {
            *season = next;
            if let Some(mut ground) = materials.get_mut(&capture.ground) {
                ground.base_color = ground_colour(next.snow);
            }
        }
    }
    // The shake clip's knocks, each once, at its frame.
    while let Some(&(at_frame, tree, radius, strength, duration)) = SHAKES.get(capture.shaken) {
        if !matches!(capture.scene, Scene::Shake) || frame < at_frame {
            break;
        }
        capture.shaken += 1;
        let (at, direction) = match tree {
            // An axe on one trunk, swinging it across the view.
            Some(i) => (capture.trees[i].xz(), Vec2::new(-1.0, 1.0)),
            // A blast at the grove's middle: every tree swings away from it.
            None => (capture.focus.xz(), Vec2::ZERO),
        };
        shakes.shake(TreeShake { at, radius, strength, direction, duration, ..default() }, &time);
    }
    // The fell clip's cuts, each once, at its frame.
    while let Some(&(at_frame, tree, bearing)) = FELLS.get(capture.felled) {
        if !matches!(capture.scene, Scene::Fell) || frame < at_frame {
            break;
        }
        capture.felled += 1;
        let (sin, cos) = bearing.to_radians().sin_cos();
        if let Some(entity) = capture.entities[tree] {
            commands.entity(entity).insert(FellTree { direction: Vec2::new(cos, sin), ..default() });
        }
    }
    if matches!(capture.scene, Scene::Grove) && frame >= GUST_FRAME && !capture.gusted {
        capture.gusted = true;
        // From well upwind of the grove, along the wind, across all of it.
        let upwind = capture.focus.xz() - capture.wind * (0.5 * capture.distance);
        wind.launch_gust(
            GustFront {
                origin: upwind,
                direction: capture.wind,
                speed: GUST_SPEED,
                width: GUST_WIDTH,
                strength: GUST_STRENGTH,
                ..default()
            },
            &time,
        );
    }

    if capture.wait > 0 {
        capture.wait -= 1;
        return;
    }
    let path = capture.out.join(format!("frame_{frame:04}.png"));
    commands
        .spawn(Screenshot::image(capture.target.clone()))
        .observe(save_to_disk(path));
    capture.in_flight = true;
}
