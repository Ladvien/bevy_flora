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
//! cypress stay green. The year is a time-lapse, so its leaves are shed where they hang: falling
//! at their own speed, they would still be in the air in winter. The blossom is too few pixels to
//! win a pink in the grove's palette (it
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
//! that swings every tree, each hit knocking leaves off (`FallingLeaves`). `--fell` records a few
//! grown trees from a still camera while the oak in front is notched by four blows of an axe
//! (`TreeNotch`), felled at the notch (`FellTree`), crashes to the ground, sheds leaves and
//! leaves its stump (`docs/fell.gif`, encoded like the grove). `--fire`
//! records the grove at dusk from a still camera while a fire (`FloraFire`) lit on its upwind edge
//! runs through it, the leaves scorching and burning away and the wood charring, its embers
//! glowing and then cooling (`docs/fire.gif`, encoded like the grove). Its fire is sped up like
//! the rest of a clip (`FireSpread` 3.5 m/s, each spot burnt through in two seconds), and its
//! ground scorches by the fire's char, as a game's own ground would read it. `--push` records a
//! patch of shrubs from a still camera while a red stand-in for a character walks through it
//! (`FoliagePusher`): the leaves and stems give way around it and close up behind it
//! (`docs/push.gif`, encoded like the grove). `--chop` records a row of four trees from a still
//! camera while a lumberjack asks `FloraIndex` for the nearest standing tree, walks to its bark
//! (a yellow ring marks the tree it is after), notches it, walks round it and fells it toward the
//! notch, then the next: a felled tree leaves the index at once (`docs/chop.gif`, encoded like the
//! grove). `--harvest` records two apple trees from a still camera: the left one is shaken and
//! its apples fall, bounce and lie in the grass (`FallingFruit`), then a picker beyond the right
//! one takes the apples nearest its hand off the tree one by one (`TreeFruit::nearest`,
//! `TreeFruit::pick`; a yellow line marks each) into a basket (`docs/harvest.gif`, encoded like
//! the grove).
//!
//! Each clip is rendered as 192 frames, eight seconds at 24 fps; the grove's GIF keeps 160 of
//! them (20 fps) because its camera orbits and every leaf moves, which makes a large GIF. The
//! age of every tree and the clock are set explicitly per frame: time stands still except for
//! one 1/24 s step per captured frame, so the wind sways smoothly, a gust crosses the grove at a
//! steady pace, and two runs write the same frames. `--out` defaults to `target/capture/growth`
//! (`target/capture/grove`), `--preset` to `maple`; `--gpu` grows the trees in the vertex shader
//! instead of rebuilding their meshes (compare the two runs' frames with the `compare_gallery`
//! example).

use std::f32::consts::{FRAC_PI_2, PI, TAU};
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
use bevy::mesh::VertexAttributeValues;
use bevy::post_process::bloom::Bloom;
use bevy_flora::{
    FallingLeaves, FellTree, FireSpread, FloraFire, FloraIndex, FloraPlugin, FloraSeason, FloraWind, FoliagePusher,
    GpuGrowth, GustFront, ProceduralTree, RenderStyle, TreeAge, TreeFruit, TreeNotch, TreeParams, TreePreset, TreeShake,
    TreeShakes, build_tree,
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
/// The fell clip's felling: the tree in `FELL_GROVE`, the bearing it falls toward in degrees from
/// +x toward +z, the frames of the axe's blows, each cutting its notch (`TreeNotch`) on that side
/// [`NOTCH_STEP`] deeper and shivering its crown, and the frame the saw goes in (`FellTree`). The
/// camera looks from +x, so the notch shows in profile and the oak falls to the left across open
/// ground, square to the view.
const FELL: (usize, f32, [u32; 4], u32) = (0, 90.0, [6, 14, 22, 30], 40);
/// How much deeper each blow of the fell clip's axe cuts the notch, a share of the trunk's width.
const NOTCH_STEP: f32 = 0.08;
/// The chop clip: a row of trees across the view (species, seed, x in metres), and its lumberjack:
/// where it starts (x, z), its pace (m/s), how far from the bark it stands to swing (metres), its
/// axe's blows on each tree and the frames between them, and the frames it takes to walk half way
/// round a trunk.
const CHOP_ROW: [(TreePreset, i32, f32); 4] =
    [(TreePreset::Birch, 3, -6.75), (TreePreset::Maple, 5, -2.25), (TreePreset::Oak, 42, 2.25), (TreePreset::Pine, 1, 6.75)];
const CHOP_FROM: Vec2 = Vec2::new(-10.0, 1.2);
const CHOP_WALK: f32 = 3.0;
const CHOP_REACH: f32 = 0.6;
const CHOP_BLOWS: u32 = 3;
const CHOP_BLOW_FRAMES: u32 = 6;
const CHOP_ROUND_FRAMES: u32 = 14;
/// The harvest clip: two apple trees across the view (seed, x in metres); its shake (frame, the
/// tree, strength); and its picker: the tree it picks from, where it stands (x, z, beyond that
/// tree), its hand's height and reach (metres), the frame it starts, the frames it reaches for
/// each apple before picking it and the frames a picked apple flies to its basket (x, z).
const HARVEST_ROW: [(i32, f32); 2] = [(42, -2.9), (11, 2.9)];
const HARVEST_SHAKES: [(u32, usize, f32); 1] = [(18, 0, 1.6)];
const PICKER_TREE: usize = 1;
const PICKER_AT: Vec2 = Vec2::new(5.5, 0.4);
const PICKER_HAND: f32 = 1.7;
const PICKER_REACH: f32 = 2.6;
const PICKER_FROM: u32 = 66;
const PICKER_FRAMES: u32 = 3;
const BASKET_AT: Vec2 = Vec2::new(4.9, 1.2);
const BASKET_FRAMES: u32 = 6;
/// The push clip's plants: shrubs (seed and place: x, z, metres from the patch's middle), a row
/// along the walker's path and a few behind it, none in front to hide it.
const PUSH_PATCH: [(i32, [f32; 2]); 10] = [
    (1, [-3.0, 0.25]),
    (2, [-1.8, -0.2]),
    (3, [-0.6, 0.3]),
    (4, [0.6, -0.25]),
    (5, [1.8, 0.2]),
    (6, [3.0, -0.15]),
    (7, [-2.4, -1.7]),
    (8, [-0.1, -1.9]),
    (9, [2.3, -1.6]),
    (10, [4.2, -1.2]),
];
/// The push clip's walker: along x from `WALK_X.0` to `WALK_X.1` at z `WALK_Z` (metres from the
/// patch's middle), from frame `WALK_FROM` to `WALK_TO` (some 1.4 m/s), then still.
const WALK_X: (f32, f32) = (-4.0, 4.0);
const WALK_Z: f32 = 0.0;
const WALK_FROM: u32 = 12;
const WALK_TO: u32 = 150;
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
/// The fire clip: the frame its fire is lit, how far upwind of the grove's middle (metres), the
/// square it covers (metres, centred on the grove) and its cells along a side (half a metre).
const FIRE_FRAME: u32 = 12;
const FIRE_UPWIND: f32 = 7.0;
const FIRE_SIZE: f32 = 48.0;
const FIRE_CELLS: u32 = 96;
/// The fire clip's spread, sped up like the rest of a clip: across the grove in a few seconds,
/// each spot burnt through in two and a half, its embers cool in five. The crowns reach 4.5 m, so
/// the fire crosses every gap in the grove.
const FIRE_SPREAD: FireSpread = FireSpread { speed: 3.5, burn_time: 2.5, cool_time: 5.0, reach: 4.5, ground: 0.0, wind: 1.0 };

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
    /// The grove, fully grown, at dusk in a wind: a fire lit on its upwind edge runs through it,
    /// scorching the leaves, burning them away and charring the wood, and the embers cool.
    Fire,
    /// A patch of shrubs in a light wind and a body walking through it (`FoliagePusher`): the
    /// leaves and stems give way and close up behind it.
    Push,
    /// The grove, fully grown, in a light wind, and a lumberjack who asks [`FloraIndex`] for the
    /// nearest tree, walks to it, notches it, walks round it and fells it, then the next.
    Chop,
    /// A row of apple trees in a light wind: two are shaken and drop apples, then a picker takes
    /// apples off the third ([`TreeFruit`]).
    Harvest,
}

/// The push clip's walker.
#[derive(Component)]
struct Walker;

/// A picked apple of the harvest clip on its way to the basket.
#[derive(Component)]
struct Flying {
    from: Vec3,
    to: Vec3,
    start: u32,
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
    /// How many of the fell clip's blows (`FELL`), then its saw, have landed.
    felled: usize,
    /// The fire clip's patch of ground that scorches.
    scorch: Handle<Mesh>,
    /// Where the fire clip's fire is lit.
    fire_start: Vec3,
    /// What the chop clip's lumberjack is doing.
    chop: Chop,
    /// How many of `HARVEST_SHAKES` have been sent.
    harvest_shaken: usize,
    /// The apple the harvest clip's picker reaches for: its tree, its key, where it hangs and the
    /// frame it reached.
    reach: Option<(Entity, u32, Vec3, u32)>,
    /// The harvest clip's picked apple: its mesh and material.
    apple: (Handle<Mesh>, Handle<StandardMaterial>),
    /// How many apples are in the harvest clip's basket.
    basketed: u32,
}

#[derive(Component)]
struct OrbitCam;

fn usage() -> ! {
    eprintln!(
        "usage: capture_growth [--out <dir>] [--preset <name> [--style <natural|painterly|toon>] | --grove | --seasons | --shake | --fell | --fire | --push | --chop | --harvest] [--gpu]"
    );
    std::process::exit(2);
}

fn main() {
    let mut out = None;
    let mut preset = None;
    // The grove clips: "grove", "seasons", "shake", "fell", "fire" or "push"; none is the single
    // growing tree.
    let mut clip: Option<&'static str> = None;
    let mut gpu = false;
    let mut style = RenderStyle::Natural;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--gpu" => gpu = true,
            "--grove" | "--seasons" | "--shake" | "--fell" | "--fire" | "--push" | "--chop" | "--harvest" => {
                if clip.is_some() {
                    usage();
                }
                clip = Some(match arg.as_str() {
                    "--grove" => "grove",
                    "--seasons" => "seasons",
                    "--shake" => "shake",
                    "--fell" => "fell",
                    "--fire" => "fire",
                    "--push" => "push",
                    "--chop" => "chop",
                    _ => "harvest",
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
        Some("fell") => Scene::Fell,
        Some("fire") => Scene::Fire,
        Some("push") => Scene::Push,
        Some("chop") => Scene::Chop,
        Some(_) => Scene::Harvest,
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

    let mut app = App::new();
    app.add_plugins((
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
    // A light wind in the shake, fell and push clips, so the knocks, the falls and the walker are
    // what moves.
    .insert_resource(FloraWind {
        strength: if matches!(scene, Scene::Shake | Scene::Fell | Scene::Push | Scene::Chop | Scene::Harvest) { 0.15 } else { 0.5 },
        direction: wind,
        speed: 1.4,
        ..default()
    })
    // The seasons clip is a time-lapse, a year in eight seconds: leaves falling at their own
    // speed would still hang in the air in winter, so the season's leaves are shed where they
    // hang, as before falling leaves (`FallingLeaves::autumn_distance` 0).
    .insert_resource(FallingLeaves {
        autumn_distance: if matches!(scene, Scene::Seasons) { 0.0 } else { FallingLeaves::default().autumn_distance },
        ..default()
    })
    // Time stands still until `drive` steps it, once per captured frame.
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    if matches!(scene, Scene::Fire) {
        // Dusk: a darker sky and a cool ambient light. The fire, sped up like the rest of a clip.
        app.insert_resource(ClearColor(Color::srgb(0.36, 0.41, 0.62)))
            .insert_resource(GlobalAmbientLight { color: Color::srgb(0.5, 0.55, 0.75), brightness: 200.0, ..default() })
            .insert_resource(FloraFire::with_cells(focus.xz(), FIRE_SIZE, FIRE_CELLS).with_spread(FIRE_SPREAD));
    }
    app.insert_resource(Capture {
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
        chop: Chop::Seek(None),
        harvest_shaken: 0,
        reach: None,
        apple: (Handle::default(), Handle::default()),
        basketed: 0,
        scorch: Handle::default(),
        fire_start: Vec3::ZERO,
    })
    .insert_resource(UseGpu(gpu))
    .add_systems(Startup, setup)
    .add_systems(Update, (drive, chop, harvest).chain())
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
        Scene::Grove | Scene::Seasons | Scene::Shake | Scene::Fire => {
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
            // Where the felled tree will lie.
            let (tree, bearing, ..) = FELL;
            let (params, at, ..) = &placed[tree];
            let (sin, cos) = bearing.to_radians().sin_cos();
            let (lo, hi) = bounds(params, Vec3::ZERO);
            let crown = 0.5 * (hi.x - lo.x).max(hi.z - lo.z);
            let lying = *at + Vec3::new(cos, 0.0, sin) * (hi.y + 0.5 * crown);
            (min, max) = (min.min(lying), max.max(lying));
            let half_width = 0.55 * (max.z - min.z);
            let focus = Vec3::new(0.5 * (min.x + max.x), 0.42 * max.y, 0.5 * (min.z + max.z));
            let half_height = (max.y - focus.y).max(focus.y);
            let aspect = size.x as f32 / size.y as f32;
            let half_h_fov = ((FOV * 0.5).tan() * aspect).atan();
            let fit = (half_width / half_h_fov.tan()).max(half_height / (FOV * 0.5).tan());
            (fit * 0.9 + 0.25 * (max.x - min.x), focus)
        }
        // Seen from +z: the patch, as wide as it is, and the walker's path through it.
        Scene::Push => {
            let (min, max) = push_patch()
                .iter()
                .map(|(params, at, ..)| bounds(params, *at))
                .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), (a, b)| (lo.min(a), hi.max(b)));
            let half_width = 0.5 * (max.x - min.x).max(WALK_X.1 - WALK_X.0);
            let focus = Vec3::new(0.5 * (min.x + max.x), 0.35 * max.y, 0.5 * (min.z + max.z));
            let half_height = (max.y - focus.y).max(focus.y);
            let aspect = size.x as f32 / size.y as f32;
            let half_h_fov = ((FOV * 0.5).tan() * aspect).atan();
            let fit = (half_width / half_h_fov.tan()).max(half_height / (FOV * 0.5).tan());
            (fit * 0.7 + 0.5 * (max.z - min.z), focus)
        }
        // Seen from +z: the row, the lumberjack's start and the first tree lying to its left.
        Scene::Chop => {
            let row = chop_row();
            let (mut min, max) = row
                .iter()
                .map(|(params, at, ..)| bounds(params, *at))
                .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), (a, b)| (lo.min(a), hi.max(b)));
            min.x = min.x.min(row[0].1.x - 0.8 * (max.y - min.y)).min(CHOP_FROM.x);
            let half_width = 0.5 * (max.x - min.x);
            let focus = Vec3::new(0.5 * (min.x + max.x), 0.4 * max.y, 0.0);
            let half_height = (max.y - focus.y).max(focus.y);
            let aspect = size.x as f32 / size.y as f32;
            let half_h_fov = ((FOV * 0.5).tan() * aspect).atan();
            let fit = (half_width / half_h_fov.tan()).max(half_height / (FOV * 0.5).tan());
            (fit * 0.95, focus)
        }
        // Seen from +z: the row of apple trees and the picker beside the last.
        Scene::Harvest => {
            let (min, mut max) = harvest_row()
                .iter()
                .map(|(params, at, ..)| bounds(params, *at))
                .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), (a, b)| (lo.min(a), hi.max(b)));
            max.x = max.x.max(PICKER_AT.x + 1.3);
            let half_width = 0.5 * (max.x - min.x);
            let focus = Vec3::new(0.5 * (min.x + max.x), 0.45 * max.y, 0.0);
            let half_height = (max.y - focus.y).max(focus.y);
            let aspect = size.x as f32 / size.y as f32;
            let half_h_fov = ((FOV * 0.5).tan() * aspect).atan();
            let fit = (half_width / half_h_fov.tan()).max(half_height / (FOV * 0.5).tan());
            // The crowns' near halves, the picker and its basket stand nearer the camera than the
            // row: a wide margin.
            (fit * 1.35, focus)
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

/// The push clip's plants, mature, at their places in metres.
fn push_patch() -> Vec<(TreeParams, Vec3, u32, u32)> {
    PUSH_PATCH
        .iter()
        .map(|&(seed, [x, z])| (TreeParams { seed, ..TreePreset::Shrub.params() }, Vec3::new(x, 0.0, z), 0, 1))
        .collect()
}

/// The chop clip's row of trees, mature, at their places in metres.
fn chop_row() -> Vec<(TreeParams, Vec3, u32, u32)> {
    CHOP_ROW.iter().map(|&(preset, seed, x)| (TreeParams { seed, ..preset.params() }, Vec3::new(x, 0.0, 0.0), 0, 1)).collect()
}

/// The harvest clip's apple trees, mature, at their places in metres.
fn harvest_row() -> Vec<(TreeParams, Vec3, u32, u32)> {
    HARVEST_ROW.iter().map(|&(seed, x)| (TreeParams { seed, ..TreePreset::Apple.params() }, Vec3::new(x, 0.0, 0.0), 0, 1)).collect()
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
    // The fire burns at dusk: a low, warm sun.
    let (illuminance, colour, sun) = if matches!(capture.scene, Scene::Fire) {
        (4_000.0, Color::srgb(1.0, 0.74, 0.52), Vec3::new(18.0, 7.0, 9.0))
    } else {
        (10_000.0, Color::WHITE, Vec3::new(10.0, 20.0, 10.0))
    };
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance,
            color: colour,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            first_cascade_far_bound: capture.distance * 0.5,
            maximum_distance: capture.distance * 2.5,
            ..default()
        }
        .build(),
        Transform::from_translation(sun).looking_at(Vec3::ZERO, Vec3::Y),
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
        Scene::Seasons | Scene::Shake | Scene::Fire => {
            (1.0, grove(Vec3::ZERO).into_iter().map(|(p, at, _, frames)| (p, at, 0, frames)).collect())
        }
        Scene::Chop => (1.0, chop_row()),
        Scene::Harvest => (1.0, harvest_row()),
        Scene::Fell => (1.0, fell_grove()),
        Scene::Push => (1.0, push_patch()),
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
    // The fire clip's patch of ground that scorches (its vertex colours follow the fire), a few
    // millimetres over the grass.
    if matches!(capture.scene, Scene::Fire) {
        let mut patch = Plane3d::default().mesh().size(FIRE_SIZE, FIRE_SIZE).subdivisions(95).build();
        let colours = vec![ground_colour(0.0).to_linear().to_f32_array(); patch.count_vertices()];
        patch.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
        capture.scorch = meshes.add(patch);
        commands.spawn((
            Mesh3d(capture.scorch.clone()),
            MeshMaterial3d(materials.add(StandardMaterial { perceptual_roughness: 1.0, ..default() })),
            Transform::from_translation(capture.focus.with_y(0.004)),
        ));
        // Upwind of the grove's middle, on the edge of its crowns.
        capture.fire_start = (capture.focus.xz() - capture.wind * FIRE_UPWIND).extend(0.0).xzy();
    }
    // The push clip's walker: a red stand-in for a character, 1.8 m tall, pushing the foliage aside.
    if matches!(capture.scene, Scene::Push) {
        commands.spawn((
            Mesh3d(meshes.add(Capsule3d::new(0.36, 1.08))),
            MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.75, 0.15, 0.12), ..default() })),
            Transform::from_translation(capture.focus.with_y(0.9) + Vec3::new(WALK_X.0, 0.0, WALK_Z)),
            FoliagePusher::default(),
            Walker,
        ));
    }
    // The chop clip's lumberjack: the same stand-in, to the left of the row and a little in front.
    if matches!(capture.scene, Scene::Chop) {
        commands.spawn((
            Mesh3d(meshes.add(Capsule3d::new(0.3, 1.2))),
            MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.75, 0.15, 0.12), ..default() })),
            Transform::from_translation(CHOP_FROM.extend(0.9).xzy()),
            Walker,
        ));
    }
    // The harvest clip's picker: the same stand-in, beyond the right tree, facing it, with a
    // basket at its feet, and the look of the apples it picks.
    if matches!(capture.scene, Scene::Harvest) {
        commands.spawn((
            Mesh3d(meshes.add(Capsule3d::new(0.3, 1.2))),
            MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.75, 0.15, 0.12), ..default() })),
            Transform::from_translation(PICKER_AT.extend(0.9).xzy()),
            Walker,
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.26, 0.3))),
            MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.45, 0.30, 0.15), ..default() })),
            Transform::from_translation(BASKET_AT.extend(0.15).xzy()),
        ));
        let red = StandardMaterial { base_color: Color::srgb(0.72, 0.08, 0.06), perceptual_roughness: 0.35, ..default() };
        capture.apple = (meshes.add(Sphere::new(0.05)), materials.add(red));
    }
    let mut camera = commands.spawn((
        Camera3d::default(),
        RenderTarget::Image(capture.target.clone().into()),
        camera_at(&capture, 0),
        OrbitCam,
    ));
    // At dusk the embers and the burning leaves glow.
    if matches!(capture.scene, Scene::Fire) {
        camera.insert(Bloom::NATURAL);
    }
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
        Scene::Shake | Scene::Fire => orbit(capture, 0.5),
        // From +z, square to the row: the lumberjack works from left to right and each tree
        // falls to the left, across the view.
        Scene::Chop | Scene::Harvest => {
            let pitch = 10f32.to_radians();
            let dir = Vec3::new(0.0, pitch.sin(), pitch.cos());
            Transform::from_translation(capture.focus + dir * capture.distance).looking_at(capture.focus, Vec3::Y)
        }
        // From +z, a little higher than the others, so the parting shows: the walker crosses the
        // view from left to right.
        Scene::Push => {
            let pitch = 22f32.to_radians();
            let dir = Vec3::new(0.0, pitch.sin(), pitch.cos());
            Transform::from_translation(capture.focus + dir * capture.distance).looking_at(capture.focus, Vec3::Y)
        }
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
    fire: Option<ResMut<FloraFire>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut walkers: Query<&mut Transform, (With<Walker>, Without<OrbitCam>)>,
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
    // The fell clip's axe, each blow once at its frame, then the saw.
    if matches!(capture.scene, Scene::Fell) {
        let (tree, bearing, blows, saw) = FELL;
        let (sin, cos) = bearing.to_radians().sin_cos();
        let direction = Vec2::new(cos, sin);
        while capture.felled < blows.len() && frame >= blows[capture.felled] {
            capture.felled += 1;
            if let Some(entity) = capture.entities[tree] {
                let depth = NOTCH_STEP * capture.felled as f32;
                commands.entity(entity).insert(TreeNotch { direction, depth, ..default() });
            }
            // The blow lands on the notch's side and shivers the crown away from it.
            let knock = TreeShake { at: capture.trees[tree].xz(), radius: 1.0, strength: 1.5, direction: -direction, duration: 1.5, ..default() };
            shakes.shake(knock, &time);
        }
        if capture.felled == blows.len() && frame >= saw {
            capture.felled += 1;
            if let Some(entity) = capture.entities[tree] {
                commands.entity(entity).insert(FellTree { direction, ..default() });
            }
        }
    }
    // The push clip's walker: across the patch at a steady pace, from its frame to its last.
    if matches!(capture.scene, Scene::Push) {
        let t = (frame.saturating_sub(WALK_FROM) as f32 / (WALK_TO - WALK_FROM) as f32).min(1.0);
        for mut at in &mut walkers {
            at.translation = capture.focus.with_y(0.9) + Vec3::new(WALK_X.0 + (WALK_X.1 - WALK_X.0) * t, 0.0, WALK_Z);
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
    // The fire clip: lit at its frame on the grove's upwind edge; the ground under it scorches.
    if let (Scene::Fire, Some(mut fire)) = (&capture.scene, fire) {
        if frame == FIRE_FRAME {
            fire.ignite(capture.fire_start, 2.0);
        }
        if let Some(mut ground) = meshes.get_mut(&capture.scorch) {
            scorch_ground(&mut ground, &fire, capture.focus.with_y(0.004));
        }
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

/// What the chop clip's lumberjack is doing.
#[derive(Clone, Copy, Debug)]
enum Chop {
    /// Looking for the nearest standing tree that is not `.0` (the one it just felled, which
    /// leaves the index the frame after).
    Seek(Option<Entity>),
    /// Walking to the bark of the tree.
    Walk(Entity),
    /// Cutting the notch on the tree's side toward `side` (world x, z): `blows` landed, the next
    /// in `wait` frames.
    Notch { tree: Entity, side: Vec2, blows: u32, wait: u32 },
    /// Walking round the tree to the far side of its notch, `step` frames of `CHOP_ROUND_FRAMES`.
    Round { tree: Entity, side: Vec2, step: u32 },
}

/// The chop clip's lumberjack, one step a captured frame: ask the index for the nearest tree,
/// walk to it, notch it, walk round it and fell it toward the notch, then the next. A yellow ring
/// marks the tree it is after and a line runs to it.
#[allow(clippy::too_many_arguments)]
fn chop(
    mut commands: Commands,
    mut capture: ResMut<Capture>,
    index: Res<FloraIndex>,
    mut shakes: ResMut<TreeShakes>,
    time: Res<Time>,
    mut walker: Query<&mut Transform, With<Walker>>,
    mut gizmos: Gizmos,
    mut stepped: Local<Option<u32>>,
) {
    if !matches!(capture.scene, Scene::Chop) {
        return;
    }
    let Ok(mut walker) = walker.single_mut() else { return };
    let at = walker.translation.xz();
    let target = match capture.chop {
        Chop::Seek(_) => None,
        Chop::Walk(tree) | Chop::Notch { tree, .. } | Chop::Round { tree, .. } => index.get(tree).copied(),
    };
    if let Some(tree) = target {
        let ring = Isometry3d::new(tree.base + Vec3::Y * 0.05, Quat::from_rotation_x(FRAC_PI_2));
        gizmos.circle(ring, tree.base_radius + 0.2, Color::srgb(1.0, 0.85, 0.1));
        gizmos.line(walker.translation.with_y(0.05), (tree.base + Vec3::Y * 0.05).move_towards(walker.translation.with_y(0.05), tree.base_radius + 0.2), Color::srgb(1.0, 0.85, 0.1));
    }
    // One step a captured frame.
    if *stepped == Some(capture.frame) {
        return;
    }
    *stepped = Some(capture.frame);
    capture.chop = match capture.chop {
        Chop::Seek(felled) => {
            let nearest = index.within(at, 40.0).filter(|t| Some(t.entity) != felled).min_by(|a, b| a.distance_to(at).total_cmp(&b.distance_to(at)));
            nearest.map_or(Chop::Seek(felled), |tree| Chop::Walk(tree.entity))
        }
        Chop::Walk(entity) => match target {
            None => Chop::Seek(None),
            Some(tree) => {
                // To arm's length from its bark, on the near side.
                let side = (at - tree.base.xz()).normalize_or(Vec2::X);
                let stand = tree.base.xz() + side * (tree.base_radius + CHOP_REACH);
                let step = CHOP_WALK / FPS;
                if at.distance(stand) <= step {
                    walker.translation = stand.extend(0.9).xzy();
                    Chop::Notch { tree: entity, side, blows: 0, wait: 0 }
                } else {
                    walker.translation = at.move_towards(stand, step).extend(0.9).xzy();
                    Chop::Walk(entity)
                }
            }
        },
        Chop::Notch { tree, side, blows, wait } if wait > 0 => Chop::Notch { tree, side, blows, wait: wait - 1 },
        Chop::Notch { tree, side, blows, .. } => {
            // A blow: the notch deepens and the crown shivers away from the axe.
            let blows = blows + 1;
            commands.entity(tree).insert(TreeNotch { direction: side, depth: NOTCH_STEP * blows as f32, ..default() });
            if let Some(trunk) = target {
                let knock = TreeShake { at: trunk.base.xz(), radius: 1.0, strength: 1.5, direction: -side, duration: 1.5, ..default() };
                shakes.shake(knock, &time);
            }
            if blows == CHOP_BLOWS { Chop::Round { tree, side, step: 0 } } else { Chop::Notch { tree, side, blows, wait: CHOP_BLOW_FRAMES } }
        }
        Chop::Round { tree, side, step } => {
            // Half way round the trunk, then the saw: it falls toward the notch, away from the
            // lumberjack.
            let step = step + 1;
            if let Some(trunk) = target {
                let turn = PI * step as f32 / CHOP_ROUND_FRAMES as f32;
                let around = Vec2::from_angle(turn).rotate(side);
                walker.translation = (trunk.base.xz() + around * (trunk.base_radius + CHOP_REACH)).extend(0.9).xzy();
            }
            if step < CHOP_ROUND_FRAMES {
                Chop::Round { tree, side, step }
            } else {
                commands.entity(tree).insert(FellTree { direction: side, ..default() });
                Chop::Seek(Some(tree))
            }
        }
    };
}

/// The harvest clip, one step a captured frame: the shake at its frame; then the picker reaches
/// for the apple on its tree nearest its hand (a yellow line marks it), picks it a few frames later
/// (`TreeFruit::pick`) and drops it into its basket, and reaches for the next.
#[allow(clippy::too_many_arguments)]
fn harvest(
    mut commands: Commands,
    mut capture: ResMut<Capture>,
    mut shakes: ResMut<TreeShakes>,
    time: Res<Time>,
    season: Res<FloraSeason>,
    mut trees: Query<(&GlobalTransform, &mut TreeFruit)>,
    mut flying: Query<(Entity, &Flying, &mut Transform)>,
    mut gizmos: Gizmos,
    mut stepped: Local<Option<u32>>,
) {
    if !matches!(capture.scene, Scene::Harvest) {
        return;
    }
    // The picker's hand, at its shoulder on the tree's side.
    let hand = Vec3::new(PICKER_AT.x - 0.3, PICKER_HAND, PICKER_AT.y);
    if let Some((_, _, apple, _)) = capture.reach {
        gizmos.line(hand, apple, Color::srgb(1.0, 0.85, 0.1));
    }
    if *stepped == Some(capture.frame) {
        return;
    }
    *stepped = Some(capture.frame);
    let frame = capture.frame;
    // Across the view, so the crown swings and the apples fly sideways.
    while let Some(&(at_frame, tree, strength)) = HARVEST_SHAKES.get(capture.harvest_shaken) {
        if frame < at_frame {
            break;
        }
        capture.harvest_shaken += 1;
        let at = capture.trees[tree].xz();
        shakes.shake(TreeShake { at, radius: 1.5, strength, direction: Vec2::X, duration: 2.5, ..default() }, &time);
    }
    // Picked apples arc into the basket and stay there.
    for (entity, apple, mut transform) in &mut flying {
        let t = ((frame - apple.start) as f32 / BASKET_FRAMES as f32).min(1.0);
        transform.translation = apple.from.lerp(apple.to, t) + Vec3::Y * (2.4 * t * (1.0 - t));
        if t >= 1.0 {
            commands.entity(entity).remove::<Flying>();
        }
    }
    if frame < PICKER_FROM {
        return;
    }
    let Some(tree) = capture.entities[PICKER_TREE] else { return };
    let Ok((transform, mut fruit)) = trees.get_mut(tree) else { return };
    capture.reach = match capture.reach {
        Some((_, key, from, since)) if frame >= since + PICKER_FRAMES => {
            if fruit.pick(key) {
                // A loose heap in the basket.
                let n = capture.basketed;
                capture.basketed += 1;
                let spot = Vec2::from_angle(n as f32 * 2.4) * 0.17 * ((n % 7) as f32 / 7.0).sqrt();
                let to = Vec3::new(BASKET_AT.x + spot.x, 0.33 + 0.03 * (n / 7) as f32, BASKET_AT.y + spot.y);
                let (mesh, material) = capture.apple.clone();
                commands.spawn((Mesh3d(mesh), MeshMaterial3d(material), Transform::from_translation(from), Flying { from, to, start: frame }));
            }
            None
        }
        Some(reach) => Some(reach),
        None => fruit.nearest(&season, transform, hand, PICKER_REACH).map(|(apple, at)| (tree, apple.key, at, frame)),
    };
}

/// The fire clip's scorched ground: the patch's vertex colours from grass to ash as the fire's char
/// under each vertex (`FloraFire::char_at`), the patch standing at `origin`.
fn scorch_ground(mesh: &mut Mesh, fire: &FloraFire, origin: Vec3) {
    let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { return };
    let grass = ground_colour(0.0).to_linear().to_vec4();
    let ash = Vec4::new(0.012, 0.010, 0.008, 1.0);
    let colours: Vec<[f32; 4]> = positions
        .iter()
        .map(|&p| grass.lerp(ash, fire.char_at(origin + Vec3::from(p)).powf(0.6)).to_array())
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
}
