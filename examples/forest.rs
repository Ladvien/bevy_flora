//! Forest-scale benchmark: how many trees can be drawn at what frame rate.
//!
//! ```text
//! cargo run --release --example forest -- [--trees <n>] [--mode <naive|pool|gpu|gpu-live|cpu-live>]
//!     [--preset <name>] [--frames <n>] [--warmup-secs <s>] [--spacing <m>]
//!     [--no-impostors] [--lod <lod1,lod2,impostor>]
//!     [--sun <x,y,z>] [--wind <strength>] [--wind-dir <x,z>] [--gust <speed>]
//!     [--eye <x,y,z>] [--at <x,y,z>] [--fov <deg>]
//!     [--shadows <off|distance,cascades>] [--wind-animate] [--deterministic]
//!     [--offscreen] [--screenshot <path>]
//! ```
//!
//! Trees stand on a square grid with a deterministic jitter and a random yaw. A
//! fixed camera 30 m above one corner looks across the grid toward its centre,
//! so near, mid and far trees are all on screen. The window is 1920x1080 with
//! vsync off and one shadow-casting directional light.
//!
//! `--mode naive` gives every tree its own meshes (`ProceduralTree`, today's
//! path without growth). `--mode pool` draws the same forest from a
//! `TreePool`: a few seed variants shared by every tree, with mesh levels of
//! detail and, beyond the last of them, instanced impostors (`--no-impostors`
//! leaves those out; `--lod 30,80,200` sets the distances, in metres, at which
//! each takes over). The impostors are lit by the scene's own sun, so `--sun <x,y,z>` (the
//! sun's position, looking at the origin; default `-20,25,20`) moves their light too, and
//! `--wind <strength>` (0 still to 1 strong; the library's default if not given) sways the
//! trees, leaves and impostors alike; `--wind-dir <x,z>` turns the wind. `--gust <speed>`
//! sends a gust front across the grid along +x, from its edge, when the trees are ready.
//! `--wind-animate` changes the wind's strength every frame, as a game with weather would.
//! `--deterministic` steps the clock a fixed 1/60 s per frame and counts `--warmup-secs` as
//! 60 frames a second, so a `--screenshot` of two versions of the code is the same image
//! wherever they draw the same (compare the PNGs byte for byte).
//! `--eye <x,y,z>` and `--at <x,y,z>` move the camera and what it looks at (world metres, the
//! grid centred on the origin), and `--fov <deg>` sets its vertical field of view (45 by default).
//! `--mode gpu` and `--mode gpu-live` draw the trees as `ProceduralTree` with `GpuGrowth`,
//! fully grown or still growing through the whole run; `--mode cpu-live` grows them on the
//! CPU, rebuilding every mesh each frame.
//!
//! After the trees exist and `--warmup-secs` have passed (shaders compile in the
//! background), the example times `--frames` frames and prints one line:
//!
//! ```text
//! forest pool oak 100000 trees: 12.3 ms/frame (81.3 fps), p95 14.0 ms, median 11.9 ms, min 9.0 ms, cpu 18.2 ms/frame
//! ```
//!
//! `cpu` is the CPU time of every thread per measured frame. It does not grow when other
//! programs compete for the machine, and it is far below the frame time when the GPU is the
//! limit, so it tells the two apart and compares runs on a machine that is not quiet.
//!
//! With `--screenshot <path>` the camera renders into an offscreen image
//! instead, which is saved to `path` once the forest is ready; nothing is timed.
//!
//! `--shadows off` turns shadow maps off; `--shadows 120,2` limits them to 120 m
//! in 2 cascades (Bevy's default reaches 150 m in 4), which keeps the far trees
//! out of the shadow maps.
//!
//! Display note: the window runs flat out (`WinitSettings::continuous`), but on
//! macOS the compositor still paces a windowed app to the display's refresh rate,
//! so frame times below one refresh interval read as the interval. `--offscreen`
//! has no window: the camera renders into an image, nothing is presented, and the
//! frame time is what the CPU and the GPU actually take.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::app::AppExit;
use bevy::camera::RenderTarget;
use bevy::light::{CascadeShadowConfig, CascadeShadowConfigBuilder};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{PresentMode, WindowResolution};
use bevy_flora::{
    FloraPlugin, FloraWind, GpuGrowth, Growing, GustFront, LodDistances, LodSettings, PooledTree,
    ProceduralTree, SeededRng, TreeAge, TreeParams, TreePool, TreePreset, build_pool,
};

const WINDOW: UVec2 = UVec2::new(1920, 1080);
/// Seeds the forest cycles through; the naive forest uses the same ones as the pool.
const VARIANTS: u32 = 8;
/// Height of the camera above the ground, in metres.
const CAMERA_HEIGHT: f32 = 30.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Naive,
    Pool,
    /// `ProceduralTree` with `GpuGrowth`, fully grown.
    Gpu,
    /// `ProceduralTree` with `GpuGrowth`, still growing every frame of the run.
    GpuLive,
    /// `ProceduralTree` without `GpuGrowth`, still growing every frame of the run.
    CpuLive,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Naive => "naive",
            Mode::Pool => "pool",
            Mode::Gpu => "gpu",
            Mode::GpuLive => "gpu-live",
            Mode::CpuLive => "cpu-live",
        }
    }
}

/// How the directional light's shadow maps are set up.
#[derive(Clone, Copy, PartialEq)]
enum Shadows {
    /// Bevy's defaults.
    Default,
    Off,
    /// Out to this many metres, in this many cascades.
    Reach(f32, usize),
}

struct Args {
    trees: u32,
    mode: Mode,
    preset: TreePreset,
    frames: usize,
    warmup: Duration,
    spacing: f32,
    impostors: bool,
    /// Where the levels of detail and the impostor take over, if not the defaults.
    lod: Option<[f32; 3]>,
    /// How the sun's shadow maps are set up.
    shadows: Shadows,
    /// Where the sun stands; it looks at the origin.
    sun: Vec3,
    /// Where the camera stands and what it looks at, if not the default corner view.
    eye: Option<Vec3>,
    at: Option<Vec3>,
    /// Vertical field of view, degrees.
    fov: f32,
    /// Strength of the wind, if not the library's default.
    wind: Option<f32>,
    /// Where the wind blows toward (world x and z), if not the library's default.
    wind_dir: Option<Vec2>,
    /// Speed of a gust front to send across the grid when the trees are ready, if any.
    gust: Option<f32>,
    /// Change the wind's strength every frame, as a game with weather would.
    wind_animate: bool,
    /// Step the clock a fixed 1/60 s per frame and count the warmup in frames, so two runs of
    /// different code render the same image at the same frame.
    deterministic: bool,
    /// Render into an image, with no window.
    offscreen: bool,
    screenshot: Option<PathBuf>,
}

impl Args {
    /// Whether the camera draws into an image instead of a window.
    fn is_offscreen(&self) -> bool {
        self.offscreen || self.screenshot.is_some()
    }
}

fn usage() -> ! {
    eprintln!(
        "usage: forest [--trees <n>] [--mode <naive|pool|gpu|gpu-live|cpu-live>] [--preset <name>] [--frames <n>] \
         [--warmup-secs <s>] [--spacing <m>] [--no-impostors] [--lod <a,b,c>] \
         [--shadows <off|distance,cascades>] [--sun <x,y,z>] [--wind <strength>] \
         [--eye <x,y,z>] [--at <x,y,z>] [--fov <deg>] \
         [--wind-dir <x,z>] [--gust <speed>] [--wind-animate] [--deterministic] \
         [--offscreen] [--screenshot <path>]"
    );
    std::process::exit(2);
}

fn parse_args() -> Args {
    let mut args = Args {
        trees: 10_000,
        mode: Mode::Pool,
        preset: TreePreset::Oak,
        frames: 600,
        warmup: Duration::from_secs(5),
        spacing: 8.0,
        impostors: true,
        lod: None,
        shadows: Shadows::Default,
        sun: Vec3::new(-20.0, 25.0, 20.0),
        eye: None,
        at: None,
        fov: 45.0,
        wind: None,
        wind_dir: None,
        gust: None,
        wind_animate: false,
        deterministic: false,
        offscreen: false,
        screenshot: None,
    };
    let mut it = std::env::args().skip(1);
    fn value<T: std::str::FromStr>(it: &mut impl Iterator<Item = String>) -> T {
        it.next()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| usage())
    }
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--trees" => args.trees = value(&mut it),
            "--mode" => {
                args.mode = match it.next().as_deref() {
                    Some("naive") => Mode::Naive,
                    Some("pool") => Mode::Pool,
                    Some("gpu") => Mode::Gpu,
                    Some("gpu-live") => Mode::GpuLive,
                    Some("cpu-live") => Mode::CpuLive,
                    _ => usage(),
                }
            }
            "--preset" => {
                let name = it.next().unwrap_or_else(|| usage());
                args.preset = TreePreset::ALL
                    .into_iter()
                    .find(|p| p.name() == name)
                    .unwrap_or_else(|| usage());
            }
            "--frames" => args.frames = value(&mut it),
            "--warmup-secs" => args.warmup = Duration::from_secs_f32(value(&mut it)),
            "--spacing" => args.spacing = value(&mut it),
            "--no-impostors" => args.impostors = false,
            "--offscreen" => args.offscreen = true,
            "--shadows" => {
                args.shadows = match it.next().as_deref() {
                    Some("off") => Shadows::Off,
                    Some(reach) => {
                        let parts: Vec<&str> = reach.split(',').collect();
                        match parts[..] {
                            [distance, cascades] => Shadows::Reach(
                                distance.parse().unwrap_or_else(|_| usage()),
                                cascades.parse().unwrap_or_else(|_| usage()),
                            ),
                            _ => usage(),
                        }
                    }
                    None => usage(),
                }
            }
            "--sun" => {
                let list = it.next().unwrap_or_else(|| usage());
                let parts: Vec<f32> = list.split(',').filter_map(|p| p.parse().ok()).collect();
                let [x, y, z]: [f32; 3] = parts.try_into().unwrap_or_else(|_| usage());
                args.sun = Vec3::new(x, y, z);
            }
            "--eye" | "--at" => {
                let list = it.next().unwrap_or_else(|| usage());
                let parts: Vec<f32> = list.split(',').filter_map(|p| p.parse().ok()).collect();
                let [x, y, z]: [f32; 3] = parts.try_into().unwrap_or_else(|_| usage());
                if arg == "--eye" {
                    args.eye = Some(Vec3::new(x, y, z));
                } else {
                    args.at = Some(Vec3::new(x, y, z));
                }
            }
            "--fov" => args.fov = value(&mut it),
            "--wind" => args.wind = Some(value(&mut it)),
            "--wind-dir" => {
                let list = it.next().unwrap_or_else(|| usage());
                let parts: Vec<f32> = list.split(',').filter_map(|p| p.parse().ok()).collect();
                let [x, z]: [f32; 2] = parts.try_into().unwrap_or_else(|_| usage());
                args.wind_dir = Some(Vec2::new(x, z));
            }
            "--gust" => args.gust = Some(value(&mut it)),
            "--wind-animate" => args.wind_animate = true,
            "--deterministic" => args.deterministic = true,
            "--lod" => {
                let list = it.next().unwrap_or_else(|| usage());
                let parts: Vec<f32> = list.split(',').filter_map(|p| p.parse().ok()).collect();
                args.lod = Some(parts.try_into().unwrap_or_else(|_| usage()));
            }
            "--screenshot" => args.screenshot = Some(it.next().unwrap_or_else(|| usage()).into()),
            _ => usage(),
        }
    }
    if args.trees == 0 || args.frames == 0 || args.spacing <= 0.0 {
        usage();
    }
    args
}

#[derive(PartialEq, Eq)]
enum Phase {
    /// Waiting for every tree to get its render entities.
    Loading,
    /// Trees exist; rendering until the warmup time has passed.
    Warmup,
    Measuring,
    /// A screenshot was requested and has not come back yet.
    Capturing,
    /// The result was printed; the app is exiting.
    Done,
}

#[derive(Resource)]
struct Bench {
    args: Args,
    phase: Phase,
    /// When the trees all existed.
    loaded: Instant,
    /// Start of the previous frame while measuring.
    last: Option<Instant>,
    /// Process CPU seconds (all threads) when measuring began.
    cpu_start: Option<f64>,
    /// Frames rendered in the warmup so far.
    warm_frames: u32,
    /// Seconds per measured frame.
    frames: Vec<f32>,
    /// Offscreen target for `--screenshot`.
    target: Handle<Image>,
}

/// Marks a tree the benchmark spawned.
#[derive(Component)]
struct ForestTree;

fn main() {
    let args = parse_args();
    let mut app = App::new();
    // Offscreen: no window to present to, so nothing paces the frames.
    let window = if args.is_offscreen() {
        WindowPlugin {
            primary_window: None,
            exit_condition: bevy::window::ExitCondition::DontExit,
            ..default()
        }
    } else {
        WindowPlugin {
            primary_window: Some(Window {
                resolution: WindowResolution::new(WINDOW.x, WINDOW.y)
                    .with_scale_factor_override(1.0),
                present_mode: PresentMode::AutoNoVsync,
                title: "bevy_flora forest".to_string(),
                ..default()
            }),
            ..default()
        }
    };
    if args.wind.is_some() || args.wind_dir.is_some() {
        let base = FloraWind::default();
        app.insert_resource(FloraWind {
            strength: args.wind.unwrap_or(base.strength),
            direction: args.wind_dir.unwrap_or(base.direction),
            ..base
        });
    }
    if args.deterministic {
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)));
    }
    // Unfocused, Bevy's default settings update 60 times a second; a benchmark
    // must run flat out whatever has the focus.
    app.insert_resource(bevy::winit::WinitSettings::continuous())
        .add_plugins((DefaultPlugins.set(window), FloraPlugin))
        .insert_resource(ClearColor(Color::srgb(0.55, 0.72, 0.92)))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.6, 0.6, 0.6),
            brightness: 300.0,
            ..default()
        })
        .insert_resource(Bench {
            args,
            phase: Phase::Loading,
            loaded: Instant::now(),
            last: None,
            frames: Vec::new(),
            cpu_start: None,
            warm_frames: 0,
            target: Handle::default(),
        })
        .add_systems(Startup, setup)
        .add_systems(Update, drive)
        .run();
}

/// Grid side length (in trees) and the grid's extent in metres.
fn grid(trees: u32, spacing: f32) -> (u32, f32) {
    let side = (trees as f32).sqrt().ceil() as u32;
    (side, side as f32 * spacing)
}

#[allow(clippy::too_many_arguments)]
fn setup(
    mut commands: Commands,
    mut bench: ResMut<Bench>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut pools: ResMut<Assets<TreePool>>,
) {
    let (side, extent) = grid(bench.args.trees, bench.args.spacing);
    let half = extent * 0.5;
    let spacing = bench.args.spacing;

    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: bench.args.shadows != Shadows::Off,
            illuminance: 10_000.0,
            ..default()
        },
        match bench.args.shadows {
            Shadows::Reach(maximum_distance, num_cascades) => CascadeShadowConfigBuilder {
                maximum_distance,
                num_cascades,
                ..default()
            }
            .build(),
            _ => CascadeShadowConfig::default(),
        },
        // High, off to one side by default: the trees are lit from the side.
        Transform::from_translation(bench.args.sun).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(extent * 1.2 + 200.0, extent * 1.2 + 200.0),
            ),
        ),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.45, 0.30),
            perceptual_roughness: 1.0,
            ..default()
        })),
    ));

    let mut camera = commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: bench.args.fov.to_radians(),
            far: extent * 2.0 + 500.0,
            ..default()
        }),
        Transform::from_translation(bench.args.eye.unwrap_or(Vec3::new(-half, CAMERA_HEIGHT, -half)))
            .looking_at(bench.args.at.unwrap_or(Vec3::new(0.0, 8.0, 0.0)), Vec3::Y),
    ));
    if bench.args.is_offscreen() {
        bench.target = images.add(Image::new_target_texture(
            WINDOW.x,
            WINDOW.y,
            TextureFormat::Rgba8Unorm,
            Some(TextureFormat::Rgba8UnormSrgb),
        ));
        camera.insert(RenderTarget::Image(bench.target.clone().into()));
    }

    let mut base = bench.args.preset.params();
    base.seed = 1;
    let pool = (bench.args.mode == Mode::Pool).then(|| {
        let mut lod = LodSettings::default();
        if let Some([lod1, lod2, impostor]) = bench.args.lod {
            lod.distances = LodDistances {
                lod1,
                lod2,
                impostor,
            };
        }
        if !bench.args.impostors {
            lod.distances.impostor = f32::INFINITY;
        }
        let started = Instant::now();
        let pool = build_pool(
            &base,
            VARIANTS,
            f32::INFINITY,
            &lod,
            &mut meshes,
        );
        eprintln!(
            "forest: built a pool of {VARIANTS} variants in {:.2} s",
            started.elapsed().as_secs_f32()
        );
        pools.add(pool)
    });
    for i in 0..bench.args.trees {
        let mut rng = SeededRng::new(i as i32);
        let jitter = spacing / 3.0;
        let x = (i % side) as f32 * spacing - half + rng.range(-jitter, jitter);
        let z = (i / side) as f32 * spacing - half + rng.range(-jitter, jitter);
        let yaw = rng.range(0.0, std::f32::consts::TAU);
        let transform = Transform::from_xyz(x, 0.0, z).with_rotation(Quat::from_rotation_y(yaw));
        match &pool {
            None => {
                let mut params: TreeParams = base.clone();
                params.seed = base.seed.wrapping_add((i % VARIANTS) as i32);
                let mut tree = commands.spawn((ProceduralTree(params), transform, ForestTree));
                match bench.args.mode {
                    Mode::Gpu => {
                        tree.insert(GpuGrowth);
                    }
                    // Young enough to keep growing for as long as the run takes.
                    Mode::GpuLive => {
                        tree.insert((GpuGrowth, TreeAge(3.0), Growing { rate: 0.01 }));
                    }
                    // The same, rebuilding the meshes on the CPU every frame.
                    Mode::CpuLive => {
                        tree.insert((TreeAge(3.0), Growing { rate: 0.01 }));
                    }
                    _ => {}
                }
            }
            Some(pool) => {
                commands.spawn((
                    PooledTree {
                        pool: pool.clone(),
                        variant: i % VARIANTS,
                    },
                    transform,
                    ForestTree,
                ));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    mut bench: ResMut<Bench>,
    unfinished: Query<(), (With<ForestTree>, Without<Children>)>,
    screenshots: Query<(), With<Screenshot>>,
    pools: Res<Assets<TreePool>>,
    mut exit: MessageWriter<AppExit>,
    mut wind: ResMut<FloraWind>,
    time: Res<Time>,
) {
    let bench = &mut *bench;
    if bench.args.wind_animate && bench.phase != Phase::Loading {
        // Mutated every frame on purpose: the wind changes, so the shared wind buffer is written
        // every frame (and no material is touched).
        wind.strength = 0.6 + 0.3 * (time.elapsed_secs() * 1.7).sin();
    }
    match bench.phase {
        Phase::Loading => {
            if unfinished.is_empty() {
                eprintln!(
                    "forest: {} trees ready after {:.1} s",
                    bench.args.trees,
                    bench.loaded.elapsed().as_secs_f32()
                );
                bench.loaded = Instant::now();
                bench.phase = Phase::Warmup;
                if let Some(speed) = bench.args.gust {
                    // From the grid's -x edge, along +x, across every tree.
                    let (_, extent) = grid(bench.args.trees, bench.args.spacing);
                    wind.launch_gust(
                        GustFront {
                            origin: Vec2::new(-extent * 0.5, 0.0),
                            direction: Vec2::X,
                            speed,
                            width: 20.0,
                            strength: 1.5,
                            ..default()
                        },
                        &time,
                    );
                }
            }
        }
        Phase::Warmup => {
            bench.warm_frames += 1;
            let warm = if bench.args.deterministic {
                (bench.warm_frames as f32 / 60.0) < bench.args.warmup.as_secs_f32()
            } else {
                bench.loaded.elapsed() < bench.args.warmup
            };
            if warm {
                return;
            }
            // Trees wait for their impostors, so a pool without them here means the
            // bake went wrong (a shader that does not compile, a scene that never rendered).
            if bench.args.mode == Mode::Pool && bench.args.impostors {
                let baked = pools
                    .iter()
                    .all(|(_, pool)| pool.variants.iter().all(|v| v.impostor.is_some()));
                if !baked {
                    eprintln!("forest: the impostors were not baked");
                    exit.write(AppExit::error());
                    return;
                }
            }
            if let Some(path) = bench.args.screenshot.clone() {
                commands
                    .spawn(Screenshot::image(bench.target.clone()))
                    .observe(save_to_disk(path));
                bench.phase = Phase::Capturing;
            } else {
                bench.phase = Phase::Measuring;
                bench.cpu_start = cpu_seconds();
            }
        }
        Phase::Measuring => {
            let now = Instant::now();
            if let Some(last) = bench.last.replace(now) {
                bench.frames.push((now - last).as_secs_f32());
            }
            if bench.frames.len() >= bench.args.frames {
                report(bench);
                bench.phase = Phase::Done;
                exit.write(AppExit::Success);
            }
        }
        Phase::Capturing => {
            // `save_to_disk` despawns the screenshot entity once it has written the file.
            if screenshots.is_empty() {
                exit.write(AppExit::Success);
            }
        }
        Phase::Done => {}
    }
}

fn report(bench: &Bench) {
    let mut sorted = bench.frames.clone();
    sorted.sort_by(f32::total_cmp);
    let mean = bench.frames.iter().sum::<f32>() / bench.frames.len() as f32;
    let p95 = sorted[((sorted.len() as f32 * 0.95) as usize).min(sorted.len() - 1)];
    let median = sorted[sorted.len() / 2];
    // CPU time of every thread per frame: unlike the frame time it does not grow when other
    // programs compete for the machine, and it is far below the frame time when the GPU is the
    // limit.
    let cpu = match (bench.cpu_start, cpu_seconds()) {
        (Some(start), Some(end)) => format!("{:.2}", (end - start) * 1000.0 / bench.frames.len() as f64),
        _ => "?".to_string(),
    };
    println!(
        "forest {} {} {} trees: {:.2} ms/frame ({:.1} fps), p95 {:.2} ms, median {:.2} ms, min {:.2} ms, cpu {} ms/frame",
        bench.args.mode.name(),
        bench.args.preset.name(),
        bench.args.trees,
        mean * 1000.0,
        1.0 / mean,
        p95 * 1000.0,
        median * 1000.0,
        sorted[0] * 1000.0,
        cpu,
    );
}

/// CPU time this process has used so far, all threads, in seconds (`ps`, 10 ms resolution).
fn cpu_seconds() -> Option<f64> {
    let out = std::process::Command::new("ps")
        .args(["-o", "cputime=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    // `[hh:]mm:ss.cc`
    let mut seconds = 0.0;
    for part in text.trim().split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(seconds)
}
