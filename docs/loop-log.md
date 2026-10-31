# bevy_flora loop log

One entry per iteration of `.omp/commands/build-loop.md`, newest last. Append; never rewrite an
earlier entry.

## Iteration 0 — 2026-10-30 — loop setup

Written by the owner's session, not by a pass. It added `.omp/commands/build-loop.md` (from the
bevy_stigmergy loop command, rewritten for this crate) and this log; nothing under `src/`, `tests/`,
`Cargo.toml` or CI changed.

State at setup:

- `main` at `25e956b` (Merge PR #88, CI on big's self-hosted runners). `Cargo.toml` version 0.46.0.
- Latest published: **0.46.0** on crates.io (`cargo info bevy_flora`), tag `v0.46.0` dated
  2026-10-13 20:45 -0500, so the 6-hour release rule allows a release now.
- `fix/plant-lean-share` holds **0.46.1** (`f45fac5` plant lean as a share of the plant's height,
  `1e4c379` qa D-071/D-072 sway retune): version bump, `## [0.46.1] - 2026-10-15` CHANGELOG entry,
  `src/presets.rs`, `src/shading/common.wesl`, `src/shading/mod.rs`, `qa/visual_findings.md`. It was
  never merged or published. `git merge-tree --write-tree origin/main origin/fix/plant-lean-share`
  merges without conflict (checked at setup).
- `CHANGELOG.md` exists but has no `## [Unreleased]` heading (its top entry is `## [0.46.0]`): the
  first pass that ships adds one above the newest release.
- `cargo fmt --check` fails on the existing tree and CI does not run it, so it is not in the gate.
- CI (`.github/workflows/ci.yml`): build with declared features, `--features serde`, test,
  clippy, docs, examples, `cargo package --locked`; and `gpu-smoke`, the gallery with and without
  `--gpu` on lavapipe under `xvfb-run`, compared with `compare_gallery`. No wasm build.
- Gallery: 491 scenarios (README). `qa/visual_findings.md` open: D-052 (willow curtains, taste),
  D-067 (turned maple veins read pink close up, taste).
- No `benches/`; the forest benchmark is `examples/forest.rs --offscreen`, which prints one
  `forest <mode> ...` line with ms/frame, p95, median and cpu ms/frame.
- Host: big (Linux, Xvfb `DISPLAY=:87`, RTX 3090 shared through `gpu-tenant`). The README's frame
  times are from an Apple M4 Pro; big's numbers are not comparable to them, only to each other.

### Ranked backlog

Open issues first (bugs, then `consumer`, then the rest, oldest first). Neither open issue carries a
label; both come from Sanctuary.

1. **#89 Merge `fix/plant-lean-share` (0.46.1) into `main` and publish it.** Sanctuary pins
   `rev = "1e4c379"` on an unmerged branch, and crates.io (0.46.0) is older than what it runs.
   Ranked first: it is a consumer running on code that a branch deletion would lose, and it is
   ready. Merge with a merge commit (never rebase or squash: the issue's acceptance names
   `f45fac5` and `1e4c379`), run the full gate and the GPU smoke, re-record the README clips
   (`AGENTS.md`), then the Release block publishes 0.46.1. Close #89 with the crates.io URL.
   Acceptance: `git merge-base --is-ancestor 1e4c379 origin/main` succeeds and
   `cargo info bevy_flora@0.46.1` resolves.
2. **#87 Per-tree condition (healthy, sick, dying), settable and readable through `FloraIndex`.**
   Opened 2026-10-20. Its acceptance is five headless tests: set and read back through
   `FloraIndex`; a pooled tree keeps its condition after a pool recycle (or the condition is
   per-pool, documented, with a test); setting it on a felled tree does not panic; a `RecolorTree`
   and a condition change in one frame both apply without desyncing `SharedFlora`; existing tests
   pass. The issue leaves where the condition lives (entity component or index) to the crate:
   decide it, record the assumed decision on the issue. Keep the API general (no Sanctuary
   names); whether a condition also changes the look is a second slice.
3. **bevy_emerge #102 (F6): a stroke-bounded scatter.** The editor's scatter brush must plant
   trees only under the stroke, on the terrain. `Scatter::points_where(density)` can express a
   mask through a closure; measure whether a caller can get "only inside this polygon / along
   this capsule stroke, on `FloraGround`, deterministic per seed" without hand-rolling it, and
   add the general form if not. Acceptance: a test that a stroke's points all lie inside it and
   on the ground, and that the same seed gives the same points.
4. **bevy_emerge #102: same seed, same tree, across a `.tree.ron` round-trip.** A test that a
   `TreeParams` written with `to_ron` and read back with `from_ron` builds a tree whose vertex
   positions equal the original's bit for bit, for every preset (the editor saves `.tree.ron`,
   the game loads it). Acceptance: the test exists and passes; any field that does not
   round-trip is fixed.
5. **D-052: willow curtains.** The willow reads as a low mound with ~1 m of bare trunk above
   (seed 1). A generator feature (strands that keep hanging for a set length) rather than preset
   values. Acceptance: in `mature_willow`, the lowest leaf vertex sits below 30% of the tree's
   height and the crown silhouette is wider at its bottom third than at its middle, with
   `qa/visual_findings.md` D-052 moved to `## Fixed`.
6. **Baseline the forest bench on big.** No numbers from this host exist. Record three runs each
   of `forest --offscreen --deterministic --mode pool`, `--mode naive --trees 1000` and
   `--mode gpu-live` (ms/frame, p95, cpu ms/frame) in the first pass's log entry so later
   performance gates have a reference.

### For the owner

- GPU: the gallery, `capture_growth`, `forest` and the GPU smoke open a window; on big they need
  `DISPLAY=:87` and either the RTX 3090 (check `gpu-tenant status`) or software Vulkan. Every
  release re-records 11 README clips (`AGENTS.md`), which needs the display.
- `skill://bevy-flora-visual-qa` describes the owner's Mac clone (`/Users/ladvien/pixy_tree`) and
  319 scenarios; the gallery has 491 now. The command uses its checklist, not its paths.

## Iteration 4 — 2026-10-30 — 0.46.1 lands on main; the lean counts a tree's scale once (#90)

Host: this Mac (Apple M5, Metal, macOS 26.6.2), not big; `gpu-tenant` does not exist here. Clone
at `d07d870`, the baseline built from a detached worktree (`~/f4_base`). Passes 1, 2 and 3 left
no log entries: their work is on `origin/loop/iter-1`, `-2` and `-3` (pass 3 merged the first two
into itself, `e2e7c79`), and **`origin/main` was never pushed**, so #89 to #94 were all still open
and crates.io was still 0.46.0.

**State found.** Open issues: #89 (merge `fix/plant-lean-share`, 0.46.1), #90 (bug: the lean counts a
tree's `Transform` scale twice, blocked by pass 2 on a protected test), #91 (bug: crossed
`min_radial_segments` > `max_radial_segments` panicked), #92 (bug: `FloraIndex` queries with an
infinite bound never returned), #93 (bug: the forest bench's cpu figure on Linux), #94 (bug: the
release checklist re-recorded five clips of eleven), #87 (enhancement). No parked `wiggum/*`
branch. home-still: `system_status` ok (18 661 documents, 18 070 embedded), scribe healthy, distill
answered (the first call timed out at 30 s and succeeded when given 240 s).

**Pick.** Land the stranded passes and fix #90, then release 0.46.1. A bug outranks #87, and #90 was
a bug *in* the code #89 merges, so merging #89 alone would have put a known bug on `main`. Runner-up:
#87 (per-tree condition), untouched. Merged `origin/loop/iter-3` with a merge commit (it carries
`f45fac5` and `1e4c379`, which #89's acceptance names, and the finished commits of passes 2 and 3; none
is a `WIP(pass N, parked)` commit), then ran the gate on the result myself.

**What changed.**

- #90: `wind_bend`'s lean is `MAIN_BEND * sway * length(rel)`; `rel = world - origin` is already in
  world units, so the `scale_y` factor of 0.46.1 counted the scale twice. The function takes no scale
  and its six callers no longer pass one. The old test
  `the_lean_comes_from_the_vertexs_own_distance_from_the_root` finds the lean line by `scale_y` and a
  loop may not loosen it: **Assumed decision (owner may override)**: leave it alone and end the lean
  line with a comment that names `scale_y`, and add
  `wind_bend_takes_no_scale_and_the_lean_has_no_scale_factor` (comments stripped; fails on 0.46.1, run
  with the old shaders put back). Only a doc comment of a neighbouring test was corrected.
- Clippy on the stranded work: `FloraIndex::cells_in` (pass 3) failed `clippy::reversed_empty_ranges`
  under `-D warnings`; its empty ranges are built with `RangeInclusive::new(1, 0)`.
- All eleven README clips re-recorded; the README's GPU parity line says 492 scenarios (measured:
  mean 0.077, worst 2.109, 79 byte-identical, so 0.08, 2.11 and 79 stand); `ARCHITECTURE.md`,
  `WindResponse::sway`'s rustdoc, the README's wind paragraph, the 0.46.1 CHANGELOG entry (the Known
  issue became a Fix), `qa/visual_findings.md` (D-073 to Fixed, a `loop-04` line).

**Sources.** Botanical: Minamino and Tateno 2014 (`10.1371_journal.pone.0093535`), McMahon and
Kronauer's elastic similarity ("the deflection angle ... is determined by its relative position on
the branch, independent of branch length"); Eloy et al. 2017 (`10.1038_s41467-017-00995-6`).
Shipping game: Sousa, GPU Gems 3 chapter 16 (Crysis; web). Technical: Zioma, GPU Gems 3 chapter 6;
Pelzer, GPU Gems 1 chapter 7 (`GPU Gems- Programming Techniques...`). All quoted in
`docs/research/wind.md`. Corpus re-scan added three documents (`10.1111_cgf.13263`,
`10.14214_sf.a8529`, `sig2024_Scintilla_...`). Pending index: none new.

**Numbers** (`/tmp/f4/findings.md`, removed with the scratch).

- Gallery 491 -> 492 scenarios, 0 flags before and after, no BLANK_CAPTURE or CAPTURE_TIMEOUT, no
  PNG under 100 KB; `stats.tsv` columns 1-10 identical on the 491 shared scenarios; every image
  identical to main's below the overlay (mean of means 0.000; the gallery has no wind). New
  `extreme_crossed_sides`: three-sided oak, closed, leaves attached.
- Wind (`zz_probe`, deleted): a mature pine, steady +x wind of strength 1, crown top shift over height.
  Scale 0.3 / 1 / 3 / 7: 0.69% / 2.36% / 7.25% / 17.0% before, 2.36% at all four after (ratio of 3 to 1:
  3.07 -> 1.00), as a `ProceduralTree`, with `GpuGrowth`, pool levels 0-2 and the level 3 impostor.
- Forest bench, three runs, median of medians, ms/frame and cpu ms/frame, main -> this tree: pool
  10 000 4.71 / 12.72 -> 4.92 / 14.72 in sequential sittings, and interleaved four times each
  4.945 / 14.715 -> 4.955 / 14.675 (the same base binary moved from 12.4-13.5 to 14.7 cpu between
  sittings: the host, not the code); naive 1 000 35.49 / 19.60 -> 34.83 / 18.77; gpu-live 1 000, 200
  frames (the 10 000 default never finished here) 185.09 / 251.50 -> 188.48 / 250.95. Build probe: 20
  presets, mature and age 4, vertex and triangle counts identical, times within noise. The bench
  now polls (`UpdateMode::reactive(ZERO)`) offscreen on every OS, not just Linux: the interleaved
  runs show no cost on macOS.
- GPU parity, seven CI scenarios: mean of means 0.581, worst 1.205 (same as main to the digit); full
  gallery 0.077 / 2.109. Determinism: two runs of the seven, columns 1-10 and every PNG identical.
- Tests: 323 lib + 65 doc green; clippy `--all-targets --all-features -D warnings`, `cargo doc -D
  warnings`, `cargo package --locked` (verify build) clean.

**Critique and verdicts** (`/tmp/f4/critique.md`): the permanent guard for #90 reads shader source (a
GPU test is not allowed), so a different way of scaling twice would not trip it; the behaviour is proven
by the probe on every path. #90 GREAT. Landing 0.46.1 GREAT after the clippy fix. The clips: growth,
grove against main's binary on this machine differ at leaf edges (the 0.46.1 sway retune), nothing
else; first, middle and last frame of all eleven looked at. Passes 1-3's gate claims were not true
(clippy failed); their regression tests (#91, #92) pass.

**Gate gaps, said plainly.** The disk was at 97% before the pass and 100% during it (the shared
`CARGO_TARGET_DIR` holds 79 GB of debug builds). `cargo build --examples` links 2 to 4 GB of debug
binaries and filled it, so the gate ran `cargo test --all-features --lib --tests` plus `--doc`, and
`cargo check --examples`; clippy `--all-targets --all-features` compiles every example. A shared
`CARGO_TARGET_DIR` across two worktrees also made cargo call an example binary fresh by mtime after
a source change: `touch src/lib.rs` before trusting a rebuilt example. `cargo-semver-checks` is not
installed and a baseline rustdoc build would not fit; the version is 0.46.1 by reading the diff
(`git diff v0.46.0..HEAD -- src | grep '^[+-].*pub '`: no public item changed). `Cargo.lock` carries
`clipboard-win 5.4.1`, which crates.io marks yanked (bevy's transitive dependency).

**Commits** (`loop/iter-4`): `ca5bcb3` Research: wind, `da71cb2` Research: corpus, `88ddba7` the fix
(Fixes #90), `a132924` docs, `1544352` clippy, `e846e11` clips, `e6dd828` qa.

**Issues.** #90, #92, #93, #94 carry `Fixes` in their commits and close when `main` is pushed; #93 is
Linux-only code that this host cannot run, so it is landed, not verified; #89 and #91 close with the
release. #87 skipped (enhancement, not reached).

### Ranked backlog after iteration 4

1. **#87 per-tree condition** (healthy, sick, dying), through `FloraIndex`, five headless tests in the
   issue; decide entity component or index, record the assumed decision.
2. **bevy_emerge #102 (F6) stroke-bounded scatter**: points only under a brush stroke, on `FloraGround`,
   deterministic per seed.
3. **bevy_emerge #102: `.tree.ron` round trip**: same seed, same vertices bit for bit for every preset
   through `to_ron` / `from_ron`.
4. **D-074 / D-075**: a lone gallery scenario draws no leaves; the gallery's renders are not
   byte-identical run to run on big. Both undermine the harness; D-075 did not show on this Mac.
5. **D-052 willow curtains**, then **owner clean-up of #90's locator** (drop `scale_y` from the old
   test's locator and the comment together) and a CPU-side behavioural mirror of `wind_bend` held to
   the shader as `growth_common.wesl` is held to `frame.rs`.
