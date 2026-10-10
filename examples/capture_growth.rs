//! Records the README clip: one tree growing from a bare seedling to maturity
//! while the camera orbits it, rendered offscreen to numbered PNG frames.
//!
//! ```text
//! cargo run --release --example capture_growth -- [--out <dir>] [--preset <name>]
//! ffmpeg -framerate 24 -i <dir>/frame_%04d.png \
//!   -vf "fps=24,scale=480:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse" docs/growth.gif
//! ```
//!
//! The age is set explicitly per frame (no wall clock), so two runs write the
//! same frames. `--out` defaults to `target/capture/growth`, `--preset` to `oak`.

use std::f32::consts::TAU;
use std::fs;
use std::path::PathBuf;

use bevy::app::AppExit;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy_flora::{FloraPlugin, ProceduralTree, TreeAge, TreePreset, build_tree};

const SIZE: UVec2 = UVec2::new(800, 600);
/// Frames over which the tree grows from age 0 to maturity.
const GROW_FRAMES: u32 = 144;
/// Frames the mature tree is held at the end (the orbit continues).
const HOLD_FRAMES: u32 = 48;
/// Frames rendered before the first capture so meshes and shadows exist.
const WARMUP_FRAMES: u32 = 10;
/// Frames between setting an age and capturing it (mesh upload, shadows).
const SETTLE_FRAMES: u32 = 2;
/// Camera orbit over the whole clip, in turns.
const ORBIT_TURNS: f32 = 0.35;

#[derive(Resource)]
struct Capture {
    out: PathBuf,
    target: Handle<Image>,
    mature_at: f32,
    /// Orbit radius and look-at point, framed on the mature tree.
    distance: f32,
    focus: Vec3,
    frame: u32,
    wait: u32,
    in_flight: bool,
}

#[derive(Component)]
struct OrbitCam;

fn usage() -> ! {
    eprintln!("usage: capture_growth [--out <dir>] [--preset <name>]");
    std::process::exit(2);
}

fn main() {
    let mut out = PathBuf::from("target/capture/growth");
    let mut preset = TreePreset::Oak;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out = args.next().unwrap_or_else(|| usage()).into(),
            "--preset" => {
                let name = args.next().unwrap_or_else(|| usage());
                preset = TreePreset::ALL
                    .into_iter()
                    .find(|p| p.name() == name)
                    .unwrap_or_else(|| usage());
            }
            _ => usage(),
        }
    }
    fs::create_dir_all(&out).expect("create output directory");

    let params = preset.params();
    let mature = build_tree(&params, f32::INFINITY);
    let (min, max) = mature
        .trunk
        .positions
        .iter()
        .chain(&mature.foliage.positions)
        .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    let radius = (0.5 * (max - min).length()).max(0.5);
    let focus = Vec3::new(0.0, 0.5 * (min.y + max.y), 0.0);

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
        .insert_resource(Capture {
            out,
            target: Handle::default(),
            mature_at: mature.mature_at,
            distance: radius / (std::f32::consts::PI / 8.0).sin() * 0.8,
            focus,
            frame: 0,
            wait: WARMUP_FRAMES,
            in_flight: false,
        })
        .insert_resource(SelectedPreset(params))
        .add_systems(Startup, setup)
        .add_systems(Update, drive)
        .run();
}

#[derive(Resource)]
struct SelectedPreset(bevy_flora::TreeParams);

fn setup(
    mut commands: Commands,
    mut capture: ResMut<Capture>,
    preset: Res<SelectedPreset>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    capture.target = images.add(Image::new_target_texture(
        SIZE.x,
        SIZE.y,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 10_000.0,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(200.0, 200.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.45, 0.30),
            perceptual_roughness: 1.0,
            ..default()
        })),
    ));
    commands.spawn((ProceduralTree(preset.0.clone()), TreeAge(0.0)));
    commands.spawn((
        Camera3d::default(),
        RenderTarget::Image(capture.target.clone().into()),
        orbit(&capture, 0.0),
        OrbitCam,
    ));
}

/// Camera transform at fraction `t` (0..1) of the clip.
fn orbit(capture: &Capture, t: f32) -> Transform {
    // The sun is at +X+Z (yaw 1/8 turn); the orbit is centred on it so the
    // tree stays front-lit.
    let yaw = (0.125 - 0.5 * ORBIT_TURNS + t * ORBIT_TURNS) * TAU;
    let pitch = 12f32.to_radians();
    let dir = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos());
    Transform::from_translation(capture.focus + dir * capture.distance)
        .looking_at(capture.focus, Vec3::Y)
}

fn drive(
    mut commands: Commands,
    mut capture: ResMut<Capture>,
    mut tree: Single<&mut TreeAge>,
    mut camera: Single<&mut Transform, With<OrbitCam>>,
    screenshots: Query<(), With<Screenshot>>,
    mut exit: MessageWriter<AppExit>,
) {
    let total = GROW_FRAMES + HOLD_FRAMES;
    if capture.in_flight {
        if !screenshots.is_empty() {
            return;
        }
        capture.in_flight = false;
        capture.frame += 1;
        if capture.frame == total {
            println!("capture_growth: wrote {total} frames to {}", capture.out.display());
            exit.write(AppExit::Success);
            return;
        }
        capture.wait = SETTLE_FRAMES;
    }

    let progress = (capture.frame as f32 / (GROW_FRAMES - 1) as f32).min(1.0);
    // Ease out so the early, fast-changing years get more frames.
    let age = capture.mature_at * (1.0 - (1.0 - progress).powi(2));
    if tree.0 != age {
        tree.0 = age;
    }
    **camera = orbit(&capture, capture.frame as f32 / (total - 1) as f32);

    if capture.wait > 0 {
        capture.wait -= 1;
        return;
    }
    let path = capture.out.join(format!("frame_{:04}.png", capture.frame));
    commands
        .spawn(Screenshot::image(capture.target.clone()))
        .observe(save_to_disk(path));
    capture.in_flight = true;
}
