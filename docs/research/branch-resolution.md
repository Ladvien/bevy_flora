# Branch resolution: how many sides a branch tube gets

The system: `MeshParams::{adaptive_resolution, resolution_scale, min_radial_segments,
max_radial_segments}` and `radial_segments_for` in `src/mesh/branch.rs`, which every branch tube,
collar and root piece of every level of detail goes through (`skeleton_mesh_with`). Issue #91: a
lower bound above the upper bound panicked in `u32::clamp` (`assertion failed: min <= max`) on
caller data, from Rust code, a `.tree.ron` or a reflection inspector.

## Sources

- Botanical: `10.1038_s41467-017-00995-6`, Eloy, Fournier, Lacointe and Moulia 2017, "Wind loads
  and competition for light sculpt trees into self-similar structures" (taper allometry, area
  conservation); `10.1371_journal.pone.0093535`, Minamino and Tateno 2014, "Tree branching:
  Leonardo da Vinci's rule versus biomechanical models".
- Shipping game: the SpeedTree Modeler manual (Unity), "Branch generator properties", Segments:
  Radial and Dynamic LOD (web, not indexed),
  <https://docs.unity3d.com/speedtree-modeler/manual/branch-generator-properties.html>.
- Technical: `book_level_of_detail_3d_graphics_luebke`, Luebke et al., "Level of Detail for 3D
  Graphics", chapter 4 (fixed-frame-rate LOD selection); the NumPy reference for `numpy.clip`
  (web), <https://numpy.org/doc/stable/reference/generated/numpy.clip.html>; Rust's `Ord::clamp`
  ("Panics if `min > max`").

## Craft principle

A tree's wood spans two orders of magnitude of thickness, so the number of sides that reads as
round on the trunk is waste on a twig, and the count that suits a twig facets the trunk. Leonardo's
rule, as Minamino and Tateno state it: "the sum of the cross-sectional area of all tree branches
above a branching point at any height is equal to the cross-sectional area of the trunk or the
branch immediately below the branching point". Eloy et al. measure the taper that follows: "for
small \( d \), the scatter is important and data do not follow a simple allometric relation; for
intermediate \( d \), the allometric relation \( d \propto \langle \ell^{\beta} \rangle \) with
\( \beta \approx 3/2 \) seems to be reached; for large values of \( d \) corresponding to the
trunk, \( \beta \approx 2/3 \)". With area conserved, a crown of `n` terminal twigs has twigs about
`trunk / sqrt(n)` thick: the oak's 0.3 m trunk ends in 5 mm twigs. Sides chosen from the radius,
between a floor that keeps a twig a tube and a ceiling that keeps a trunk affordable, is the model
`adaptive_resolution` implements.

## How shipping games do it

SpeedTree, the middleware behind most shipped game trees, sets a branch's sides the same way, from
its radius on top of a floor: "Absolute: Sets the minimum number of radial segments used." and
"+ Relative: The number of segments is computed based on the branch radius and this value. This
number is added to the Absolute segments for the final number of radial segments used." Its levels
of detail then cut sides per level: "Radial segments: Scales back the number of radial segments
for each successive LOD." SpeedTree's floor is *added* to the radius term, so its two controls can
never contradict each other; this crate exposes a floor and a ceiling as two independent fields,
which a caller can cross, and must then be told which one holds.

## Model adopted

`segments = round(base_radius * resolution_scale)`, raised to `min_radial_segments`, then capped at
`max_radial_segments`. The cap is applied last, so when a caller's bounds cross, **the upper bound
wins**: every branch gets `max_radial_segments` sides (the tube, collar and root builders still
floor every count at 3, so 0 and 1 stay safe). Whenever `min <= max` this is exactly `clamp`, so no
tree with valid bounds changes.

Why the ceiling and not the issue's suggested reorder (swap the bounds) or the floor:

- The ceiling is the budget. Luebke et al. on fixed-frame-rate selection: with a "target frame
  time of 100 ms, the system never exceeded this threshold". A game that sets
  `max_radial_segments` is capping its wood's vertex cost; dragging the floor past it in an
  inspector must not raise the cost past the cap, which the reorder does (`(20, 16)` reordered is
  16 to 20 sides).
- It is the convention of the numeric libraries a technical artist knows: NumPy documents `clip`
  with crossed bounds as "When a_min is greater than a_max, clip returns an array in which all
  values are equal to a_max", and glam's `clamp` (`max(min).min(max)`, unchecked unless
  `glam_assert`) does the same.
- It honours the issue's own low-poly file, `(mesh: (adaptive_resolution: true,
  max_radial_segments: 3))`: three sides everywhere, as written, where the reorder gives 3 to 4.

## Parameters

Unchanged defaults: `adaptive_resolution` false, `resolution_scale` 16 sides per metre of radius,
`min_radial_segments` 4, `max_radial_segments` 16. No preset sets them.

## Validation targets

- No panic for any pair of bounds: `(20, 16)`, `(4, 3)` (the file's defaults under a `max` of 3)
  and `(u32::MAX, 0)`.
- Crossed bounds build wood vertex for vertex equal to the tree whose floor equals its ceiling (the
  upper bound alone), closed and facing outward.
- Valid bounds: the gallery's `stats.tsv` columns 1-10 are unchanged in every scenario and the
  `oak_adaptive` and `extreme_*` images are byte-identical.
- `build_tree` time per preset within 5% of before (one `min`/`max` pair instead of `clamp`).

## What great looks like

bevy_emerge's tree inspector over a live `.tree.ron`: a designer drags `min_radial_segments` past
`max_radial_segments` and the tree simply stops gaining sides at the cap, closed and lit, the frame
cost never above what the cap allows; nothing panics, nothing is logged per frame, and the same
file builds the same tree in the game.

## Deviations

- Crossed bounds: the upper bound wins (NumPy `clip`, glam `clamp`), where the issue suggested
  reordering them. A design default, recorded on #91 as an assumed decision the owner may
  override.

## Pending index

- The SpeedTree Modeler manual and the NumPy reference are web pages, not in home-still.
