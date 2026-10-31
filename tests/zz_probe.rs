//! Throwaway probe (deleted before committing): how a leaf appears as a branch grows.
use bevy_flora::foliage::leaf_points_for_node;
use bevy_flora::params::FoliagePlacement;
use bevy_flora::{TreePreset, generate_skeleton};
use std::collections::HashMap;

#[test]
fn probe_leaf_appearance() {
    let step: f32 = std::env::var("STEP").ok().and_then(|s| s.parse().ok()).unwrap_or(0.05);
    for preset in TreePreset::ALL {
        let params = preset.params();
        if params.foliage.placement != FoliagePlacement::AllBranches {
            continue;
        }
        let full = generate_skeleton(&params);
        let mature = full.aged(f32::INFINITY, &params.growth);
        let mature_age = full.mature_at(&params.growth);
        let mut mature_size: HashMap<(u32, u32), f32> = HashMap::new();
        for (i, n) in mature.nodes.iter().enumerate() {
            let mut pts = Vec::new();
            leaf_points_for_node(&mature, i, &params.foliage, &params.growth, &params.trunk, params.seed, &mut pts);
            for p in pts {
                mature_size.insert((n.id, p.tint.to_bits()), p.size);
            }
        }
        let mut prev: HashMap<(u32, u32), f32> = HashMap::new();
        let mut appeared = 0usize;
        let mut worst = 0.0f32;
        let mut sum_first = 0.0f32;
        let mut big = 0usize;
        let mut max_jump = 0.0f32;
        let mut sum_jump_area = 0.0f64;
        let mut steps = 0;
        let mut age = 0.0f32;
        while age <= mature_age {
            let sk = full.aged(age, &params.growth);
            let mut now: HashMap<(u32, u32), f32> = HashMap::new();
            for (i, n) in sk.nodes.iter().enumerate() {
                let mut pts = Vec::new();
                leaf_points_for_node(&sk, i, &params.foliage, &params.growth, &params.trunk, params.seed, &mut pts);
                for p in pts {
                    let key = (n.id, p.tint.to_bits());
                    let m = mature_size.get(&key).copied().unwrap_or(p.size);
                    let ratio = if m > 0.0 { p.size / m } else { 1.0 };
                    let before = prev.get(&key).copied().unwrap_or(0.0);
                    let jump = ratio - before;
                    if before == 0.0 && ratio > 0.0 {
                        appeared += 1;
                        sum_first += ratio;
                        if ratio > 0.5 {
                            big += 1;
                        }
                        worst = worst.max(ratio);
                    }
                    if jump.abs() > max_jump {
                        max_jump = jump.abs();
                    }
                    sum_jump_area += (jump.abs() as f64) * (m as f64) * (m as f64);
                    now.insert(key, ratio);
                }
            }
            prev = now;
            age += step;
            steps += 1;
        }
        println!(
            "{:15} steps {:4} appeared {:6} mean first ratio {:.2} > 0.5: {:5} ({:.0}%) worst first {:.2} max one-step jump {:.2} mean |jump| area/step {:.4}",
            preset.name(),
            steps,
            appeared,
            sum_first / appeared.max(1) as f32,
            big,
            100.0 * big as f32 / appeared.max(1) as f32,
            worst,
            max_jump,
            sum_jump_area / steps as f64
        );
    }
}
