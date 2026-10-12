//! Leaf veins by open space colonisation (Runions, Fuhrer, Lane, Prusinkiewicz, Hammel and
//! Mech, "Modeling and visualization of leaf venation patterns", SIGGRAPH 2005, section 4).
//!
//! Auxin sources are scattered over the leaf card by dart throwing, and a vein network grows
//! from the petiole toward them: every vein node that is the closest to some source within reach
//! grows one step toward the mean direction of its sources, and a source is used up once a vein
//! node comes within the kill distance of it. The veins fork where sources lie on both sides, so
//! the pattern is a real midrib, side veins and a fine mesh, not the zigzag a formula draws.
//! Vein widths follow Murray's law: a parent's width cubed is the sum of its children's.
//!
//! Everything is in *card units*: the unit of the procedural leaf's outline, `x`
//! across the leaf (-1..1), `y` from the petiole at -1 to the tip at 1. The veins are
//! rasterised into a small `R8` image that [`FloraTextures`](crate::shading::FloraTextures)
//! uploads and the leaf shader samples with the card's own UVs.

use bevy::math::Vec2;

use crate::foliage::LEAF_OUTLINE;
use crate::rng::SeededRng;

/// Smallest distance between two auxin sources, in card units.
pub const BIRTH_DISTANCE: f32 = 0.06;
/// How far a vein grows toward its sources each iteration.
pub const STEP: f32 = 0.02;
/// A source this close to a vein node is used up.
pub const KILL_DISTANCE: f32 = 0.04;
/// How far a source reaches for a vein node.
pub const ATTRACTION_DISTANCE: f32 = 0.4;
/// Iterations before growth stops whatever is left.
pub const MAX_ITERATIONS: u32 = 400;
/// Width of a vein tip, in card units.
pub const TIP_WIDTH: f32 = 0.008;
/// Width of the raster the veins are drawn into, in pixels.
pub const IMAGE_WIDTH: u32 = 128;
/// Height of the raster, in pixels.
pub const IMAGE_HEIGHT: u32 = 256;

/// Where the petiole joins the blade: the first vein node.
const PETIOLE: Vec2 = Vec2::new(0.0, -1.0);
/// Dart throwing gives up after this many throws in a row that landed too close to a source.
const MAX_MISSES: u32 = 4_000;
/// Bounds of the sources: [`LEAF_OUTLINE`]'s box.
const BOX: (Vec2, Vec2) = (Vec2::new(-0.8, -1.0), Vec2::new(0.8, 1.0));

/// One vein node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VeinNode {
    /// Position, in card units.
    pub position: Vec2,
    /// The node this one grew from, `None` for the petiole. Always an earlier node.
    pub parent: Option<u32>,
    /// Width of the vein segment from the parent to this node, by Murray's law.
    pub width: f32,
}

/// A grown vein network.
#[derive(Clone, Debug, PartialEq)]
pub struct Venation {
    /// Every vein node, parents before children.
    pub nodes: Vec<VeinNode>,
    /// Auxin sources thrown over the leaf.
    pub sources: usize,
    /// Sources a vein reached (used up).
    pub reached: usize,
}

/// Whether `p` lies inside the procedural leaf's outline.
pub fn inside_leaf(p: Vec2) -> bool {
    let mut inside = false;
    let n = LEAF_OUTLINE.len();
    for i in 0..n {
        let (a, b) = (LEAF_OUTLINE[i], LEAF_OUTLINE[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// Throw auxin sources over the leaf: random points inside it, none closer than
/// [`BIRTH_DISTANCE`] to another (dart throwing, Poisson-disc by rejection).
fn throw_sources(rng: &mut SeededRng) -> Vec<Vec2> {
    let mut sources: Vec<Vec2> = Vec::new();
    let mut misses = 0;
    while misses < MAX_MISSES {
        let p = Vec2::new(rng.range(BOX.0.x, BOX.1.x), rng.range(BOX.0.y, BOX.1.y));
        if inside_leaf(p) && sources.iter().all(|s| s.distance_squared(p) >= BIRTH_DISTANCE * BIRTH_DISTANCE) {
            sources.push(p);
            misses = 0;
        } else {
            misses += 1;
        }
    }
    sources
}

/// Grow the veins of a leaf for `seed`: the same seed always grows the same network.
pub fn grow(seed: i32) -> Venation {
    let mut rng = SeededRng::from_parts(seed, 0x7E1A);
    let sources = throw_sources(&mut rng);
    let mut alive = vec![true; sources.len()];
    let mut nodes = vec![VeinNode { position: PETIOLE, parent: None, width: 0.0 }];

    for _ in 0..MAX_ITERATIONS {
        if !alive.iter().any(|&a| a) {
            break;
        }
        // Each live source goes to its closest vein node within reach.
        let mut pull = vec![(Vec2::ZERO, 0u32); nodes.len()];
        let mut any = false;
        for (source, _) in sources.iter().zip(&alive).filter(|(_, alive)| **alive) {
            let closest = nodes
                .iter()
                .enumerate()
                .map(|(i, n)| (i, n.position.distance_squared(*source)))
                .filter(|&(_, d)| d <= ATTRACTION_DISTANCE * ATTRACTION_DISTANCE)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((i, d)) = closest {
                pull[i].0 += (*source - nodes[i].position) / d.sqrt().max(1.0e-6);
                pull[i].1 += 1;
                any = true;
            }
        }
        if !any {
            break;
        }
        // Every node with sources grows one step toward their mean direction.
        let grown = nodes.len();
        for (i, &(direction, count)) in pull.iter().enumerate().take(grown) {
            if count == 0 {
                continue;
            }
            let direction = direction.normalize_or_zero();
            if direction == Vec2::ZERO {
                continue;
            }
            let position = nodes[i].position + direction * STEP;
            nodes.push(VeinNode { position, parent: Some(i as u32), width: 0.0 });
        }
        // Sources within the kill distance of any vein node are used up.
        for (source, alive) in sources.iter().zip(&mut alive) {
            if *alive
                && nodes[grown..]
                    .iter()
                    .any(|n| n.position.distance_squared(*source) <= KILL_DISTANCE * KILL_DISTANCE)
            {
                *alive = false;
            }
        }
    }

    // Murray's law, leaves to root: a parent's width cubed is the sum of its children's.
    let mut cubes = vec![0.0f32; nodes.len()];
    let mut has_child = vec![false; nodes.len()];
    for i in (0..nodes.len()).rev() {
        let width = if has_child[i] { cubes[i].cbrt() } else { TIP_WIDTH };
        nodes[i].width = width;
        if let Some(parent) = nodes[i].parent {
            cubes[parent as usize] += width * width * width;
            has_child[parent as usize] = true;
        }
    }

    let reached = alive.iter().filter(|&&a| !a).count();
    Venation { nodes, sources: sources.len(), reached }
}

/// Draw `veins` into a `width` x `height` image of `R8` texels (row 0 at the petiole, `y` = -1;
/// the last row at the tip): 255 at the centre of a vein and 0 away from them, anti-aliased, a
/// vein is at least a pixel wide so the finest ones stay visible.
pub fn rasterize(veins: &Venation, width: u32, height: u32) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let pixel = Vec2::new(2.0 / width as f32, 2.0 / height as f32);
    let half_pixel = 0.5 * 0.5 * (pixel.x + pixel.y);
    let mut image = vec![0u8; w * h];
    for node in &veins.nodes {
        let Some(parent) = node.parent else { continue };
        let (a, b) = (veins.nodes[parent as usize].position, node.position);
        let radius = (0.5 * node.width).max(half_pixel);
        let reach = radius + 2.0 * half_pixel;
        let to_pixel = |p: Vec2| Vec2::new((p.x + 1.0) / pixel.x, (p.y + 1.0) / pixel.y);
        let (lo, hi) = (to_pixel(a.min(b) - Vec2::splat(reach)), to_pixel(a.max(b) + Vec2::splat(reach)));
        let (x0, x1) = (lo.x.floor().max(0.0) as usize, (hi.x.ceil() as usize).min(w - 1));
        let (y0, y1) = (lo.y.floor().max(0.0) as usize, (hi.y.ceil() as usize).min(h - 1));
        let ab = b - a;
        let length_squared = ab.length_squared().max(1.0e-12);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let p = Vec2::new(-1.0 + (x as f32 + 0.5) * pixel.x, -1.0 + (y as f32 + 0.5) * pixel.y);
                let t = ((p - a).dot(ab) / length_squared).clamp(0.0, 1.0);
                let distance = p.distance(a + ab * t);
                let coverage = ((radius + half_pixel - distance) / (2.0 * half_pixel)).clamp(0.0, 1.0);
                let texel = &mut image[y * w + x];
                *texel = (*texel).max((coverage * 255.0).round() as u8);
            }
        }
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_grows_the_same_veins_and_another_grows_others() {
        let (a, b) = (grow(5), grow(5));
        assert_eq!(a, b);
        assert_ne!(a.nodes, grow(6).nodes);
        assert_eq!(rasterize(&a, 32, 64), rasterize(&b, 32, 64));
    }

    #[test]
    fn veins_reach_nearly_every_source_and_stay_in_the_leaf() {
        for seed in [0, 1, 7, 42, -3] {
            let veins = grow(seed);
            assert!(veins.sources > 200, "seed {seed}: only {} sources", veins.sources);
            assert!(
                veins.reached * 10 >= veins.sources * 9,
                "seed {seed}: reached {} of {} sources",
                veins.reached,
                veins.sources
            );
            for (i, node) in veins.nodes.iter().enumerate() {
                if let Some(parent) = node.parent {
                    assert!((parent as usize) < i, "seed {seed}: node {i} grew from a later node");
                    // Growth is toward sources inside the leaf, one step at a time, so a vein
                    // node can overshoot the outline by at most a step.
                    let inside = inside_leaf(node.position)
                        || (0..8).any(|k| {
                            let angle = k as f32 * std::f32::consts::FRAC_PI_4;
                            inside_leaf(node.position - Vec2::from_angle(angle) * STEP)
                        });
                    assert!(inside, "seed {seed}: node {i} at {:?} is outside the leaf", node.position);
                }
            }
        }
    }

    #[test]
    fn widths_follow_murrays_law() {
        let veins = grow(3);
        let mut cubes = vec![0.0f32; veins.nodes.len()];
        for node in &veins.nodes {
            if let Some(parent) = node.parent {
                cubes[parent as usize] += node.width.powi(3);
            }
        }
        let mut forks = 0;
        for (node, cube) in veins.nodes.iter().zip(&cubes) {
            if *cube > 0.0 {
                assert!((node.width.powi(3) - cube).abs() <= 1.0e-6 + 1.0e-4 * cube, "{node:?}: {cube}");
                forks += 1;
            } else {
                assert_eq!(node.width, TIP_WIDTH);
            }
        }
        assert!(forks > 10, "only {forks} nodes carry veins");
        // The midrib at the petiole is the widest vein.
        assert!(veins.nodes[0].width >= veins.nodes.iter().map(|n| n.width).fold(0.0, f32::max) - 1e-6);
        assert!(veins.nodes[0].width > 4.0 * TIP_WIDTH);
    }

    #[test]
    fn every_drawn_vein_pixel_lies_inside_the_leaf() {
        for seed in [1, 2, 3] {
            let veins = grow(seed);
            let image = rasterize(&veins, IMAGE_WIDTH, IMAGE_HEIGHT);
            let (w, h) = (IMAGE_WIDTH as usize, IMAGE_HEIGHT as usize);
            let mut drawn = 0;
            for y in 0..h {
                for x in 0..w {
                    if image[y * w + x] < 128 {
                        continue;
                    }
                    drawn += 1;
                    // The pixel's centre, widened by a vein-step's reach so a vein's rounded end
                    // at the very edge counts.
                    let p = Vec2::new(-1.0 + (x as f32 + 0.5) * 2.0 / w as f32, -1.0 + (y as f32 + 0.5) * 2.0 / h as f32);
                    let near_inside = inside_leaf(p)
                        || (0..8).any(|k| inside_leaf(p + Vec2::from_angle(k as f32 * std::f32::consts::FRAC_PI_4) * 0.04));
                    assert!(near_inside, "seed {seed}: vein pixel ({x}, {y}) at {p:?} is outside the leaf");
                }
            }
            assert!(drawn > 500, "seed {seed}: only {drawn} vein pixels");
            // The midrib runs up the middle of the leaf (it may wander a few pixels).
            let column: Vec<u8> =
                (0..h).map(|y| (w / 2 - 6..w / 2 + 6).map(|x| image[y * w + x]).max().unwrap_or(0)).collect();
            assert!(column.iter().filter(|&&v| v >= 128).count() > h / 2, "seed {seed}: no midrib");
        }
    }
}
