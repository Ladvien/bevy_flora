# Changelog

All notable changes to `bevy_flora`. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate uses
[Semantic Versioning](https://semver.org/). Before 1.0, a minor version bump
(`0.x`) may break the API.

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

[0.3.2]: https://github.com/Ladvien/bevy_flora/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/Ladvien/bevy_flora/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/Ladvien/bevy_flora/compare/v0.2.2...v0.3.0
[0.2.2]: https://github.com/Ladvien/bevy_flora/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/Ladvien/bevy_flora/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/Ladvien/bevy_flora/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/Ladvien/bevy_flora/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Ladvien/bevy_flora/releases/tag/v0.1.0
