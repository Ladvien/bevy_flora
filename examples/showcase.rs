//! Interactive showcase: a tree that grows from a seedling, with preset cycling,
//! reseeding and an orbit camera.
//!
//! Controls: right-drag orbit, wheel zoom, `[`/`]` previous/next preset,
//! `N` reseed, `Space` toggle growing, `R` restart growth, `M` mature, `G` rebuild,
//! `U` grow in the vertex shader (`GpuGrowth`) or rebuild on the CPU, `C` show the gameplay shape (`TreeShape`),
//! `Y` run the year (`FloraSeason`): the leaves turn, fall and come back, `H` hit the tree (`TreeShakes`).

use std::f32::consts::PI;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy_flora::{
    FloraPlugin, FloraSeason, GpuGrowth, Growing, ProceduralTree, TreeAge, TreeGenerated, TreePreset, TreeShake,
    TreeShakes, TreeShape,
};

#[derive(Resource)]
struct Showcase {
    /// Index into `TreePreset::ALL`.
    preset: usize,
    seed: i32,
    /// How far through the year the season is (`FloraSeason::of_year`), and whether it runs.
    year: f32,
    year_running: bool,
}

/// Seconds a year takes in the showcase while it runs.
const YEAR_SECONDS: f32 = 40.0;

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
        .insert_resource(Showcase { preset: 0, seed: 42, year: 0.3, year_running: false })
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_keys, advance_year, frame_tree, orbit_camera, update_hud, draw_shape).chain())
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
    tree: Single<(Entity, &mut ProceduralTree, &mut TreeAge, Option<&Growing>, Has<GpuGrowth>, Has<TreeShape>)>,
    mut shakes: ResMut<TreeShakes>,
    time: Res<Time>,
) {
    let (entity, mut tree, mut age, growing, gpu, shaped) = tree.into_inner();
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
    if keys.just_pressed(KeyCode::KeyC) {
        if shaped {
            commands.entity(entity).remove::<TreeShape>();
        } else {
            commands.entity(entity).insert(TreeShape::default());
        }
    }
    if keys.just_pressed(KeyCode::KeyY) {
        showcase.year_running = !showcase.year_running;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        // An axe on the trunk: the tree (at the origin) swings, its neighbours would not.
        let hit = TreeShake { at: Vec2::ZERO, radius: 1.0, strength: 2.0, direction: Vec2::X, ..default() };
        shakes.shake(hit, &time);
    }
}

/// While the year runs, move it on and set the season from it.
fn advance_year(time: Res<Time>, mut showcase: ResMut<Showcase>, mut season: ResMut<FloraSeason>) {
    if showcase.year_running {
        showcase.year = (showcase.year + time.delta_secs() / YEAR_SECONDS).fract();
        *season = FloraSeason::of_year(showcase.year);
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

#[allow(clippy::type_complexity)]
fn update_hud(
    showcase: Res<Showcase>,
    season: Res<FloraSeason>,
    tree: Single<(&ProceduralTree, &TreeAge, Option<&Growing>, Has<GpuGrowth>, Option<&TreeShape>)>,
    mut hud: Single<&mut Text, With<Hud>>,
) {
    let (tree, age, growing, gpu, shape) = tree.into_inner();
    let maturity = tree.0.growth.maturity_age;
    let age_text = if age.0.is_finite() {
        format!("{:.1}", age.0)
    } else {
        "mature".to_string()
    };
    let shape_text = shape.map_or("off".to_string(), |shape| {
        format!("{} capsules, {:.1} m tall", shape.capsules().count(), shape.height())
    });
    let leaves = if tree.0.foliage.autumn.is_none() {
        "evergreen".to_string()
    } else {
        format!("turned {:.0}%, fallen {:.0}%", season.autumn * 100.0, season.leaf_fall * 100.0)
    };
    let season_text = format!("{leaves}, snow {:.0}%", season.snow * 100.0);
    hud.0 = format!(
        "preset: {} ({}/{})\nseed: {}\nage: {age_text} / {maturity}\ngrowing: {}\ngrowth: {}\ncolliders: {shape_text}\n\
         season: {season_text}{}\n\n\
         right-drag: orbit   wheel: zoom\n[ / ]: preset   N: reseed\n\
         Space: toggle growing   R: restart   M: mature   G: rebuild\n\
         U: grow in the vertex shader or rebuild on the CPU   C: show the gameplay shape\n\
         Y: run the year (or stop it)   H: hit the tree",
        showcase.preset().name(),
        showcase.preset + 1,
        TreePreset::ALL.len(),
        showcase.seed,
        if growing.is_some() { "on" } else { "off" },
        if gpu { "vertex shader (GpuGrowth)" } else { "CPU rebuild" },
        if showcase.year_running { " (the year runs)" } else { "" },
    );
}

/// The tree's `TreeShape`, when shown: capsules orange, the crown's box cyan.
fn draw_shape(mut gizmos: Gizmos, shapes: Query<(&GlobalTransform, &TreeShape)>) {
    for (transform, shape) in &shapes {
        let (_, rotation, translation) = transform.to_scale_rotation_translation();
        let tree = Isometry3d::new(translation, rotation);
        for capsule in shape.capsules() {
            let (primitive, local) = capsule.to_primitive();
            gizmos.primitive_3d(&primitive, tree * local, Color::srgb(1.0, 0.45, 0.05));
        }
        if let Some((center, half)) = shape.crown {
            let cuboid = Cuboid { half_size: half };
            gizmos.primitive_3d(&cuboid, tree * Isometry3d::from_translation(center), Color::srgb(0.1, 0.9, 1.0));
        }
    }
}
