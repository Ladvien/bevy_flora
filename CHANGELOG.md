# Changelog

All notable changes to `bevy_flora`. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate uses
[Semantic Versioning](https://semver.org/). Before 1.0, a minor version bump
(`0.x`) may break the API.

## [0.1.0] - 2026-10-10

The first release.

### Added

- `TreeParams` grouped by subsystem (`TrunkParams`, `BranchParams`, `SplitParams`, `MeshParams`,
  `FoliageParams`, `GrowthParams`) and 15 `TreePreset`s: oak, pine, willow, birch, palm, cypress,
  bonsai, maple, spruce, poplar, baobab, dragon tree, Japanese maple, dead tree and redwood.
- `build_tree(params, age)`: one deterministic call from parameters and age to CPU meshes (wood and
  leaves) plus the aged skeleton. The same seed always gives the same tree.
- A continuous growth model. Every node has a birth age and a duration, so a tree can be shown at
  any age from a bare seedling to maturity, and leaves never re-roll as it grows.
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

[0.1.0]: https://github.com/Ladvien/bevy_flora/releases/tag/v0.1.0
