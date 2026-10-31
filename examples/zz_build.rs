//! Throwaway: the time of `build_tree` for every preset, mature and at age 4.
use bevy_flora::{TreePreset, build_tree};
use std::time::Instant;

fn main() {
    for preset in TreePreset::ALL {
        let params = preset.params();
        for age in [f32::INFINITY, 4.0] {
            let mut times = Vec::new();
            let mut last = None;
            for _ in 0..9 {
                let start = Instant::now();
                let tree = build_tree(&params, age);
                times.push(start.elapsed().as_secs_f64() * 1000.0);
                last = Some(tree);
            }
            times.sort_by(|a, b| a.total_cmp(b));
            let tree = last.expect("built");
            println!(
                "{}\t{}\t{:.2}\t{}\t{}\t{}\t{}",
                preset.name(),
                age,
                times[4],
                tree.trunk.positions.len(),
                tree.trunk.indices.len() / 3,
                tree.foliage.positions.len(),
                tree.foliage.indices.len() / 3
            );
        }
    }
}
