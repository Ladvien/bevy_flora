//! Compares two gallery renders, PNG by PNG.
//!
//! ```text
//! cargo run --release --example compare_gallery -- <reference dir> <candidate dir>
//!     [--filter <substr>]... [--mean <max>] [--close <min %>] [--list <n>]
//! ```
//!
//! Renders the same scenarios with the CPU path (`gallery`) and with
//! `gallery --gpu`, then reports for every PNG both directories share how far
//! the two renders are apart: the mean absolute difference over the colour
//! channels (in 1/255) and the share of pixels whose largest channel differs by
//! at most 8/255. The overlay text in the top-left corner is ignored (it shows
//! the scenario's position in the run).
//!
//! `--mean` and `--close` set the limits; the defaults (2.5 and 92.0) are the
//! worst the 319 gallery scenarios measured when `GpuGrowth` shipped (willow, whose
//! `AllBranches` leaves the CPU path respaces: 2.2 and 93.0), plus a margin. The
//! exit code is 1 when any compared image is outside them. `--list` prints the
//! worst `n` images (default 12).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, Image, ImageSampler, ImageType};

/// Overlay text region of the first view, in pixels.
const OVERLAY: (u32, u32) = (420, 64);
/// A pixel is "close" when no channel differs by more than this.
const CLOSE: u8 = 8;

struct Score {
    mean: f32,
    close: f32,
}

fn load(path: &Path) -> Image {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    Image::from_buffer(
        &bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::default(),
        RenderAssetUsages::default(),
    )
    .unwrap_or_else(|e| panic!("decode {}: {e}", path.display()))
}

fn score(a: &Image, b: &Image) -> Score {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()), "image sizes differ");
    let (da, db) = (a.data.as_deref().expect("pixels"), b.data.as_deref().expect("pixels"));
    let width = a.width() as usize;
    let (mut sum, mut channels, mut close, mut pixels) = (0u64, 0u64, 0u64, 0u64);
    for (i, (pa, pb)) in da.as_chunks::<4>().0.iter().zip(db.as_chunks::<4>().0).enumerate() {
        let (x, y) = ((i % width) as u32, (i / width) as u32);
        if x < OVERLAY.0 && y < OVERLAY.1 {
            continue;
        }
        let diffs = [0, 1, 2].map(|c| pa[c].abs_diff(pb[c]));
        sum += diffs.iter().map(|&d| u64::from(d)).sum::<u64>();
        channels += 3;
        pixels += 1;
        close += u64::from(diffs.iter().all(|&d| d <= CLOSE));
    }
    Score {
        mean: sum as f32 / channels as f32,
        close: 100.0 * close as f32 / pixels as f32,
    }
}

fn pngs(dir: &Path) -> BTreeMap<String, PathBuf> {
    fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "png"))
        .filter_map(|p| Some((p.file_stem()?.to_string_lossy().into_owned(), p)))
        .collect()
}

fn main() {
    let mut dirs = Vec::new();
    let mut filters: Vec<String> = Vec::new();
    let (mut max_mean, mut min_close, mut list) = (2.5f32, 92.0f32, 12usize);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| usage());
        match arg.as_str() {
            "--filter" => filters.push(value()),
            "--mean" => max_mean = value().parse().unwrap_or_else(|_| usage()),
            "--close" => min_close = value().parse().unwrap_or_else(|_| usage()),
            "--list" => list = value().parse().unwrap_or_else(|_| usage()),
            _ => dirs.push(PathBuf::from(arg)),
        }
    }
    let [reference, candidate] = dirs.as_slice() else { usage() };
    let (reference, candidate) = (pngs(reference), pngs(candidate));

    let mut scores = Vec::new();
    for (name, path) in &candidate {
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        if let Some(other) = reference.get(name) {
            scores.push((name.clone(), score(&load(other), &load(path))));
        }
    }
    if scores.is_empty() {
        eprintln!("no PNG in common");
        std::process::exit(2);
    }

    scores.sort_by(|a, b| b.1.mean.total_cmp(&a.1.mean));
    println!("{:<34} {:>9} {:>9}", "scenario", "mean/255", "close %");
    for (name, s) in scores.iter().take(list) {
        println!("{name:<34} {:>9.3} {:>9.2}", s.mean, s.close);
    }
    let n = scores.len() as f32;
    let mean = scores.iter().map(|(_, s)| s.mean).sum::<f32>() / n;
    let worst_close = scores.iter().map(|(_, s)| s.close).fold(100.0, f32::min);
    println!(
        "{} images: mean of means {mean:.3}, worst mean {:.3}, worst close {worst_close:.2}%",
        scores.len(),
        scores[0].1.mean
    );
    let outside: Vec<_> = scores
        .iter()
        .filter(|(_, s)| s.mean > max_mean || s.close < min_close)
        .map(|(name, _)| name.as_str())
        .collect();
    if !outside.is_empty() {
        println!("{} outside mean <= {max_mean} and close >= {min_close}%: {}", outside.len(), outside.join(" "));
        std::process::exit(1);
    }
}

fn usage() -> ! {
    eprintln!("usage: compare_gallery <reference dir> <candidate dir> [--filter <substr>]... [--mean <max>] [--close <min %>] [--list <n>]");
    std::process::exit(2);
}
