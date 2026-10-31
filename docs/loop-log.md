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

**Release and clean-up (iteration 4).** Merge `63439ab` on `main` (pushed, tree identical to
`loop/iter-4`, so the gate stood), `cargo publish` **bevy_flora 0.46.1**, tag `v0.46.1`, GitHub
release: https://crates.io/crates/bevy_flora/0.46.1 (`cargo info bevy_flora@0.46.1` resolves).
#89, #90, #91, #92, #93 and #94 are closed (#90 carries the `assumed-decision` label); #87 is open.
Removed, mine: the worktree `~/f4_base` and `~/f4_base_bin`, the release-profile example binaries and
incremental caches and the debug example binaries and incremental caches in `CARGO_TARGET_DIR`
(`/Users/ladvien/target/flora-loop`), `/tmp/f4` and its findings and critique files. Kept: that
target directory's dependency builds, release ones included (about 12 GB added by this pass: the
release profile did not exist there before, and the debug default-feature variant is new). The disk
was 16 GB free when the pass began and is 3.7 GB now (`df -h /`, 100% by Apple's rounding): the next
pass on this Mac should free space first (`cargo build --examples` alone needs 2 to 4 GB). Leftovers
of earlier passes: none found (`git worktree list` shows only the clone; no `f*` directories in `~`
or `/tmp`); the branches `loop/iter-1`, `-2`, `-3` and `-4` remain on origin and are not deleted.

## Iteration 5 — 2026-10-31 — TreeCondition: a tree a game can call sick or dying (#87)

Host: this Mac (Apple M5, Metal, macOS 26.6.2), not big. Disk 13 GB free at the start, 6 GB at the end
(the shared `CARGO_TARGET_DIR` is 73 GB). HomeStill: `system_status` ok (18 668 documents, 488 794
embedded chunks), `scribe_health` ready (11 VLM slots); `distill_search` answered a GPU-busy 500 for two of
twelve queries (bge-m3 wanted 5 000 MB, 4 794 free) and the retries returned. Corpus re-scan of
`catalog_recent` (200): one phenology paper added (`10.5194_bg-2015-619`); the other 199 were unrelated.

**State found.** One open issue, #87 (enhancement, opened 2026-10-20, skipped last pass). No bug, no
`consumer`-labelled issue, no parked branch (`git branch --list 'wiggum/parked-*'` empty; the
`WIP(pass 1, checkpoint)` commits are driver snapshots). bevy_emerge's F6 (#102) is closed: its own loop
landed the stroke-masked scatter, single-tree placement, the `.tree.ron` editor and `forest_view` against
this crate (so backlog items 2 and 3 of iteration 0 were met on its side). v0.46.1 was tagged 49 minutes
before this pass ended, so the 6-hour release rule allows no release: the change rides in the next one.

**Pick.** #87, by the rule that an open enhancement outranks a candidate the pass invents. Runner-up:
the look of a condition (below), which the issue itself leaves as a separate matter. Ranked candidates:
(1) #87 state, (2) #87 look (colour, defoliation), (3) `.tree.ron` round trip bit for bit for every
preset, (4) D-074 / D-075 harness bugs, (5) D-052 willow curtains.

**What changed.**

- `TreeCondition { Healthy, Sick, Dying }` (`src/condition.rs`): a component on the tree's entity,
  ordered, reflected and registered. A tree without one is healthy, so a 100 000-tree pooled forest pays
  nothing for the trees that are well. Set by inserting, cleared by inserting `Healthy` or removing it.
- `IndexedTree::condition` and `FloraIndex::condition_of(entity) -> Option<TreeCondition>`. `index_trees`
  reads the component whenever it inserts a tree and patches the entry in place from
  `RemovedComponents` then `Changed<TreeCondition>` (removals first: a remove and an insert in one frame
  end at the insert; a tree not indexed has no entry to patch).
- Per instance, not per pool: survives a variant change, a pool reload and a `PooledTree` taken off and
  put back; a condition set before the pool exists shows when it does; a felled tree leaves the index
  (`None`) and keeps the component.
- **Assumed decision (owner may override)**, recorded on #87 with the `assumed-decision` label: a
  component on the entity, not a `SetTreeCondition` message or an index-owned map (a message that only
  inserts a component is a second way to do one thing; Bevy's change detection is the notification), and
  `Option<TreeCondition>` from `condition_of` (the issue's example returned a bare value). No `Dead`
  class: a dead tree is the game's to fell. State only: the crate draws a sick tree as a healthy one.
- **Breaking:** `IndexedTree` has a new public field, so the next release is 0.47.0. `index.rs`'s test
  helper `tree()` gained `condition: Healthy` (an assumed owner exception: a new field's default in a
  literal; no assertion changed; stated in the commit).
- Docs: rustdoc, a crate-root doctest (`# Sick and dying trees`), the README's "Finding trees" section
  (its snippet gained `blight` and compiles as a doctest), `ARCHITECTURE.md`, `CLAUDE.md`, an
  `## [Unreleased]` CHANGELOG entry (the first; none existed), a `loop-05` line in `qa/visual_findings.md`.

**Sources.** Botanical: `10.3390_f8040137` ("Crown defoliation is the most widely used parameter for the
assessment of tree vitality"), `10.1007_s10342-005-0085-3` (transparency classes; "next-year mortality
rate increased exponentially with transparency"), `10.3389_fpls.2026.1809811` (stand grades by crown
dieback, ~10 / 30 / 50 / 80%), `10.1038_s41467-020-18996-3` (mode of death). Shipping game: Timberborn's
alive / dying / dead plants as one mod's notes on the decompiled game record them
(<https://github.com/timbermods/dam-good-maps/pull/742>, a web page, second hand), Gregory ch. 16 (a game
object is its attributes and behaviours). Technical: `10.1145_3748522.3779910` (components added to and
removed from entities), Bevy 0.20 `Changed` / `RemovedComponents` doc comments. All quoted in
`docs/research/tree-condition.md`. Pending index: Dobbertin and Brang 2001, Timberborn first-party.

**Numbers** (`/tmp/f5/findings.md`, removed with the scratch).

- Gallery narrowed to the seven CI scenarios (the change reaches no mesh or draw call; the full 492 not
  run, said plainly): 7 PNG = 7 rows, none under 100 KB, 0 flags, no BLANK_CAPTURE / CAPTURE_TIMEOUT;
  against main's binary `stats.tsv` columns 1-10 identical, 0 of 7 PNG differ (`compare_gallery` mean
  of means 0.000); two new CPU runs byte for byte alike; CPU vs `--gpu`: mean of means 0.581, worst 1.205,
  exit 0, the same to the digit as main's and as iteration 4's. Warnings: only the asset-watcher WARN
  (binaries run from `/tmp`, no `assets` dir; textures are compiled in) and the benign winit `Destroyed`.
- Forest bench, interleaved base / new, three runs each, median of medians ms/frame (cpu ms/frame): pool
  10 000 4.90 (14.58) -> 4.91 (14.70); naive 1 000 34.87 (18.57) -> 34.86 (18.62); gpu-live 1 000, 200
  frames 185.11 (247.65) -> 186.67 (250.05). All within 1.0%. No building code touched, so no build probe.
- Smoke at forest scale (a throwaway example, deleted): 10 000 pooled oaks spawn and index in 22.6 ms,
  an idle update is 1.1 ms, 1 993 conditions inserted through `within` take 0.8 ms and one update
  applies them in 0.5 ms; the index reads back 1 993 dying and none outside the radius; a scan for the
  ill is 0.03 ms; a felled one: `condition_of` `None`, component kept, index 9 999.
- Tests: 328 lib (323 + 5 new) and 66 doc green; clippy `--all-targets --all-features -D warnings`,
  `cargo doc -D warnings`, `cargo build`, `--features serde`, `--release`, `cargo check --examples`,
  `cargo package --locked` (verify build) clean. Mutation check of the new tests: no-`Changed` path,
  no-removal path, procedural insert ignoring the component and pooled insert ignoring it each fail a
  test (the third survived the first draft; the move-and-rebuild step was added for it).

**Critique and verdict** (`/tmp/f5/critique.md`): the feature is state only, so a sick tree is drawn as a
healthy one and a game shows it with `PooledLook` / `RecolorTree` beside the component (two components to
keep in step); the recolour test cannot fail until the look slice exists and says so; `IndexedTree`'s new
field breaks hand-built literals. #87's five criteria: GREAT. The look: not built, NOT YET, first in the
backlog.

**Gate gaps, said plainly.** `cargo build --examples` (default features) was `cargo check --examples`
because of the disk (clippy `--all-targets --all-features` compiled every example); the full 492-scenario
gallery and the eleven README clips were not re-run (nothing that draws changed, no release this pass).
`cargo-semver-checks` is not installed: 0.47.0 follows from the new public field.

**Commits** (`loop/iter-5`): `8dd1ed1` Research: corpus, `323ba42` Research: tree-condition, `f1ce6c9`
the implementation (Fixes #87), `5e81b2f` docs.

### Ranked backlog after iteration 5

1. **The look of a condition** (#87 second slice): `MeshTag` per instance on a pool's mesh entities
   (batches survive), read in the leaf and wood fragment stages and the leaf vertex stage; foliage colour
   green to ochre and a share of leaves hidden by the per-leaf hash `FloraFire` uses (~30-50% sick, ~80%
   dying); impostors and `GpuGrowth` follow; gallery scenarios for each class. Design in
   `docs/research/tree-condition.md`.
2. **`.tree.ron` round trip**: `to_ron` / `from_ron` builds vertex-identical trees for every preset (the
   editor writes the file, the game loads it); emerge's own tests only check the seed and one field.
3. **Release 0.47.0** (breaking: `IndexedTree::condition`) once the tag is 6 hours old: re-record the
   eleven clips, `cargo semver-checks` if installed.
4. **D-074 / D-075**: a lone gallery scenario draws no leaves; renders not byte-identical run to run on big.
5. **D-052 willow curtains**; owner clean-up of #90's locator test; a CPU mirror of `wind_bend`.

**Clean-up (iteration 5).** Merge `434f411` on `main` (pushed, tree identical to `loop/iter-5`, so the
gate stood). No release: v0.46.1 was 49 minutes old, the 6-hour rule; 0.47.0 (breaking: the new
`IndexedTree` field) carries it. #87 closed by the push (`Fixes #87`), with a comment on delivery; it
carries `assumed-decision`. Baseline kept as copies of main's three release binaries under
`/tmp/f5/base` (no second worktree or target directory: `git worktree list` shows only the clone).
Removed, mine: `/tmp/f5` (720 MB: binaries, captures, findings, critique), the release example
binaries and incremental caches of `gallery`, `forest`, `compare_gallery` and `zz_probe`, the debug
incremental caches and `bevy_flora` rlibs and test binary this pass built, and
`package/bevy_flora-0.46.1*` in `CARGO_TARGET_DIR`; `df -h /` 6.1 GB free before, 12 GB after. Kept: the
target directory's dependency builds. Leftovers of earlier passes: none found (no `f*_base` in `~` or
`/tmp`); the branches `loop/iter-1` to `-5` remain on origin, not deleted.

## Iteration 6 — 2026-10-31 — TreeCondition is drawn: a sick crown thins and mottles, a dying one is bare and dry (#87)

Host: this Mac (Apple M5, Metal, macOS 26.6.2), not big. Disk 12 GB free at the start, about 4 GB before
the clean-up (the shared `CARGO_TARGET_DIR` is 78 GB). HomeStill: `system_status` ok (18 668 documents,
488 794 embedded chunks), `scribe_health` ok; `distill_search` and `hs distill search` answered a GPU-busy
500 to every query (bge-m3 wanted 5 000 MB, 2 500 to 2 800 free), so the sources were read with
`markdown_read` and `paper_search`. Corpus re-scan of `catalog_recent` (200): one relevant paper
(`10.3832_ifor0538-003`, crown defoliation as an indicator; in the catalog, its conversion did not return
within the call) and one general text (Flake, "The Computational Beauty of Nature"), as `Research: corpus`.

**State found.** No open issue on `bevy_flora` (the last five, #90-#94, are closed), no `consumer` label, no
parked branch (`git branch --list 'wiggum/parked-*'` empty), bevy_emerge's family issue F6 (#102) closed and
no open issue there naming this crate. v0.46.1 was 52 minutes old, so the 6-hour rule allows no release this
pass; 0.47.0 (breaking: `IndexedTree::condition`, now also `FloraUniform::withered`) carries it.

**Pick.** The look of a condition, the first item of iteration 5's backlog: #87 asks for a sick tree that "the
player can read ... as a symptom rather than as scenery", and iteration 5 shipped only the state. Ranked:
(1) the look; (2) release 0.47.0 (blocked until 04:42 local); (3) D-074 / D-075, the two harness bugs: **not
reproducible here**, `gallery --filter extreme_mesh` and `--filter extreme_crossed_sides` alone drew their
leaves and HUD at a warmup of 0, 5 and 20 s on Metal, so they cannot be worked on this host (reading
`gallery.rs`, the four cameras stand at the origin, inside the tree, until `Phase::Apply`, so the leaf
pipelines may not be requested until then; a suspicion for the next pass on big, not tested); (4) D-052
willow curtains; (5) a gradual decline. Dropped from the backlog: the `.tree.ron` round trip, which
`tree_asset::tests::every_preset_survives_a_tree_file` already holds (`from_ron(to_ron(p)) == p` for every
preset, and equal parameters build the same tree).

**What changed.**

- A tree is drawn in its `TreeCondition`. The class travels in bits 8-9 of the `MeshTag` of every mesh the
  tree is drawn with, above a pooled look's glide tag (the low byte): `sync_condition_tags` (PostUpdate,
  only trees that have the component) writes it, `spawn_pooled_trees` and `sync_pooled_looks` carry it with
  the look's tag, so equal mesh and material still batch and no material is made for a condition (a test
  holds the material count).
- The leaf vertex stages (main pass, prepass and shadow maps, GPU growth) hide `TreeCondition::defoliation`
  of the leaves (sick 0.35, dying 0.8; a per-leaf draw from the leaf's phase, not the season's moment) and
  move the colour multiplier the season gives each leaf toward a dry ochre-brown by
  `TreeCondition::withering` (0.5, 1.0, each leaf 0.4 either side) through `FloraUniform::withered`, the
  ratio of the dry colour to the species' leaf colour. Flowers and fruit (alpha below 0.5) and wood are not
  touched, so what `TreeFruit` counts is what is drawn; a hull crown keeps its facets. The impostor does it
  per leaf-sized patch of baked texels. A mix toward the dry colour, not a product with the season's ratio
  (the first build multiplied, and a sick maple's autumn leaves came out pink-white).
- Falling leaves: `condition::Decline` mirrors the draw in `falling.rs` (a leaf the shaders hide is not
  chosen to fall) and the burst carries each leaf's withering in the fourth lane of `ATTRIBUTE_FALL_FACE`
  for `falling.wesl`, so they fall as dry as those on the tree.
- The shaders read the tag only while the glide header's lane y or x says some tree has a condition or a look
  glides (`tags_read`, `glide::sync_condition_flag`).
- `gallery`: 30 `condition_*` scenarios (522 in all; two join CI's GPU-parity filters, additive);
  `inspect --condition sick|dying`; `forest --sick <share> --dying <share>`.
- Breaking: `FloraUniform::withered` is a new public field (with `IndexedTree::condition`, next release
  0.47.0).
- **Assumed decision (owner may override)**, to go on #87 (it carries `assumed-decision`): the shares 0.35 and
  0.8 (the survey's slightly-to-moderately and severely declining grades), the withering 0.5 and 1.0 and the dry
  colour srgb (0.47, 0.31, 0.11) are design defaults; the alternatives were the species' autumn colour for the
  dry leaf (a dying maple would look like autumn) and a continuous severity (the issue asks for three
  classes). Recorded as D-076.

**Sources.** Botanical: `10.3390_f8040137` ("It is a visual indicator of the relative amount of foliage in the
crown of the assessed tree"; "symptoms displayed on leaves and branches"), `10.1007_s10342-005-0085-3`
("defoliation by insects becomes first visible in crown transparency ... foliage reduction becomes only
visible months later"), `10.3389_fpls.2026.1809811` (the grades). Shipping game: Timberborn's dry models (a
mod's notes on the decompiled game, second hand: "green to ochre", "A grown plant loses no leaves"). Technical:
Bevy 0.20 `MeshTag` ("the tag is free for you to use for whatever purpose you wish"), `get_tag`; this
crate's `glide.rs`. All quoted in `docs/research/tree-condition.md`. Pending index: the iForest paper, a
first-party source for how shipped games draw a dying tree.

**Numbers** (`/tmp/f6/findings.md`, removed with the scratch).

- Gallery, 522 scenarios on CPU and `--gpu`: 522 PNG = 522 rows, none under 100 KB, no flag, no
  BLANK_CAPTURE / CAPTURE_TIMEOUT, no `ERROR` or `panicked`. Against main's binary the 492 earlier
  scenarios: `stats.tsv` columns 1-10 identical, and every image identical outside the overlay (0 differing
  pixels in 492 of 492; the overlay's "(n/N)" counts the list, so the overlay band is masked).
- GPU parity: all 522 mean of means 0.073, worst 2.109 (`smoothyoung_shrub_4`, as in iteration 4); the 30
  condition scenarios 0.023 and worst 0.511 (`condition_oak_dying_6`); the nine CI scenarios 0.521 and
  1.205 (the seven before: 0.581 and 1.205).
- Forest bench, interleaved base / new, medians of the runs, ms/frame (cpu ms/frame): pool 10 000 4.93 (14.69)
  -> 4.97 (14.68) +0.9%; naive 1 000 34.81 (18.75) -> 34.95 (18.93) +0.4%; gpu-live 1 000, 200 frames 181.7
  (248.4) -> 178.9 (244.5) -1.5%. The Mac has a fast mode (pool median 4.5-4.6, cpu 13.9) that hits both
  binaries and made two earlier windows unusable; a forest with 25% sick and 12% dying trees, 4.75 ms against
  4.97 healthy (the lost leaves are not drawn). The first build cost +1.8% on the pool (the per-instance tag
  fetch); with a binary that has the condition code compiled out the same window read +1.0%, and the
  `tags_read` flag left +0.9%, so the fetch was not all of it. No build code touched, so no build probe.
- Looked at: `/tmp/f6/after/sheet_*.png` (every condition scenario, front view), `inspect` at levels 0, 1 and 2
  and the impostor at 70 m and 140 m, a mixed forest screenshot, the falling leaves of a dying oak.
- Tests: 334 lib (328 + 6 new) and 66 doc green; clippy `--all-targets --all-features -D warnings`,
  `cargo doc -D warnings`, `cargo build`, `--features serde`, `--release`. Three mutations each fail a test:
  removal not clearing the tags, `sync_pooled_looks` writing the look's tag without the condition, a drop
  that ignores the leaves a tree has lost.

**Critique and verdict** (`/tmp/f6/critique.md`): the state, the mesh look on every preset (all levels of
detail, shadow maps, GPU growth, falling leaves, composed with the season): GREAT. The impostor: GREAT at 70 m,
and at 140 m a dying crown is faint patches against the sky (four fifths gone), NOT YET only in that its colour
washes toward the sky. Open flaws: a class change pops (leaves vanish and recolour in one frame, the lost
leaves do not fall), three classes and no severity, wood and bark untouched, one dry colour for every species,
a casting impostor's shadow is the healthy silhouette. Two defects of the first build (the autumn product and
the impostor's salmon fringe) were found by looking, not by a test, and fixed before shipping.

**Gate gaps, said plainly.** `cargo build --examples` (default features) was clippy's `--all-targets
--all-features` (it compiles every example) because of the disk; no README clip was re-recorded (healthy
trees draw as before: the 492 images above; the release re-records all eleven). The docs ride in the
implementation commit (the harness staged them with it). `cargo-semver-checks` is not installed.
`cargo package --locked` verifies (131 files) and warns that `clipboard-win 5.4.1` in `Cargo.lock` is
yanked on crates.io (a transitive dependency of Bevy; not touched here).

**Commits** (`loop/iter-6`): `2bdd2b8` Research: tree-condition look, `14d1523` Research: corpus, `5958c9b`
the implementation and its docs.

### Ranked backlog after iteration 6

1. **Release 0.47.0** (breaking: `IndexedTree::condition`, `FloraUniform::withered`) once v0.46.1 is 6 hours
   old (04:42 local, Oct 31): re-record the eleven clips, `cargo semver-checks` if installed; a twelfth clip
   of a blighted stand would show the new look (`AGENTS.md` lists eleven, so that is an owner call).
2. **A gradual decline**: a class change pops. On a change toward ill the leaves the new class loses should
   fall (`DropLeaves` from the cards the old class kept and the new one hides) and the colour should glide,
   so "a copse that dies in one season" is a season, not a frame.
3. **A dying tree's wood and shadow**: bark greyed and cracked for `Dying`, and a casting impostor's
   silhouette thinned by the class.
4. **D-074 / D-075** on big: try the gallery's cameras placed at the first scenario's views during the warmup.
5. **D-052 willow curtains**; owner clean-up of #90's locator test; a CPU mirror of `wind_bend`.

**Clean-up (iteration 6).** Merge `63eea41` on `main` (pushed, tree identical to `loop/iter-6`, so the gate
stood). No release: v0.46.1 was 52 minutes old at the start and under 3 hours old at the end, the 6-hour rule;
0.47.0 carries this and iteration 5's work. #87 (closed in iteration 5, label `assumed-decision`) has a comment
on delivery and on the assumed decision. Baseline: copies of main's release binaries under `/tmp/f6/base` (no
second worktree or target directory: `git worktree list` shows only the clone). A `file_watcher` build reloads
an embedded shader when its file changes, so a baseline binary run while the shaders in the clone are edited is
not a baseline: no `.wesl` was edited during the windows quoted above. Removed, mine: `/tmp/f6` (1.9 GB:
binaries, galleries, captures, bench output, findings, critique), and what this pass built in
`CARGO_TARGET_DIR`: the release example binaries and the `bevy_flora` rlibs and test binaries of both profiles
(files modified in the last four hours), the release and debug incremental caches of that window, and
`package/bevy_flora-0.46.1*`; `df -h /` 12 GB free at the start, 3.4 GB before, 15 GB after. Kept: the target
directory's dependency builds (71 GB). Leftovers of earlier passes: none found (no `f*_base` in `~` or `/tmp`);
the branches `loop/iter-1` to `-6` remain on origin, not deleted.

## Iteration 7 — 2026-10-31 — Weeping strands: the willow's branches hang to the ground (D-052)

Host: this Mac (Apple M5, Metal, macOS 26.6.2), not big. Disk 15 GB free at the start, 5.6 GB at the end (the
shared `CARGO_TARGET_DIR` is 76 GB); `bevy_emerge#195` says the loop hosts are out of memory, and this pass ran
one build or one render at a time. HomeStill: `system_status` ok (18 669 documents, 488 829 embedded chunks;
Qdrant upserts time out and bge-m3 finds the GPU busy at times), `scribe_health` ok, `distill_search` answered
(thin for weeping habit; `paper_search` and the web gave the sources). Corpus re-scan of `catalog_recent` (200):
nothing a note needs that `corpus.md` did not list (Scintilla is already there; the rest of the batch is SIGGRAPH
rendering and medical text), so no `Research: corpus` commit.

**State found.** No open issue on `bevy_flora`, no `consumer` label, no parked branch, bevy_emerge's open family
issues (#96 F0, #103 F7) name nothing this crate lacks; its `forest_scatter` hand-rolls a polyline distance test
(`under_stroke`, 10 lines) around `Scatter::points_where`, which works and ships (not taken). v0.46.1 was 2 h 25 m
old, so the 6-hour rule allows no release this pass; 0.47.0 (breaking, see below) carries iterations 5, 6 and 7.

**Pick.** D-052, the willow, the one tree in the set that fails the brief's first test: "the silhouette reads as
the species it is named for". `mature_willow` and `inspect --preset willow` read as arches of bare wood on a
flat-capped pole with a thin scatter of leaves; eight preset variants had been tried and all gave the mound, so
it needed a generator feature. Ranked: (1) D-052 as a generator feature; (2) release 0.47.0 (blocked until 04:42);
(3) a gradual decline (#87's class change pops); (4) `Scatter::along` a polyline for bevy_emerge (they have it);
(5) D-074 / D-075 on big. The runner-up was the gradual decline: a game can live with a pop, and cannot ship that
willow.

**What changed** (`docs/research/weeping-strands.md`, committed first as `33ba0d7`).

- `BranchParams::strands` (`StrandParams`: `density`, `start`, `length`, `spread`, `clearance`) and
  `NodeKind::Strand`. After the skeleton is seated, every branch gets `density * length * (1 - start)` straight
  ropes hung from its outer part, plumb within `spread`, `length` times the trunk's height long or down to
  `clearance` above the floor, a twig's radius, two rings of wood. They draw from streams of their own and come
  after every other node: a tree without them is the tree it was and one with them keeps every other branch
  where it was (both tested). A strand is born when its branch reaches it and carries its leaves along its whole
  length; its leaf `i` keeps its place on the grown strand at every age, which is what `GpuGrowth` shows (the
  first build re-spread them as every branch does and the GPU differed by 2.3 of 255 on `age_willow_8`, 1.4 after).
  Levels of detail, felling, fire, wind and falling leaves needed nothing: a strand is a node.
- The willow uses them (`qa/visual_findings.md` D-077): strands 1.6 per metre from a tenth of each branch,
  0.7 of the trunk's height; limbs from 40% of the trunk (was 50%), `angle` 46 (50), `angle_curve` -0.1 (-0.4),
  `trunk.taper` 0.22 (0.4), leaves 6.5 per metre (10) and 0.34 m (0.26). A mature willow is 119k vertices (wood
  30k, leaves 93k) where it was 55k, builds in 4.4 ms where it took 2.2, level 1 holds 24k leaf vertices (9.5k)
  and level 2 9k (3.6k). Every other preset is unchanged.
- **Assumed decision (owner may override)**, in the commit body and D-077: the willow's look values. Options:
  strands alone (left bare arcs on a pole), the strands with the limb retune above (taken), or more branches
  (D-052's eight failed variants). The retune is a design default; the owner's reply wins.
- Found on the way, fixed: `TreeShape`'s crown box did not hold the leaves on a bent branch's arch (it spanned a
  branch's start and end; `the_crown_holds_every_leaf_and_the_box_holds_the_crown_at_every_age` caught it on the
  new willow at age 5), and under `pipe_radius_enabled` floor avoidance let drooping wood dip up to
  `pipe_radius_min` below the floor (the first build's `pipe_willow` `BELOW_FLOOR`); the pipe model also floored
  every strand to a 2 cm rod, so a strand keeps its own radius there. The pipe fix moves `pipe_shrub`,
  `pipe_fern` and their young scenarios by a centimetre or two (opt-in mode only).
- Breaking: `BranchParams::strands` (new field), `NodeKind::Strand` (new variant): 0.47.0.

**Sources.** Botanical: "Gravity sensing and responses in the coordination of the shoot gravitropic setpoint
angle" (doi `10.1111/nph.18474`, read at PMC9828789: "Stems grow vertically upward, whereas lateral organs ...
tend to grow at a specific angle according to a gravity vector known as the gravitropic setpoint angle (GSA)";
the WEEP gene "results in pendulous branch growth in peach trees"), Missouri Botanical Garden on *Salix
babylonica* ("a stout trunk topped by a graceful broad-rounded crown of branches that sweep downward to the
ground ... as wide" as tall), the URI identification sheet ("hanging or drooping for long distances, almost
rope-like"). Shipping game: the SpeedTree Modeler manual's branch generator (Gravity "equivalent to a downward
pointing direction force", the force's profile curve along the branch, Length "+ % of parent", Prune > Ground).
Technical: `book_real_time_rendering_4th_akenine` (alpha-textured leaf cards against overdraw), this crate's
`lod.rs`. Pending index: Weber and Penn 1995 (paywalled), Moulia et al. 2022, the WEEP paper, EDIS ENH-734.

**Numbers** (`/tmp/f7`, removed with the scratch).

- Gallery, 526 scenarios on CPU and `--gpu`: 526 PNG = 526 rows, none under 100 KB, no flag, no BLANK_CAPTURE
  or CAPTURE_TIMEOUT, no `ERROR` or `panicked` (one `WARN` under `--gpu`: the known hull-crown note). Against
  main's binary the 522 earlier scenarios: `stats.tsv` columns 1-10 identical for 489; the 33 that moved are 29
  willow ones and the four pipe-model shrub and fern ones; all 488 non-willow scenarios with an unmoved mesh are
  the same image below the HUD (the willow's `grow_willow_1`, a bare young trunk, differs by its taper). Two runs
  of one filter and the full run agree on columns 1-10; CPU and `--gpu` stats agree.
- GPU parity: all 526 mean of means 0.080, worst 2.109 (`smoothyoung_shrub_4`, as before); the seven CI
  scenarios 0.581 and 1.205 before, **0.694 and 1.452 after**: `age_willow_6` 0.664 to 1.452. Not "no worse", and
  said so: with strands off the retuned willow reads 1.29, so most of it is the fuller, larger-leafed crown showing
  the existing difference in how a growing limb's leaves are spread (CPU re-spreads them over the aged length, the
  GPU shows the first ones of the grown branch); the strands add 0.13. `compare_gallery` exits 0 on both sets.
- Forest bench, interleaved base / new, medians of 4, 3 and 3 runs, ms/frame (cpu): pool 10 000 4.97 (14.51) ->
  4.90 (14.45) -1.5%; naive 1 000 35.01 (18.92) -> 34.96 (18.92) -0.1%; gpu-live 1 000, 200 frames 182.46
  (248.1) -> 179.01 (244.9) -1.9%. One pool run of the new binary was in the Mac's fast mode (4.56).
- Build probe (`build_tree`, best of 5): the willow 2.22 -> 4.44 ms mature, 0.43 -> 0.75 ms at age 4; `build_lods`
  3.7 -> 7.4 ms; every other preset's vertex and triangle counts unchanged and its time within noise.
- Looked at: `/tmp/f7/cur/{a..h}` (eight tuning rounds: front and side at 15 m, a 9 m game view, the gallery's four
  views), the age and seed series, seasons, conditions, painterly, toon, the four oaks with strands, pipe willow.
- Tests: 341 lib (334 + 7 new) and 67 doc green; clippy `--all-targets --all-features -D warnings`, `cargo doc
  -D warnings`, `cargo check`, `--features serde`, `--examples`, `cargo package --locked` (131 files, the
  yanked `clipboard-win` note as before). Mutation: re-spreading the strand leaves over the current count fails
  `a_strands_leaves_keep_their_places_as_it_grows`; the unfixed crown box fails the shape test.

**Critique and verdict** (`/tmp/f7/critique.md`): the generator feature (`StrandParams`: deterministic, closed,
off the trunk and the floor, grows with its branch, CPU and GPU agree on its leaves, no other tree moves): GREAT.
The willow's look: **NOT YET**, much better than before and not a tree to ship at 9 to 15 m. Flaws, measured
against the note's "what great looks like": the strands' wood reads as thin pale wires among sparse leaf cards (a
string curtain; the leaf cards are 6 cm wide and 6 a metre, the rope is bare between them); the crown's top is bare
arcs on a slim flat-capped trunk; from 30 m level 1 drops all wood under 1.5 cm, so the curtain is gone and the
tree reads as before; it costs 2.2 times the vertices. Eight tuning rounds (strand density, leaf size and count,
limb angle, branch start, trunk taper; a rising top limb made a plume above the trunk and was dropped) and none
reached GREAT with straight, constant-colour ropes of this vertex budget, so D-052 stays Open with its remaining
flaws and options, and the improved willow ships rather than reverting to the mound. The rule's "revert to the last
GREAT state" has no state to revert to: the old willow was never GREAT.

**Gate gaps, said plainly.** `cargo build`, `cargo build --release` and `cargo build --examples` (default
features) were `cargo check` and `cargo check --examples`, and `cargo package --locked`'s own verification build,
because each would rebuild Bevy in a new feature set (3 to 4 GB) on a disk with 5 GB free; `cargo test
--all-features` ran as `--lib --tests` and `--doc` for the same reason (clippy `--all-targets` compiles every
example). No README clip was re-recorded: no healthy non-willow tree changed, and the release re-records all
eleven (the `grove.gif` and `growth.gif` clips that show the willow will change then). No sibling-repo commit.

**Commits** (`loop/iter-7`): `33ba0d7` Research: weeping strands, `a8ee746` the implementation, its docs and
the QA log.

### Ranked backlog after iteration 7

1. **Release 0.47.0** (breaking: `IndexedTree::condition`, `FloraUniform::withered`, `BranchParams::strands`,
   `NodeKind::Strand`) once v0.46.1 is 6 hours old (04:42 local, Oct 31): re-record the eleven clips (the willow
   in `grove.gif` and `growth.gif` changes), `cargo semver-checks` if installed.
2. **Finish the willow (D-052)**: leaf ribbons (a wider or longer leaf on a strand, so the leaves cover the rope),
   strands kept as lines at level 1, limbs whose leaves reach their tips (`add_distributed_leaves` bunches them at
   the base), a greener strand colour (a per-node wood colour), a forked trunk instead of a flat-capped pole; and
   the cost: coarse levels keep every 3rd and 8th leaf, so a willow's far levels are 2.5 times an old willow's.
3. **Age-stable leaf places for every `AllBranches` node** (CPU = GPU for growing limbs): the strand's rule
   (`leaf i keeps its place`) for all of them. It is the cause of the willow's parity (1.29 without its strands)
   and of the `age_*` residue in every `AllBranches` preset; it moves those presets' young trees, so it needs a
   gallery pass of its own.
4. **A gradual decline** (iteration 6's #2): a class change pops; leaves the new class loses should fall.
5. **D-074 / D-075** on big; a dying tree's wood and shadow; D-067; a CPU mirror of `wind_bend`.

**Clean-up (iteration 7).** Merge `aef29ad` on `main` (pushed, tree identical to `loop/iter-7`, so the gate
stood). No release: v0.46.1 was 3 h 43 m old at the push, the 6-hour rule; 0.47.0 carries iterations 5 to 7. No
issue referenced or closed (none was open). Baseline kept as copies of main's release binaries under `/tmp/f7/base`
(no second worktree or target directory: `git worktree list` shows only the clone). Removed, mine: `/tmp/f7`
(3.1 GB: binaries, galleries, tuning renders, bench output, findings, critique, a PMC page), the throwaway
`examples/zz_probe.rs` (never committed), and in `CARGO_TARGET_DIR` every file this pass wrote (modified after
01:00 today): the release example binaries and `bevy_flora` rlibs, the debug test binaries and rlibs and the
default-feature check metadata of Bevy, the release and debug incremental caches, and `package/bevy_flora-0.46.1*`;
`df -h /` 15 GB free at the start, 5.7 GB before, 15 GB after. Kept: the target directory's dependency builds (all
older than this pass). Leftovers of earlier passes: none found (no `f*_base` in `~` or `/tmp`); the branches
`loop/iter-1` to `-7` remain on origin, not deleted.

## Iteration 8 — 2026-10-31 — Leaves keep their places as a branch grows: CPU growth = GPU growth (D-078)

Host: this Mac (Apple M5, Metal, macOS 26.6.2), not big. Disk 15 GB free at the start, 4.9 GB before the clean-up
(the shared `CARGO_TARGET_DIR` is 79 GB); one build or one render at a time. HomeStill: `system_status` ok (18 669
documents, 488 829 embedded chunks), `scribe_health` ok, `distill_search` and `markdown_read` answered (the
shoot-apex papers and de Reffye et al. 1988 were in the corpus; SpeedTree's growth manual came from the web).
Corpus re-scan of `catalog_recent` (200): nothing on trees, plants, leaves, wind, fire, impostors or levels of
detail that `corpus.md` lacked (one SIGGRAPH winding-number paper), so no `Research: corpus` commit.

**State found.** No open issue on `bevy_flora` (the list was read; `gh` answered), no `consumer` label, no parked
branch (`git branch --list 'wiggum/parked-*'` empty), a clean `main` at `a91e2dc`. bevy_emerge's family issue #102
(F6) is closed for its slices 2 to 6 and owes a clip only (its `forest_view` draws a scattered tree as a
`PooledTree` and a placed one as a `ProceduralTree` with `TreeAge`, the crate's own components): nothing
this crate lacks. v0.46.1 was 3 h 41 m old at the start, so the 6-hour rule gave no release before 04:42.

**Pick.** Iteration 7's backlog item 3, the one in the brief's first category (a GPU-grown tree that does not
match the CPU one, and growth that jumps): under `AllBranches` (11 of the 20 presets) a branch spread the leaves it
had over its length, so each time it gained one every leaf of it moved. Measured with a throwaway probe over every
preset (ages 0 to 20 in steps of 0.1, branches neither new nor grown): a leaf hopped a mean 1 to 2 cm and a p95 of
9 to 18 cm (oak 15, maple 18, up to 36) per step, and a leaf of the CPU tree stood a mean 8 to 14 cm, p95 35 to
58 cm and up to 1.3 m from where `GpuGrowth` puts it (oak cards are 33 cm). Ranked: (1) this, correctness and
both growth paths; (2) release 0.47.0 (blocked until 04:42; it follows the pass); (3) a gradual decline for
`TreeCondition` (a class change pops); (4) finish the willow (D-052, eight tuning rounds already failed); (5)
D-074/D-075 on big. The runner-up, the gradual decline, is a feature a game can fake with two class changes; this
is a fault a game cannot fix from outside.

**What changed** (`docs/research/leaf-places.md`, committed first as `f4ce982`; implementation `43fbb3d`).

- A grown `AllBranches` branch has `N` leaf slots (`distributed_leaf_count` of `Node::full_length`, a new public
  field: the exact grown length, because `length / progress` is it only to a rounding error and a count that
  lands on a whole number fell by one at some ages, which moved every leaf of a branch for that age: found by the
  probe on a pine's leader at 6.3 years, before the field), each slot at `0.3 + 0.7 t^2` of the branch with
  `t = (j + u_j) / N` and draws of its own (size, direction, turn, tint, hue, drawn in slot order, `draw_per_leaf`).
  A branch `n` leaves long shows the `n` slots that appear first, `foliage::leaf_ranks`: the tip's, then the gaps'
  (van der Corput order counted from the tip). A grown branch shows every slot in slot order, so a mature tree
  is byte for byte what it was (all 462 unchanged gallery images, every mature scenario among them; levels of
  detail, pools and impostors are built from the mature list and do not move).
- `GpuGrowth` shows the same leaves with no change to its shaders or its node buffer: `gpu/mesh.rs` tags each
  leaf's vertices with its rank as the ordinal and `state.rs` already wrote `leaf_count = n`. Its doc loses the
  "`AllBranches` leaves keep their mature positions; the CPU path respaces them" difference and gains the one
  that remains (a leaf that faces out from its branch's tip, `LeafOrientation::RadialOutward`, faces out from the
  mature tip on the GPU and from the tip it has now on the CPU).
- **Assumed decision (owner may override)**, in the commit body and `docs/research/leaf-places.md`: the order in
  which leaves appear. Options: base-first, acropetal (the botanical order, what `GpuGrowth` showed; my first
  build, which left thin bare limb ends past the last leaf on young oaks and apples: `s1.png`), tip-first then
  the gaps (taken: young crowns keep the look they had, and mature trees and coarse levels are unchanged), or
  absolute "in place" timing (SpeedTree's: a leaf starts when the branch has grown to where it stands; the
  shader cannot find a point on a bent axis at a fraction it was not measured at). The rank order is a design
  default; the owner's reply wins.
- **Assumed owner exception**, in the commit body and the CHANGELOG: `a_strands_leaves_keep_their_places_as_it_grows`
  asserted that a young strand shows the first leaves of the grown strand; the rule it pins is unchanged and now
  holds for every `AllBranches` branch, but the leaves that show first are the tip's, so it became
  `every_branchs_leaves_keep_their_places_as_it_grows` (the same places, draws and counts, on every preset and
  every age from 30% to 95% of maturity, and at least ten young strands). `grown_leaves_and_flowers_match_the_cpu_meshers`
  no longer skips the `AllBranches` presets (widened, with `RadialOutward` leaves made to face along their branch).
- Breaking: `Node::full_length` (a new public field): 0.47.0.

**Sources.** Botanical: Mirabet, Besnard, Vernoux and Boudaoud, "Noise and robustness in phyllotaxis" (stem
`10.1371_journal.pcbi.1002389`: "Lateral organs (leaves and flowers) are successively initiated as primordia at
the shoot apex yielding a regular arrangement on the stem known as phyllotaxis"); de Reffye, Edelin, Francon,
Jaeger and Puech, "Plant models faithful to botanical structure and development" (stem `10.1145_378456.378505`:
"the leaves and fruits are inserted in the right places on the branches (according to the age of the tree)").
Shipping game: the SpeedTree Modeler manual, "Growth" and "Leaf mesh generator properties" ("In place: The leaf
will start growing exactly when the parent is long enough to reach its starting point in the fully grown
model", "you create the model in the fully grown state and use these properties to control how it gets there",
and Smoothing "to prevent wild leaf motion during growth"). Technical: `book_real_time_rendering_4th_akenine`
(alpha LODs "experienced as much more continuous than the discrete geometry LOD method, and so avoids
popping"), this crate's `gpu/frame.rs` and `growth.rs`. Pending index: Prusinkiewicz and Lindenmayer (not in the
corpus as text), Hallé, Oldeman and Tomlinson (paywalled); SpeedTree's real-time modelers have no growth.

**Numbers** (`/tmp/f8`, removed with the scratch).

- Gallery, 526 scenarios on CPU and `--gpu`: 526 PNG = 526 rows each, none under 100 KB, no flag, no
  BLANK_CAPTURE or CAPTURE_TIMEOUT, no `ERROR` or `panicked` (one `WARN` under `--gpu`: the known hull-crown note).
  Against main's binary: 462 images byte for byte, 64 changed (the young trees of the 11 presets, plus their
  `hull_*` crowns); `stats.tsv` columns 2-8 identical but for the leaf count of `age_apple_8` and
  `pipeyoung_apple_5` (a leaf's fruit is its slot's), the bounding box of 37 young scenarios; CPU and `--gpu`
  columns 1-10 identical for all 526. Two runs of `--filter age_ --filter grow_`: identical columns and all 117 PNGs
  byte for byte, and the same as the full run.
- GPU parity (`compare_gallery`), all 526: mean of means **0.030** (0.080 before), worst **1.186** (`grow_willow_7`;
  2.109, `smoothyoung_shrub_4`, before), 83 PNGs byte-identical (79 of 492 in the README); the seven CI scenarios
  **0.221 and 0.920** against 0.694 and 1.452 measured here with main's binary. The probe's leaf places, CPU against
  GPU, every `AllBranches` preset and age: 0 (was p95 35 to 58 cm, max 1.3 m); a leaf's re-spread hop per step: 0
  (was p95 9 to 18 cm).
- Forest bench, interleaved base / new, medians of three, ms/frame (cpu): pool 10 000 4.96 (14.55) -> 4.98 (14.60)
  +0.4% (+0.3%); naive 1 000 35.05 (19.18) -> 34.94 (18.85) -0.3% (-1.7%); gpu-live 1 000, 200 frames 177.47 (243.35)
  -> 178.82 (243.30) +0.8% (0.0%).
- Build probe (`build_tree`, best of 9, `/tmp/f8/after/build_new.txt` and `before/build_base.txt`): mature and age 4
  within +0 to 3.5% for 19 of 20 presets (oak +3.5%, willow 4.89 -> 5.01 ms) and `dead_tree` 0.020 -> 0.021 ms, a
  microsecond; vertex and triangle counts identical for every preset at both ages.
- Looked at: `/tmp/f8/cmp/{s1..s5,r1..r7}.png` (before | after, front, side and close-up quarters of grow_oak_6,
  age_apple_6, age_maple_6, age_spruce_4, age_willow_6, age_shrub_4, grow_willow_9, extreme_lagthick_6,
  extreme_fastgrow_2, hull_fir_2, hull_spruce_2; hull_spruce_5 was read before the final rule and not after).
- Tests: 343 lib (341 before: one replaced, three added, one widened) and 67 doc green; clippy
  `--all-targets --all-features -D warnings`, `cargo doc -D warnings`, `cargo check`, `--features serde`,
  `--examples`, `--examples --all-features`, `cargo package --locked` (131 files). Mutations that fail the new
  tests: the re-spread (oak branch 15 at 0.4 of maturity, leaf 0 at 0.322 of it, 0.302 grown), the leaf's index as
  its GPU ordinal (oak age 4, node 14), and `length / progress` (progress 0.8004: slot 1 at 0.370, 0.320 grown).

**Critique and verdict** (`/tmp/f8/critique.md`): the leaf places on a growing branch: **GREAT** (CPU against
GPU 0, no hop, parity 0.030 and 1.19, mature trees bit-identical, build and forest cost within 1% and 3.5%).
Flaws found, none stopping a ship: a branch with one leaf carries it at its tip, so a young shrub shows a single
leaf standing above its crown (`age_shrub_4`, the largest change outside the hull scenarios, mean 4.5 of 255); a
leaf that appears still appears at the node's current leaf size, not from nothing (one pop per count increment,
as before: this change ended the re-spread of the others, not the appearance of the new one; a fractional
`leaf_count` in the vertex stage would ease it in: backlog 1); the `RadialOutward` facing difference above; leaves
move outward with their wood while sub-branches hold their attach points (continuous, the shader's limit); the
README clips are re-recorded by the release.

**Gate gaps, said plainly.** `cargo build`, `cargo build --release` and `cargo build --examples` (default
features) were `cargo check` and `cargo check --examples`, and `cargo package --locked`'s own verification build
(it compiles `bevy_flora` alone), because each would rebuild Bevy in a new feature set (3 to 4 GB) on a disk with
5 GB free; `cargo test --all-features` ran as `--lib --tests` and `--doc` for the same reason (clippy
`--all-targets` compiles every example). The release example binaries (`--all-features`) were built and run.

**Commits** (`loop/iter-8`): `f4ce982` Research: leaf places, `43fbb3d` the implementation and its tests, the
docs commit (CHANGELOG, ARCHITECTURE, README parity numbers, the note, D-078).

### Ranked backlog after iteration 8

1. **Release 0.47.0** (breaking: `IndexedTree::condition`, `FloraUniform::withered`, `BranchParams::strands`,
   `NodeKind::Strand`, `Node::full_length`) once v0.46.1 is 6 hours old (04:42 local): re-record the eleven clips
   (`growth.gif` and `grove.gif` change: young leaves and the willow), update the README's frame times and
   compatibility table, `cargo semver-checks` if installed.
2. **A leaf eases in**: a fractional `leaf_count` (`n + the share of the next leaf`) in the vertex stage and in
   `leaf_points_for_node`, so a leaf that appears grows from nothing over the interval until the next, instead of
   appearing at its node's full leaf size (the one pop left in a growing crown).
3. **A gradual decline** (iteration 6's #2): a class change pops; leaves the new class loses should fall.
4. **Finish the willow (D-052)**: leaf ribbons, strands kept as lines at level 1, limbs whose leaves reach their
   tips, a greener strand colour, a forked trunk instead of a flat-capped pole.
5. **`RadialOutward` facing on a growing branch** (the CPU uses the tip it has now, the GPU the mature tip: an
   orientation difference of a few degrees), D-074 / D-075 on big, a dying tree's wood and shadow, D-067, a CPU
   mirror of `wind_bend`.
