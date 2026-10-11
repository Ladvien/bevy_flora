# Changelog

All notable changes to `bevy_flora`. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate uses
[Semantic Versioning](https://semver.org/). Before 1.0, a minor version bump
(`0.x`) may break the API.

## [0.4.1] - 2026-10-10

Bug fix for 0.4.0; no API changes, and the meshes are bit-identical (all 319 gallery images
byte-identical to 0.4.0).

### Fixed

- `FloraPlugin` panicked at startup (`Requested resource ... does not exist`) when it was added
  before `AssetPlugin` / `DefaultPlugins`, as in `App::new().add_plugins(FloraPlugin)
  .add_plugins(DefaultPlugins)`. 0.3.5 accepted any order and so does 0.4.1. The plugin now
  registers `GrowthMaterial` with the renderer only when the renderer's plugins (and `PbrPlugin`)
  were added first, and otherwise registers the asset types in `Plugin::finish`; trees with
  `GpuGrowth` then grow on the CPU, and one warning says to add the plugin after `DefaultPlugins`
  to use GPU growth. Checked in a fresh project against both orders (CPU fallback and GPU growth
  both render) and by a regression test that fails on 0.4.0.
- `docs/growth.gif`, the README clip, was re-recorded for this release; it comes out
  byte-identical.

## [0.4.0] - 2026-10-10

A new opt-in growth path. Without the new component a tree is exactly as in 0.3.5 (all 319
gallery images byte-identical to 0.3.5, `growth.gif` unchanged).

### Added

- **`GpuGrowth`**, a component that grows a tree in the vertex shader. The mature tree is built
  and uploaded once; each frame the plugin ages the skeleton and writes a few kilobytes of
  per-node state into a storage buffer, and nothing else is uploaded. Main-world cost per frame
  with every tree growing (headless `App`, release build, Apple M-series, median of three runs):

  | 200 growing trees | CPU rebuild | `GpuGrowth` |
  |---|---|---|
  | oak | 17.6 ms | 0.81 ms |
  | willow | 25.4 ms | 0.90 ms |
  | oak, 2 smoothing passes | 69.8 ms | 0.70 ms |

  That is 0.004 ms of CPU per tree per frame against 0.09-0.35 ms. The wood and the leaves use
  `GrowthMaterial` (`StandardMaterial` extended by `GrowthExtension`), the prepass shares the
  vertex shader so shadows follow the growth, and trees can gain or lose `GpuGrowth` at any time.
- `GpuGrowthSupport`, a resource the plugin sets from the device once the renderer is ready:
  `true` where the vertex stage can read a storage buffer, `false` on WebGL2. While it is `false`
  (and before the renderer exists) trees with `GpuGrowth` grow on the CPU. Insert it yourself only
  in a headless test.
- `gallery --gpu`, `capture_growth --gpu` and the `showcase` key `U` render the GPU path; the new
  `compare_gallery` example diffs two render directories (mean difference and share of close pixels
  per PNG, non-zero exit past the limits).

### What differs from the CPU path

Documented on `GpuGrowth`. Over all 319 gallery scenarios the two paths' renders differ by 0.08 of
255 on average (105 PNGs byte-identical; the worst is a willow at 2.2, the worst share of close
pixels 93.0%); the 192 frames of the README clip, grown by age-only updates, differ by 0.07 on
average (worst frame 0.75). The wood (trunk, collars, tubes, bases) matches the CPU mesh to within
0.2 mm at every age when smoothing and adaptive resolution are off (checked vertex by vertex
against the CPU meshers at hundreds of ages on every preset). Smoothing is computed once on the mature mesh and
deformed; side counts stay at the mature count; `AllBranches` leaves keep their mature positions
along a branch and appear in order; tip-cluster leaves keep the mature mirroring; a growing pointed
trunk ends in a flat cap; the `Mesh` assets (picking, physics, culling) are the mature tree;
growth does not feed motion vectors.

### Changed

- `rebuild_trees` skips trees that grow on the GPU and takes the `GpuGrowthSupport` resource;
  `TreeRender` also exists for GPU trees (its mesh handles are the mature meshes). `FloraPlugin`
  adds a `MaterialPlugin` for `GrowthMaterial` when a renderer exists, so add it after
  `DefaultPlugins`.
- The `bevy` dependency enables `bevy_log` (for one warning when a device lacks storage buffers).
- `docs/growth.gif`, the README clip, was re-recorded for this release; it comes out
  byte-identical.

## [0.3.5] - 2026-10-10

Performance only; no API changes, and the meshes are bit-identical (all 319 gallery images
byte-identical to 0.3.4). The target is a growing tree, which rebuilds both meshes every frame.

### Changed

- A growing tree no longer regenerates its full-grown skeleton every frame. The skeleton is kept
  and regenerated only when `ProceduralTree` changes; a frame that only advances `TreeAge` ages
  the kept one.
- A growing tree leaves its two materials alone unless a colour changed. Writing them every frame
  made Bevy re-prepare both on the render thread each frame.
- Wood pieces (tubes, collars, primary bases, root domes) are emitted straight into the tree's
  mesh instead of into a temporary mesh that was then copied in.
- Measured on a headless `App` with 20 trees growing at once (release build, Apple M-series,
  median of three runs, milliseconds per frame): oak 5.4 to 2.0, willow 8.8 to 3.4, oak with 2
  smoothing passes 14.5 to 10.9. Building the unsmoothed wood of one growing oak went from
  0.135 ms to 0.044 ms, the willow's from 0.228 ms to 0.078 ms.
- `docs/growth.gif`, the README clip, was re-recorded for this release; it comes out
  byte-identical.

## [0.3.4] - 2026-10-10

Performance only; no API changes, and the meshes are bit-identical (all 319 gallery images
byte-identical to 0.3.3).

### Changed

- Smoothing (`MeshParams::smooth_iterations`) is 3-5 times faster: the willow with 2 passes builds
  in 3.0 ms instead of 14.7 ms, the oak in 1.6 ms instead of 4.7 ms (release build, Apple
  M-series). Vertex welding probes one grid cell instead of 27 with a cheap hasher, adjacency is a
  flat array built from one integer sort, smoothing passes reuse one buffer, and the "keep this
  vertex put" spheres are bucketed on a grid instead of tested one by one for every vertex. The
  15 presets with smoothing on build in 16 ms together instead of 55 ms.
- `docs/growth.gif`, the README clip, was re-recorded for this release; it comes out
  byte-identical.

## [0.3.3] - 2026-10-10

Bug fixes found by a parameter sweep; no API changes.

### Fixed

- With `MeshParams::pipe_radius_enabled`, a growing tree's just-born branches (1 mm long) were
  floored to the full `pipe_radius_min` radius: wide, paper-thin discs whose faces folded inside
  out (oak and willow at intermediate ages). The floor now grows with the branch's radius.
  Mature trees are unchanged.
- A sub-branch that `keep_out_of_trunk` shortened to nothing (its parent runs inside a wobbly or
  steep trunk) still grew leaves, which sat inside the trunk. Branches without wood carry no
  leaves now; the default presets are unchanged.

### Changed

- The `gallery` example renders 319 scenarios: pipe radii at age 5 (`pipeyoung_<preset>_5`) and a
  wobbly, twisted trunk (`wobble_<preset>`) on every preset.
- `docs/growth.gif`, the README clip, was re-recorded for this release; the oak it shows is not
  touched by these changes, so the clip comes out byte-identical.

## [0.3.2] - 2026-10-10

Appearance only; no API changes.

### Changed

- The willow weeps in strands instead of sitting as a leafy mound: its twigs are fewer, longer and
  thinner (`branches.sub_branch_count` 3 -> 2, `sub_branch_scale` 0.7 -> 0.9, `length` 0.8 -> 0.7),
  and its leaves follow them (`foliage.orientation` `RandomUpward` -> `FollowBranch`, `density`
  6 -> 10), so they hang in rows from the arching limbs. 151 nodes instead of 401, and 18,500
  wood vertices instead of 48,000 for the mature tree.
- The closed, rounded base of a branch that hugs the trunk is rounder: four rings and a cap with
  twice the branch's side count, joined to the branch's own base ring, instead of three rings with
  the branch's side count. On a hexagonal cypress or poplar branch the knot is no longer a
  six-sided pebble. The wood has more vertices as a result (the mature cypress about 8,000
  instead of 5,800, the poplar 10,300 instead of 8,200, the birch 2,900 instead of 2,500).
- `docs/growth.gif`, the README clip, was re-recorded for this release; the oak it shows is not
  touched by these changes, so the clip comes out byte-identical.

### Fixed

- A sub-branch's wood could end up inside the trunk after it was seated on a bent parent and
  bent by gravity (found by the longer willow twigs). Its bend is now relaxed, or the branch is
  shortened, until it clears the trunk.

## [0.3.1] - 2026-10-10

Appearance only; no API changes.

### Changed

- A primary branch that hugs the trunk (less than half of its direction pointing away from it:
  steep upward or downward branches on the birch, cypress, poplar and some extremes) now ends in
  a closed, rounded base that sits half in the trunk, instead of a flat, sheared cut toward the
  trunk axis. Branches that leave the trunk at a wider angle are unchanged.
- The willow's limbs now start all the way up to the top of the trunk (`branches.end` 0.9 -> 1.0),
  so the crown drapes from the trunk top instead of sitting low under a bare end (401 nodes instead
  of 321).
- The spruce grows slower, so a young spruce is a sapling instead of a 5 m pole of hair-thin
  2 m branches: `growth.trunk_fraction` 0.4 -> 0.6, `branch_fraction` 0.35 -> 0.6,
  `thickening_lag` 2 -> 1.2 and `leaf_threshold` 0.5 -> 0.15. The mature spruce is unchanged; it is
  fully grown at about age 16 instead of 12.
- `docs/growth.gif`, the README clip, was re-recorded for this release; the oak it shows is not
  touched by these changes, so the clip comes out byte-identical.

## [0.3.0] - 2026-10-10

Thicker twigs, smoother drooping branches and a weeping willow. `BranchParams` gains a field, hence
the minor bump.

### Added

- `BranchParams::min_twig_radius` (metres, default 0.005; `0` turns it off). No branch is thinner
  than this at its base, or thinner than half of it at its tip, so twigs taper but no longer shrink
  to the 0.4 mm hairlines (0.0 mm on the willow) the radius ratios used to give them, and a twig is
  never thicker than the wood it grows from. The willow uses 3 mm and the bonsai and Japanese maple
  2.5 mm. Gallery close-ups show twigs as sticks instead of wires; at full-tree distance they are
  still thin lines.
- `Node::axis_points()` and `skeleton::MAX_AXIS_POINTS`: the number of mesh rings a branch's wood
  gets.

### Changed

- Drooping branches are meshed on more rings: `BENT_AXIS_POINTS` (6) is now the minimum, and a
  branch that gravity turns more than about 4.6 degrees between two rings gets more, up to 16, so
  long arching limbs are smooth instead of faceted. Wood, children, leaves and the floor test all
  use the same count. The willow's wood grows from about 10k to 38k vertices; presets with mild
  droop barely change.
- Sub-branches leave a drooping parent along its bent tangent at the attachment point, not along
  the direction the parent left the trunk with, so twigs hang from arching limbs. This shifts
  sub-branch directions slightly on every preset with `gravity_strength > 0`.
- The willow preset weeps: `branches.start` 0.4 -> 0.5, `density` 2 -> 4, `length` 0.7 -> 0.8,
  `up_attraction` -0.75 -> -0.5, `gravity_strength` 0.5 -> 2.5 and `stiffness` 0.2 -> 0.1. It has 8
  primaries instead of 5, branch tips hang nearly straight down, and the crown is a wide skirt that
  reaches the ground.
- `BranchParams` literals need the new field: end them with `..Default::default()`. Serialized
  params from 0.2.x (the `serde` feature) lack `min_twig_radius` and no longer deserialize as they
  are.
- `docs/growth.gif`, the README clip, is re-recorded for this release.

## [0.2.2] - 2026-10-10

Appearance and docs only; no API changes.

### Changed

- The poplar preset is now a leafy column. It had 14 short, nearly vertical branches hugging a 12 m
  trunk, so it read as a bare pole with about 70 leaves and a 2.4 m bare leader above them; it now
  has about 125 nodes and about 800 leaves in a narrow crown that starts near the ground and
  reaches the tip. `branches.start` 0.2 -> 0.12, `density` 1.6 -> 2.5, `length` 0.3 -> 0.35,
  `angle` 15 -> 25, `up_attraction` 0.92 -> 0.6, `sub_branch_count` 2 -> 4, `foliage.density`
  3.5 -> 10, `trunk.leader_length` 0.2 -> 0.12 and `leader_has_branches` false -> true.
- `docs/growth.gif`, the README clip, is re-recorded. It still showed the square leaves of 0.1.0.
  Re-recording it is now part of every release (see `AGENTS.md`).

## [0.2.1] - 2026-10-10

Appearance only; no API changes.

### Changed

- `FoliageParams::default().color_variation` is now 0.3 (was 0.15), so the per-leaf brightness
  differences added in 0.2.0 are visible on the dark leaf greens. Set it back to 0.15 to keep the
  0.2.0 look.
- `leaf_size` is 1.3 times larger on the nine presets that use leaf-shaped cards, to make up for
  cards covering about half the area of the old squares: oak 0.33, willow 0.26, birch 0.26,
  palm 1.05, bonsai 0.33, maple 0.39, poplar 0.29, dragon tree 0.46 and Japanese maple 0.23.
  Needle (pine, spruce, redwood) and octahedron (cypress, baobab) presets are unchanged.

## [0.2.0] - 2026-10-10

A leaf look pass, done in the mesh with no custom shader. The API gains public fields and
`foliage_mesh` changes signature, hence the minor bump.

### Added

- Leaf-shaped cards: `CrossedPlanes`, `SingleQuad` and `StarBurst` now emit a pointed oval instead
  of a square, with `CrossedPlanes` and `SingleQuad` seating the card's base on the leaf point so
  leaves grow out of their twigs.
- Crown-bent normals: `FoliageParams::normal_bend` (default 0.8; 0 = the old flat cards) bends
  every leaf normal toward pointing out of the crown, so crowns shade as a soft volume.
- Per-leaf tint: `FoliageParams::color_variation` (default 0.15, 0-0.5) scales each leaf's colour
  by a stable per-leaf factor, stored as a vertex colour. `LeafPoint::tint` holds the draw; it
  comes from its own random stream, so leaf positions, sizes and rotations are unchanged and
  nothing re-rolls as the tree ages.
- `MeshData::colors` (linear RGBA vertex colours, empty for none). `MeshData::extend` keeps it
  aligned with the positions and `into_bevy_mesh` adds `Mesh::ATTRIBUTE_COLOR` when it is set.
- Gallery scenarios `extreme_flatleaves` (the old flat look), `extreme_leafvary`,
  `smoothyoung_<preset>_4` and `pipe_<preset>`; the gallery now renders 289 scenarios.

### Changed

- `foliage_mesh(points, &FoliageParams)` replaces `foliage_mesh(points, style)`, because it now
  needs the normal bend and the colour variation as well as the style.
- `LeafPoint` has a new `tint` field, and `FoliageParams` has `normal_bend` and
  `color_variation`; build them with `..Default::default()` or `..FoliageParams::default()`.
- A leaf-shaped card covers about half the area of the square it replaces, so crowns are airier
  at the same `leaf_size`. `FoliageParams::leaf_size` is now the leaf's length from base to tip.
- The gallery's `INSIDE_OUT` flag counts only the wood, because bent foliage normals deliberately
  disagree with face winding; foliage winding is covered by a unit test.

## [0.1.1] - 2026-10-10

Documentation only; no API or behaviour changes.

### Fixed

- `MeshParams::smooth_iterations` described plain Laplacian smoothing; it now documents the
  volume-preserving (Taubin) smoothing the crate does and what it keeps in place.
- `FoliagePlacement::TerminalBranches` and `TipClusters` now state that "terminal" means terminal
  in the full-grown tree, which is why young branches that will grow twigs stay leafless.
- `ARCHITECTURE.md` explains how aged skeletons keep full-grown child counts.

## [0.1.0] - 2026-10-10

The first release.

### Added

- `TreeParams` grouped by subsystem (`TrunkParams`, `BranchParams`, `SplitParams`, `MeshParams`,
  `FoliageParams`, `GrowthParams`) and 15 `TreePreset`s: oak, pine, willow, birch, palm, cypress,
  bonsai, maple, spruce, poplar, baobab, dragon tree, Japanese maple, dead tree and redwood.
- `build_tree(params, age)`: one deterministic call from parameters and age to CPU meshes (wood and
  leaves) plus the aged skeleton. The same seed always gives the same tree.
- A continuous growth model. Every node has a birth age and a duration, so a tree can be shown at
  any age from a bare seedling to maturity; leaves never re-roll or disappear as it grows.
- `FloraPlugin` with the `ProceduralTree`, `TreeAge` and `Growing` components. It rebuilds the
  meshes when parameters or age change and reports each rebuild with a `TreeGenerated` message.
- Branch shape: gravity bend scaled by each branch's lever arm, Y-splits, recursive sub-branches,
  crown envelopes, phyllotaxis, twist, floor avoidance that holds for bent wood.
- Wood mesh: tapered, wobbling, twisted trunk with root flares; flat, pointed or leader-branch
  trunk tops; branch collars and root segments, so every junction is closed; optional pipe-model
  radii, adaptive ring resolution and volume-preserving (Taubin) smoothing.
- Leaves: five styles (crossed planes, single quad, cluster sphere, star burst, needle cluster),
  three placements (terminal branches, all branches, tip clusters) and four orientations, with
  translucent leaf material.
- Optional `serde` feature for `Serialize`/`Deserialize` on every parameter type.
- Examples: `showcase` (interactive, with growth and preset cycling), `gallery` (offscreen
  four-view renders and per-scenario statistics for visual QA) and `capture_growth` (the README
  clip).

[0.4.1]: https://github.com/Ladvien/bevy_flora/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/Ladvien/bevy_flora/compare/v0.3.5...v0.4.0
[0.3.5]: https://github.com/Ladvien/bevy_flora/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/Ladvien/bevy_flora/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/Ladvien/bevy_flora/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/Ladvien/bevy_flora/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/Ladvien/bevy_flora/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/Ladvien/bevy_flora/compare/v0.2.2...v0.3.0
[0.2.2]: https://github.com/Ladvien/bevy_flora/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/Ladvien/bevy_flora/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/Ladvien/bevy_flora/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/Ladvien/bevy_flora/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Ladvien/bevy_flora/releases/tag/v0.1.0
