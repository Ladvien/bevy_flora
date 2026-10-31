use std::collections::HashMap;

use bevy_flora::foliage::leaf_points_for_node;
use bevy_flora::params::FoliagePlacement;
use bevy_flora::{NodeKind, Skeleton, TreePreset, generate_skeleton};

// leaf (keyed by its tint draw, one per slot) -> place along the branch, in metres from the start
fn along(sk: &Skeleton, i: usize, params: &bevy_flora::params::TreeParams) -> HashMap<u32, f32> {
    let node = &sk.nodes[i];
    let mut pts = Vec::new();
    leaf_points_for_node(sk, i, &params.foliage, &params.growth, &params.trunk, params.seed, &mut pts);
    pts.iter().map(|p| (p.tint.to_bits(), (p.position - node.start).dot(node.direction) / node.length.max(1e-6))).collect()
}

fn pct(v: &mut Vec<f32>, q: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f32 * q) as usize]
}

#[test]
fn zz_probe() {
    for preset in TreePreset::ALL {
        let params = preset.params();
        if params.foliage.placement != FoliagePlacement::AllBranches {
            continue;
        }
        let full = generate_skeleton(&params);
        let mut slide = Vec::new();
        let mut mism = Vec::new();
        let mut unmatched = 0usize;
        let mut prev: Option<Skeleton> = None;
        for step in 0..=200 {
            let age = step as f32 * 0.1;
            let aged = full.aged(age, &params.growth);
            for (i, n) in aged.nodes.iter().enumerate() {
                if n.kind == NodeKind::Trunk || n.progress <= 0.0 || n.progress >= 1.0 {
                    continue;
                }
                let now = along(&aged, i, &params);
                if now.is_empty() {
                    continue;
                }
                let grown = along(&full, n.id as usize, &params);
                for (k, a) in &now {
                    match grown.get(k) {
                        Some(b) => { let d=(a - b).abs() * n.length; if d > 0.01 { println!("MISMATCH {} node {} kind {:?} age {age} progress {} young {} grown {} d {d}", preset.name(), n.id, n.kind, n.progress, now.len(), grown.len()); } mism.push(d) }
                        None => unmatched += 1,
                    }
                }
                if let Some(p) = &prev {
                    if let Some(j) = p.nodes.iter().position(|m| m.id == n.id) {
                        let before = along(p, j, &params);
                        for (k, a) in &now {
                            if let Some(b) = before.get(k) {
                                slide.push((a - b).abs() * n.length);
                            }
                        }
                    }
                }
            }
            prev = Some(aged);
        }
        let mut s2 = slide.clone();
        let mut m2 = mism.clone();
        println!(
            "{:14} slide/step n={:6} mean {:.4} p95 {:.4} max {:.4} m | vs mature n={} mean {:.5} p95 {:.5} max {:.5} m | unmatched {}",
            preset.name(),
            slide.len(),
            slide.iter().sum::<f32>() / slide.len().max(1) as f32,
            pct(&mut s2, 0.95),
            pct(&mut s2, 1.0),
            mism.len(),
            mism.iter().sum::<f32>() / mism.len().max(1) as f32,
            pct(&mut m2, 0.95),
            pct(&mut m2, 1.0),
            unmatched
        );
    }
}
