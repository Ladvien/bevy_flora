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
