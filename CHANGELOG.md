# Changelog

All notable changes to `bevy_flora`. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate uses
[Semantic Versioning](https://semver.org/). Before 1.0, a minor version bump
(`0.x`) may break the API.

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

[0.2.1]: https://github.com/Ladvien/bevy_flora/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/Ladvien/bevy_flora/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/Ladvien/bevy_flora/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Ladvien/bevy_flora/releases/tag/v0.1.0
