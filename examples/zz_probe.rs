//! Throwaway: build time and size of every preset, and the willow as it was.
use bevy_flora::*;
use std::time::Instant;

fn old_willow() -> TreeParams {
    let mut p = TreePreset::Willow.params();
    p.branches.start = 0.5;
    p.branches.angle = 50.0;
    p.branches.angle_curve = -0.4;
    p.branches.strands = StrandParams::default();
    p.foliage.density = 10.0;
    p.foliage.leaf_size = 0.26;
    p
}

fn time(params: &TreeParams, age: f32) -> (f64, usize, usize, usize, usize) {
    let mut best = f64::MAX;
    let mut sizes = (0, 0, 0, 0);
    for _ in 0..5 {
        let t = Instant::now();
        let tree = build_tree(params, age);
        best = best.min(t.elapsed().as_secs_f64() * 1000.0);
        sizes = (tree.trunk.positions.len(), tree.trunk.indices.len() / 3, tree.foliage.positions.len(), tree.foliage.indices.len() / 3);
    }
    (best, sizes.0, sizes.1, sizes.2, sizes.3)
}

fn main() {
    let mut cases: Vec<(String, TreeParams)> = TreePreset::ALL.iter().map(|p| (p.name().to_string(), p.params())).collect();
    cases.push(("willow_old".into(), old_willow()));
    for pipe in [false, true] {
        let mut p = TreePreset::Willow.params();
        p.mesh.pipe_radius_enabled = pipe;
        let tree = build_tree(&p, f32::INFINITY);
        let low = tree.trunk.positions.iter().copied().fold(bevy::math::Vec3::splat(f32::MAX), |a, b| if b.y < a.y { b } else { a });
        println!("pipe {pipe}: lowest wood vertex {low}");
        let mut near: Vec<(f32, usize)> = tree.skeleton.nodes.iter().enumerate().map(|(i, n)| (n.point_at(1.0).distance(low).min(n.start.distance(low)), i)).collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (d, i) in near.iter().take(3) {
            let n = &tree.skeleton.nodes[*i];
            println!("   node {i} {:?} d {d:.3} start {} end {} len {} base_r {} tip_r {}", n.kind, n.start, n.point_at(1.0), n.length, n.base_radius, n.tip_radius);
        }
    }
    for (name, params) in &cases {
        for age in [f32::INFINITY, 4.0] {
            let (ms, tv, tt, fv, ft) = time(params, age);
            println!("{name:18} age {age:>4}: {ms:8.2} ms  trunk {tv:6}v {tt:6}t  foliage {fv:6}v {ft:6}t");
        }
    }
    for (name, params) in [("willow", TreePreset::Willow.params()), ("willow_old", old_willow())] {
        let t = Instant::now();
        let lods = build_lods(&params, f32::INFINITY, &LodSettings::default());
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        for (i, l) in lods.iter().enumerate() {
            println!("{name} LOD{i}: trunk {}v foliage {}v", l.trunk.positions.len(), l.foliage.positions.len());
        }
        println!("{name} build_lods {ms:.1} ms");
    }
}
