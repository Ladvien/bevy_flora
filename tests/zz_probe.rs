use bevy_flora::foliage::leaf_points_for_node;
use bevy_flora::params::FoliagePlacement;
use bevy_flora::{NodeKind, Skeleton, TreePreset, generate_skeleton};

fn along(sk: &Skeleton, i: usize, params: &bevy_flora::params::TreeParams) -> Vec<f32> {
    let node = &sk.nodes[i];
    let mut pts = Vec::new();
    leaf_points_for_node(sk, i, &params.foliage, &params.growth, &params.trunk, params.seed, &mut pts);
    pts.iter().map(|p| (p.position - node.start).dot(node.direction) / node.length.max(1e-6)).collect()
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
        let mut mism_n = 0usize;
        let mut count_diff = 0usize;
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
                // parity with the mature (GPU) places
                let grown = along(&full, n.id as usize, &params);
                for (k, a) in now.iter().enumerate() {
                    if let Some(b) = grown.get(k) {
                        mism.push((a - b).abs() * n.length);
                        mism_n += 1;
                    }
                }
                if now.len() > grown.len() {
                    count_diff += 1;
                }
                if let Some(p) = &prev {
                    if let Some(j) = p.nodes.iter().position(|m| m.id == n.id) {
                        let before = along(p, j, &params);
                        for (k, a) in now.iter().enumerate() {
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
            "{:14} slide/step n={:6} mean {:.4} p95 {:.4} max {:.4} m | vs mature n={} mean {:.4} p95 {:.4} max {:.4} m | more than mature {}",
            preset.name(),
            slide.len(),
            slide.iter().sum::<f32>() / slide.len().max(1) as f32,
            pct(&mut s2, 0.95),
            pct(&mut s2, 1.0),
            mism_n,
            mism.iter().sum::<f32>() / mism.len().max(1) as f32,
            pct(&mut m2, 0.95),
            pct(&mut m2, 1.0),
            count_diff
        );
    }
}
