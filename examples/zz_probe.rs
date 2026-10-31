//! Throwaway: build every preset with every numeric field set to hostile values.
//! `zz_probe --count` prints the case count; `zz_probe --from N [--to M]` runs cases N..M.
use bevy_flora::{LodSettings, TreeParams, TreePreset, build_lods, build_tree};
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

struct Case {
    label: String,
    preset: &'static str,
    text: String,
    age: f32,
}

const PRESETS: &[&str] = &["oak", "willow", "spruce", "fern", "palm", "apple", "cherry", "dragon_tree"];
const FLOATS: &[&str] = &["0.0", "-1.0", "1e-6", "1e6", "1e30", "-1e30", "NaN", "inf"];
const INTS: &[&str] = &["0", "1", "255", "65535", "4294967295", "1000000", "-1"];

fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    for name in PRESETS {
        let preset = TreePreset::from_name(name).expect("preset");
        let base = preset.params().to_ron();
        let lines: Vec<&str> = base.lines().collect();
        let mut stack: Vec<String> = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.ends_with('(') {
                let key = t.split(':').next().unwrap_or("").to_string();
                stack.push(key);
                continue;
            }
            if t.starts_with(')') {
                stack.pop();
                continue;
            }
            let body = t.trim_end_matches(',');
            let (key, val) = match body.split_once(": ") {
                Some((k, v)) => (k, v),
                None => ("", body),
            };
            if val.parse::<f64>().is_err() {
                continue;
            }
            let float = val.contains('.') || val.contains('e') || val.contains('E');
            let values = if float { FLOATS } else { INTS };
            let group = stack.get(1).cloned().unwrap_or_default();
            let ages: &[f32] = if group == "growth" { &[f32::INFINITY, 3.0] } else { &[f32::INFINITY] };
            for v in values {
                let indent = &line[..line.len() - line.trim_start().len()];
                let mut copy = lines.iter().map(|l| l.to_string()).collect::<Vec<_>>();
                copy[i] = format!("{indent}{}{v},", if key.is_empty() { String::new() } else { format!("{key}: ") });
                let text = copy.join("\n");
                for age in ages {
                    out.push(Case {
                        label: format!("{}.{key}[{i}]={v}", stack.iter().skip(1).cloned().collect::<Vec<_>>().join(".")),
                        preset: name,
                        text: text.clone(),
                        age: *age,
                    });
                }
            }
        }
    }
    for preset in TreePreset::ALL {
        for age in [f32::NAN, -1.0, 0.0, 1e30, f32::INFINITY, 1e-30, 1e-3] {
            out.push(Case {
                label: format!("age={age}"),
                preset: preset.name(),
                text: preset.params().to_ron(),
                age,
            });
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let all = cases();
    if args.iter().any(|a| a == "--count") {
        println!("{}", all.len());
        return;
    }
    let get = |flag: &str, default: usize| {
        args.iter().position(|a| a == flag).and_then(|p| args.get(p + 1)).and_then(|v| v.parse().ok()).unwrap_or(default)
    };
    let from = get("--from", 0);
    let to = get("--to", all.len());
    std::panic::set_hook(Box::new(|_| {}));
    let lod = LodSettings::default();
    let stdout = std::io::stdout();
    for (idx, case) in all.iter().enumerate().skip(from).take(to.saturating_sub(from)) {
        {
            let mut o = stdout.lock();
            let _ = writeln!(o, "START\t{idx}\t{}\t{}\t{}", case.preset, case.label, case.age);
            let _ = o.flush();
        }
        let params = match TreeParams::from_ron(&case.text) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let start = Instant::now();
        let built = catch_unwind(AssertUnwindSafe(|| {
            let t = build_tree(&params, case.age);
            let mut verts = t.trunk.positions.len() + t.foliage.positions.len() + t.flowers.positions.len();
            let mut bad = None;
            for (n, m) in [("trunk", &t.trunk), ("foliage", &t.foliage), ("flowers", &t.flowers)] {
                if m.positions.iter().any(|p| !p.is_finite()) {
                    bad = Some(format!("{n} positions not finite"));
                }
                if m.normals.iter().any(|p| !p.is_finite()) {
                    bad = Some(format!("{n} normals not finite"));
                }
                if m.indices.iter().any(|&i| i as usize >= m.positions.len()) {
                    bad = Some(format!("{n} index out of range"));
                }
                if m.indices.len() % 3 != 0 {
                    bad = Some(format!("{n} indices not a multiple of 3"));
                }
            }
            let build_ms = start.elapsed().as_secs_f64() * 1000.0;
            if verts < 3_000_000 {
                let l = build_lods(&params, case.age, &lod);
                for m in &l {
                    verts += m.trunk.positions.len() + m.foliage.positions.len();
                    for mm in [&m.trunk, &m.foliage] {
                        if mm.positions.iter().any(|p| !p.is_finite()) {
                            bad = Some("lod positions not finite".into());
                        }
                        if mm.indices.iter().any(|&i| i as usize >= mm.positions.len()) {
                            bad = Some("lod index out of range".into());
                        }
                    }
                }
            }
            (verts, build_ms, bad)
        }));
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let mut o = stdout.lock();
        match built {
            Ok((verts, build_ms, bad)) => {
                let status = match (&bad, ms) {
                    (Some(_), _) => "BAD",
                    (_, m) if m > 2000.0 => "SLOW",
                    _ => "OK",
                };
                if status != "OK" {
                    let _ = writeln!(o, "RESULT\t{idx}\t{status}\t{}\t{}\t{}\t{ms:.0}\t{build_ms:.0}\t{verts}\t{}", case.preset, case.label, case.age, bad.unwrap_or_default());
                }
            }
            Err(e) => {
                let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                let _ = writeln!(o, "RESULT\t{idx}\tPANIC\t{}\t{}\t{}\t{ms:.0}\t0\t0\t{msg}", case.preset, case.label, case.age);
            }
        }
        let _ = o.flush();
    }
    println!("DONE");
}
