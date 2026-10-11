//! Interactive showcase: a tree that grows from a seedling, with preset cycling,
//! reseeding and an orbit camera.
//!
//! Controls: right-drag orbit, wheel zoom, `[`/`]` previous/next preset,
//! `N` reseed, `Space` toggle growing, `R` restart growth, `M` mature, `G` rebuild,
//! `U` grow in the vertex shader (`GpuGrowth`) or rebuild on the CPU.

use std::f32::consts::PI;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy_flora::{FloraPlugin, GpuGrowth, Growing, ProceduralTree, TreeAge, TreeGenerated, TreePreset};

#[derive(Resource)]
struct Showcase {
    /// Index into `TreePreset::ALL`.
    preset: usize,
    seed: i32,
}

impl Showcase {
    fn preset(&self) -> TreePreset {
        TreePreset::ALL[self.preset]
    }
}

#[derive(Component)]
struct OrbitCamera {
    target: Vec3,
    yaw: f32,
    pitch: f32,
    distance: f32,
    min_distance: f32,
    max_distance: f32,
    /// Tree height the camera was last framed on.
    framed_height: f32,
}

#[derive(Component)]
struct Hud;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, FloraPlugin))
        .insert_resource(ClearColor(Color::srgb(0.55, 0.72, 0.92)))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.6, 0.6, 0.6),
            brightness: 300.0,
            ..default()
        })
        .insert_resource(Showcase { preset: 0, seed: 42 })
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_keys, frame_tree, orbit_camera, update_hud).chain())
        .run();
}

fn setup(mut commands: Commands, showcase: Res<Showcase>) {
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 10_000.0,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let mut params = showcase.preset().params();
    params.seed = showcase.seed;
    commands.spawn((ProceduralTree(params), TreeAge(0.0), Growing { rate: 1.0 }));

    let camera = OrbitCamera {
        target: Vec3::new(0.0, 2.5, 0.0),
        yaw: PI / 4.0,
        pitch: PI / 5.0,
        distance: 30.0,
        min_distance: 5.0,
        max_distance: 100.0,
        framed_height: 0.0,
    };
    commands.spawn((Camera3d::default(), camera_transform(&camera), camera));

    commands.spawn((
        Hud,
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            ..default()
        },
    ));
}

fn camera_transform(cam: &OrbitCamera) -> Transform {
    let offset = Vec3::new(
        cam.pitch.cos() * cam.yaw.sin(),
        cam.pitch.sin(),
        cam.pitch.cos() * cam.yaw.cos(),
    ) * cam.distance;
    Transform::from_translation(cam.target + offset).looking_at(cam.target, Vec3::Y)
}

#[allow(clippy::type_complexity)]
fn handle_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut showcase: ResMut<Showcase>,
    mut commands: Commands,
    tree: Single<(Entity, &mut ProceduralTree, &mut TreeAge, Option<&Growing>, Has<GpuGrowth>)>,
) {
    let (entity, mut tree, mut age, growing, gpu) = tree.into_inner();
    let count = TreePreset::ALL.len();

    let mut preset_changed = false;
    if keys.just_pressed(KeyCode::BracketRight) {
        showcase.preset = (showcase.preset + 1) % count;
        preset_changed = true;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        showcase.preset = (showcase.preset + count - 1) % count;
        preset_changed = true;
    }
    if keys.just_pressed(KeyCode::KeyN) {
        showcase.seed += 1;
        tree.0.seed = showcase.seed;
    }
    if preset_changed {
        let mut params = showcase.preset().params();
        params.seed = showcase.seed;
        tree.0 = params;
        age.0 = 0.0;
    }
    if keys.just_pressed(KeyCode::Space) {
        if growing.is_some() {
            commands.entity(entity).remove::<Growing>();
        } else {
            commands.entity(entity).insert(Growing { rate: 1.0 });
        }
    }
    if keys.just_pressed(KeyCode::KeyR) {
        age.0 = 0.0;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        age.0 = f32::INFINITY;
    }
    if keys.just_pressed(KeyCode::KeyG) {
        tree.set_changed();
    }
    if keys.just_pressed(KeyCode::KeyU) {
        if gpu {
            commands.entity(entity).remove::<GpuGrowth>();
        } else {
            commands.entity(entity).insert(GpuGrowth);
        }
    }
}

/// Re-frame the camera when the tree height changes noticeably.
fn frame_tree(mut generated: MessageReader<TreeGenerated>, mut camera: Single<&mut OrbitCamera>) {
    for msg in generated.read() {
        if (msg.height - camera.framed_height).abs() > 0.5 {
            camera.framed_height = msg.height;
            camera.target = Vec3::new(0.0, msg.height / 2.0, 0.0);
            camera.distance = (msg.height * 1.5).clamp(camera.min_distance, camera.max_distance);
        }
    }
}

fn orbit_camera(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    camera: Single<(&mut OrbitCamera, &mut Transform)>,
) {
    let (mut cam, mut transform) = camera.into_inner();
    if buttons.pressed(MouseButton::Right) {
        cam.yaw -= motion.delta.x * 0.005;
        cam.pitch = (cam.pitch + motion.delta.y * 0.005).clamp(-PI / 2.0 + 0.1, PI / 2.0 - 0.1);
    }
    if scroll.delta.y != 0.0 {
        cam.distance = (cam.distance - scroll.delta.y * 2.0).clamp(cam.min_distance, cam.max_distance);
    }
    *transform = camera_transform(&cam);
}

fn update_hud(
    showcase: Res<Showcase>,
    tree: Single<(&ProceduralTree, &TreeAge, Option<&Growing>, Has<GpuGrowth>)>,
    mut hud: Single<&mut Text, With<Hud>>,
) {
    let (tree, age, growing, gpu) = tree.into_inner();
    let maturity = tree.0.growth.maturity_age;
    let age_text = if age.0.is_finite() {
        format!("{:.1}", age.0)
    } else {
        "mature".to_string()
    };
    hud.0 = format!(
        "preset: {} ({}/{})\nseed: {}\nage: {age_text} / {maturity}\ngrowing: {}\ngrowth: {}\n\n\
         right-drag: orbit   wheel: zoom\n[ / ]: preset   N: reseed\n\
         Space: toggle growing   R: restart   M: mature   G: rebuild\n\
         U: grow in the vertex shader or rebuild on the CPU",
        showcase.preset().name(),
        showcase.preset + 1,
        TreePreset::ALL.len(),
        showcase.seed,
        if growing.is_some() { "on" } else { "off" },
        if gpu { "vertex shader (GpuGrowth)" } else { "CPU rebuild" },
    );
}
