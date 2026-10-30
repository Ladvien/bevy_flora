//! Throwaway (iteration 1, D-073): two pines, one at scale 1 and one at scale `s`, side by side under
//! a steady wind, photographed from the side with an orthographic camera. Deleted before commit.
//!
//! zz_probe <out.png> <wind strength> <scale>

use std::time::Duration;

use bevy::app::AppExit;
use bevy::camera::{RenderTarget, ScalingMode};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::time::TimeUpdateStrategy;
use bevy_flora::{FloraPlugin, FloraWind, ProceduralTree, TreeAge, TreePreset};

#[derive(Resource)]
struct Probe {
    out: String,
    scale: f32,
    target: Handle<Image>,
    frames: u32,
    shot: bool,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args[1].clone();
    let wind: f32 = args[2].parse().unwrap_or(0.0);
    let scale: f32 = args[3].parse().unwrap_or(2.0);
    App::new()
        .add_plugins((DefaultPlugins, FloraPlugin))
        .insert_resource(ClearColor(Color::srgb(0.0, 0.0, 1.0)))
        .insert_resource(FloraWind {
            strength: wind,
            direction: Vec2::X,
            gust_strength: 0.0,
            turbulence: 0.0,
            flutter: 0.0,
            gust: None,
            ..default()
        })
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)))
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Probe { out, scale, target: Handle::default(), frames: 0, shot: false })
        .add_systems(Startup, setup)
        .add_systems(Update, drive)
        .run();
}

fn setup(mut commands: Commands, mut probe: ResMut<Probe>, mut images: ResMut<Assets<Image>>) {
    probe.target = images.add(Image::new_target_texture(1600, 1000, TextureFormat::Rgba8Unorm, Some(TextureFormat::Rgba8UnormSrgb)));
    commands.spawn((DirectionalLight { illuminance: 10_000.0, ..default() }, Transform::from_xyz(5.0, 20.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y)));
    let s = probe.scale;
    let pine = TreePreset::Pine.params();
    commands.spawn((ProceduralTree(pine.clone()), TreeAge(f32::INFINITY), Transform::from_xyz(-8.0, 0.0, 0.0)));
    commands.spawn((ProceduralTree(pine), TreeAge(f32::INFINITY), Transform::from_xyz(8.0, 0.0, 0.0).with_scale(Vec3::splat(s))));
    commands.spawn((
        Camera3d::default(),
        RenderTarget::Image(probe.target.clone().into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical { viewport_height: 12.0 * s + 4.0 },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(4.0, 6.0 * s, 200.0).looking_at(Vec3::new(4.0, 6.0 * s, 0.0), Vec3::Y),
    ));
}

fn drive(mut commands: Commands, mut probe: ResMut<Probe>, unfinished: Query<(), (With<ProceduralTree>, Without<Children>)>, shots: Query<(), With<Screenshot>>, mut exit: MessageWriter<AppExit>) {
    if !unfinished.is_empty() {
        return;
    }
    probe.frames += 1;
    if probe.frames == 600 && !probe.shot {
        probe.shot = true;
        let out = probe.out.clone();
        commands.spawn(Screenshot::image(probe.target.clone())).observe(save_to_disk(out));
    }
    if probe.shot && probe.frames > 610 && shots.is_empty() {
        exit.write(AppExit::Success);
    }
}
