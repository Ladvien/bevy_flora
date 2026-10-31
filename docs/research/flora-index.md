# FloraIndex: queries with an unbounded reach

Iteration 3, 2026-10-30. Issue #92: `nearest`, `within`, `obstacles_within` and `cast_ray` never return
for `f32::INFINITY` or `f32::MAX`. The note was written after the fix (the code came first because the
issue carries its own failing test); the fix follows it.

## Sources

- Model: Clark and Evans 1979, "Generalization of a Nearest Neighbor Measure of Dispersion for Use in K
  Dimensions" (doi `10.2307/1937660`; the 1954 paper it generalises, doi `10.2307/1931034`, is paywalled
  and not in the corpus). The nearest-neighbour distance of a plant population.
- Shipping-game and industry practice: Ericson, "Real-Time Collision Detection", chapter 7 (uniform
  grids: dense, sparse and hashed storage), home-still stem `book_real_time_collision_detection_ericson`.
  Fluids and particle engines use the same sparse hashed grid: Ihmsen et al., "SPH Fluids in Computer
  Graphics", stem `eth-cgl-sim_anim-Sol14a`.
- Technical: Laine and Karras 2010, "Efficient Sparse Voxel Octrees", stem `10.1145_1730804.1730814`,
  for the regular-grid traversal of Amanatides and Woo 1987 (the paper itself is not indexed).

## Craft principle

A forest is a point population, and the distance from a point to the nearest member of it has no upper
bound: from inside a stand it is a few metres, from a clearing or the edge of the map it is as far as the
stand is. Clark and Evans measure it as a property of the population: "the ratio of the mean distance
observed in a population of known density to the mean distance expected under the assumption of random
dispersion". A woodcutter asking for "the nearest standing tree, wherever it is" is asking a question with
an answer for every point, so the query must not be priced by how far that answer might be, only by how
many trees there are.

## How shipping games do it

Ericson's grids hold only what is in them: "Up to roughly 1,000 objects and a grid size of 100 x 100, the
implicit bit grid uses less memory than the other two representations" (the dense array and the sparse
array of linked lists), and for a world far larger than the objects in it a hashed grid, whose cells exist
only where something is, with "a distance test ... performed for all vertices within a tested cell". The
particle literature says the same of the hashed grid's occupancy: "for SPH fluids, the hash table is
generally sparsely filled". `FloraIndex` is already that structure (a `HashMap` of occupied cells); the bug
is that its queries walked the *query's* rectangle of cells, empty or not, instead of the structure's
occupied ones.

## Model adopted

Laine and Karras on the walk `cast_ray` uses: "Amanatides and Woo [1987] were the first to present the
regular grid traversal algorithm that is the basis of most derivative work, including ours. The idea is to
compute the t values of the next subdivision planes along each axis and choose the smallest one in every
iteration to determine the direction for the next step." The algorithm is defined inside a grid of finite
extent and ends where the ray leaves it; `cast_ray` stopped only at its `max`, so with no hit and an
infinite (or, for f32, a larger-than-2^27 m) `max` it walked on, and past 1.3e8 its `t_next += t_step`
stalled in f32.

- The index keeps the box of cells it has held (`extent`, `obstacle_extent`; they only grow).
- A rectangle query is cut to that box. Where the cut box still has more cells than the index has
  occupied ones, the occupied ones inside it are walked instead, sorted row by row so the answer comes in
  the order it always did (the dedup rule, "a tree in several cells is met once", is unchanged: every
  tree's first cell lies inside the box, so clipping the low corner changes nothing).
- `cast_ray` clips its ground track to the box (slab test, in distance along the ray) and ends at the
  clipped exit, or at `max` if that is nearer.
- `nearest` stays `within` + minimum: an unbounded `nearest` looks at every tree once (O(n)), which is
  what the question costs without a search ring; the README says so.

## Parameters

`FloraIndex::CELL` stays 8 m (a design default, `index.rs`). No new parameter.

## Validation targets

- An unbounded `nearest`, `within`, `obstacles_within` and `cast_ray` (miss or hit, `f32::INFINITY`,
  `f32::MAX`) return, with one tree and with 60 spread over 6 km.
- A finite bound returns the same trees in the same order as before: against brute force for 8 radii from
  5 m to infinity and three query points, and the existing order tests.
- The README's "100 000 pooled oaks: `nearest` about 1.6 us, `cast_ray` over 50 m about 0.6 us" does not
  get worse (the finite path gains two comparisons).

## What great looks like

A game calls `index.nearest(woodcutter, f32::INFINITY)` and `cast_ray(ray, f32::INFINITY)` in an update
system without thinking about the bound, in a forest of one tree or of a hundred thousand, and the frame
does not notice.

## Deviations

None: no default changed. The unbounded query is new behaviour for inputs that never returned.

## Pending index

- Clark and Evans 1954, doi `10.2307/1931034` (paywalled; `paper_download` is not attempted from the
  loop).
- Amanatides and Woo 1987, "A Fast Voxel Traversal Algorithm for Ray Tracing" (Eurographics): not in the
  corpus, cited through Laine and Karras.
