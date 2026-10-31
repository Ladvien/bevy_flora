# A leaf eases in: the last pop in a growing crown

The system: `add_distributed_leaves` in `src/foliage.rs` (`FoliagePlacement::AllBranches`: oak, willow, maple,
spruce, pine, japanese maple, shrub, cherry, apple, juniper and fern, 11 of the 20 presets) and the `leaf_count`
that `src/gpu/state.rs` writes for `GpuGrowth` (`src/gpu/frame.rs` and `growth_common.wesl` read it). Iteration 8
(`leaf-places.md`) made every leaf keep its place as its branch lengthens, and logged what it left, in its
critique and as backlog item 2: "a leaf that appears still appears at the node's current leaf size, not from
nothing (one pop per count increment, as before: this change ended the re-spread of the others, not the
appearance of the new one)".

What happens today. A branch of length `L` shows `n = floor(density * L * height_mod)` leaves (at least one).
The branch grows past the length at which `n` becomes `n + 1` and the leaf of rank `n` is in the mesh, at its
slot, at the size of every other leaf of the branch (`leaf_size * variation * node_leaf_scale`). Nothing before
it, then all of it, in one rebuild. Measured on `main` (`0.47.0`) with a throwaway probe (`tests/zz_probe.rs`,
deleted), per `AllBranches` preset: every leaf of every branch keyed by (node id, the leaf's tint draw), a sweep
of `TreeAge` from 0 to maturity in steps of 0.05 year, the leaf's size as a share of its mature size:

| preset | leaves that appear | mean share at first sight | first seen above half size | largest step in one leaf |
|---|---|---|---|---|
| oak | 419 | 0.27 | 25% | 1.00 |
| pine | 299 | 0.28 | 27% | 1.00 |
| willow (strands included) | 4 630 | 0.29 | 28% | 1.00 |
| maple | 528 | 0.30 | 30% | 1.00 |
| spruce | 759 | 0.43 | 43% | 1.00 |
| shrub | 550 | 0.29 | 28% | 1.00 |
| cherry | 460 | 0.33 | 29% | 1.00 |
| apple | 798 | 0.31 | 31% | 1.00 |
| juniper, fern | 176, 125 | 0.31, 0.28 | 30%, 28% | 0.99, 1.00 |
| japanese maple | 58 | 0.08 | 9% | 0.88 |

A leaf appears on average at a third of its size and one in four at more than half; the largest appears at
full size, a 33 cm oak card out of nothing. At the README clip's pace (192 frames over the oak's 14 years of
growth, 0.05 year a frame) an oak pops 1.5 leaves a frame and a willow 16: a sparkle at the edge of every
growing crown. The brief's "growth never jumps".

## Sources

- Botanical (model): "Tree Growth Modelling Constrained by Growth Equations", Computer Graphics Forum (home-still
  stem `10.1111_cgf.13263`); "A phenological timetable of oak growth under experimental drought and air
  warming", PLoS ONE (stem `10.1371_journal.pone.0089724`); "Divergent phenological and leaf gas exchange
  strategies of two competing tree species drive contrasting responses to drought at their altitudinal
  boundary", Tree Physiology (stem `10.1093_treephys_tpy041`, whose methods define when leaf development
  starts and ends).
- Shipping game (middleware): the SpeedTree Modeler manual (Unity), "Leaf mesh generator properties", its
  Growth group, Transitions and Timing
  (<https://docs.unity3d.com/speedtree-modeler/manual/leaf-mesh-generator-properties.html>, read in full on
  2026-10-31); SIGGRAPH 2025 "Advances in Real-Time Rendering in Games", materials talk (stem
  `s2025-advances-materials-3`: a vegetation material is chosen by season, with an alpha channel "with and
  without leaves").
- Technical: Akenine-Moller, Haines and Hoffman, Real-Time Rendering 4th edition
  (`book_real_time_rendering_4th_akenine`, alpha LODs); this crate's `src/gpu/frame.rs` (`placement`, the
  formula `GpuGrowth` places and scales a vertex by, mirrored by `growth_common.wesl`'s `piece_scale`) and
  `src/foliage.rs` (`add_distributed_leaves`).

## Craft principle

A leaf is not full size the moment it exists. Its development has a start and an end, and in between it is
growing:

> "Leaf development was considered to start with budburst and recorded as completed when most leaves and
> needles were mature" (Tree Physiology, `10.1093_treephys_tpy041`)

and the models graphics uses for the growth of a plant's size are S-shaped, slow at the start and at the end:

> "Logistic equation is famous in ecology because it generally describes the increase of size of individuals
> or the increase of populations in a limited environment. ... employing a growth equation not only
> simulates growth with variational speeds equal to a nonlinear speed curve, but also constructs a stop
> criterion of tree growth." (`10.1111_cgf.13263`, "Logistic growth equation")

Oak flushes show the scale: a flush has a growth duration of days to weeks (`10.1371_journal.pone.0089724`:
"drought reduced average growth duration of all flushes by 2 days"), and the leaves of a flush grow through it.
A leaf that goes from not there to its mature size between two frames contradicts all of it.

## How shipping games do it

SpeedTree, the middleware most open-world titles ship their trees with, gives every leaf a growth of its own
and a curve for it. Its real-time modelers ship a fully grown tree (and so did iteration 8), but its
modeler's growth animation, the thing this crate's `TreeAge` is, treats a leaf's size as a function of the
leaf's own growth time:

> "**Size** - This curve controls how the size increases over the course of the leaf's growth time."
> (Leaf mesh generator, Growth, Transitions)

> "**Speed scale** - The speed at which a leaf will grow is set on the Timeline Bar. Use this property to
> scale that value. The profile curve scales the speed relative to the leaf's total growth time. For example,
> ease in and out of the growth animation by using an S curve." (Growth)

and it pins the end of every curve to the grown model:

> "The final value for the profile curve must be 0.0 in order for the model to exactly match its correct
> final position." (Growth, Noise, Wobble)

So a shipped growth tool eases a leaf in with an S curve and promises that the grown tree is the modeled
tree. Both are this change's rules.

## Model adopted

A branch has a real leaf count `c = max(1, density * L * height_mod)` (what `distributed_leaf_count` truncates)
and `N` slots, the leaves of the grown branch, each with a rank (iteration 8: `leaf_ranks`, the order the
leaves appear). The leaf of rank `r` has a **flush** `f_r = clamp(c - r, 0, 1)`: 0 while the branch is shorter
than the length at which the leaf's turn comes, 1 once the branch has grown a leaf-spacing past it. The leaf is
drawn when `f_r > 0`, at `S(f_r)` times its size, `S(f) = f^2 (3 - 2f)` (smoothstep: slow in, slow out, `S(1) =
1` exactly). A leaf is therefore born at nothing, at its slot, on its stem, and reaches its size over exactly
the growth of branch that the next leaf takes to be born: the leaves of one branch are never all in the same
phase. The `n = floor(c)` leaves whose flush is 1 are what the branch showed before; the one change is
the leaf of rank `n`, which shows at `S(c - n)` of its size and is absent when `c` is a whole number.

- The rank-0 leaf has `f = 1` always: the "at least one leaf" rule stands, and a branch's first leaf already
  grows from nothing with its node (`node_leaf_scale`: 0 at `GrowthParams::leaf_threshold`, 1 at progress 1).
  The flush multiplies `node_leaf_scale`, never replaces it.
- A grown branch has `c >= N` and every flush is 1: a mature tree, and any tree whose branches are all grown,
  is bit for bit what it was. Levels of detail, pools and impostors are built from the mature list and do not
  move.
- The flush is a function of the branch's length, which is a function of age: reversible (a scrubbed age
  shows the same tree going back), deterministic, and no clock.
- `GpuGrowth` carries the real `c` in `leaf_count` (it carries `floor(c)` today) and its vertex stage scales a
  leaf by `leaf_scale * S(clamp(leaf_count - ordinal, 0, 1))`, with `ordinal < leaf_count` for shown: the same
  rule as the CPU's, with no new buffer and no change to a vertex's tag. `leaf_count < 0` ("all of them") is
  unchanged.

## Parameters

None added. The ease is one leaf-spacing of branch growth (`1 / (density * height_mod)` metres), shaped by
smoothstep; `FoliageParams::density` and `height_falloff` set it as they set the count, and
`GrowthParams::leaf_threshold` still decides when a node shows leaves at all.

## Validation targets

- **No leaf pops.** Over the same sweep as the table above, at steps of 0.05 and 0.01 year, for every
  `AllBranches` preset: a leaf's share of its mature size at first sight is under 0.15, and no leaf's share
  moves between two steps by more than the slope allows (smoothstep's steepest part is 1.5 times the line, so
  a step of branch growth `d` moves a flush by at most `1.5 * density * height_mod * d`, plus the node's
  own `leaf_scale` slope). A test that fails before: the largest step is 1.00 and a quarter of the leaves
  appear above half size.
- **Mature trees unchanged:** `stats.tsv` columns 1-10 identical for every scenario whose branches are all
  grown, and their PNGs byte for byte.
- **CPU = GPU, size as well as place:** a leaf's scale as `frame.rs` computes it equals the CPU leaf's size over
  its mature size (`<= 1e-4`) at every age of the sweep; `grown_leaves_and_flowers_match_the_cpu_meshers` and
  `compare_gallery` stay as they are: the seven CI scenarios no worse than 0.221 mean and 0.920 worst, the
  full gallery's mean of means no worse than 0.030 and worst 1.186.
- No new mesh flag (a leaf of 3% of its size is not a degenerate triangle); forest bench and build probe within
  5% (the change scales a leaf: the same number of vertices plus at most one leaf per branch).

## Measured (iteration 9)

Same probe, same sweep, after the change (0.05 year steps; 0.01 in brackets): share of mature size at first
sight, mean 0.00 to 0.05 (0.00) for every preset, none above half (was 9% to 43% of the leaves), the
largest 0.07 on the oak, 0.47 on the willow's strands and 0.37 on the fern, whose nodes grow fast ([0.03
and under for all]); the largest step in one leaf 0.25 to 0.33 on the oak, maple, spruce, pine, cherry,
apple and juniper, 0.80 on the willow, 0.52 shrub, 0.59 fern ([0.18 at most, the willow's strands]; it
was 1.00 at 0.05). Mature trees: `stats.tsv` columns 1-10 identical for every scenario at maturity;
CPU against `--gpu` 0.029 mean of 255 / 1.16 worst over the 526 (0.030 / 1.19), the seven CI scenarios
0.218 / 0.922 (0.221 / 0.920); forest bench within 0.5% in all three modes; a CPU build at maturity within
1%, at age 4 up to +7% (the part-grown leaf per branch: +4% to +11% leaf vertices).

## What great looks like

A growth clip in which the crown thickens like a flush of leaves and not like static: at the edge of a young
limb a leaf is a bud, then a small leaf, then a leaf, and no frame shows a leaf that was not there the frame
before. No leaf floats: a small leaf sits on its slot at the wood (C5, detached or embedded leaves, in
`bevy-flora-visual-qa`). A mature tree and a clip's last frame are unchanged.

## Deviations

- The ease is one leaf-spacing of branch growth and smoothstep, a design default: no source gives a leaf's
  expansion curve in tree-growth terms (leaf expansion is weeks of a real year; this crate's year is whatever
  the game's clock makes it), and SpeedTree leaves the curve to the artist ("Size: this curve"). Making it a
  parameter would add a knob no game has asked for; the rule is one line to change if one does.
- No default changes.

## Pending index

- A per-leaf expansion curve for broadleaf trees (leaf area against days from budburst): the corpus has
  phenology timetables (`10.1371_journal.pone.0089724`) and the logistic growth law (`10.1111_cgf.13263`) but not
  a leaf-area growth curve; `paper_search` for "leaf expansion" returns tropical physiology.
- SpeedTree's real-time modelers have no growth; no shipped real-time source for leaf-size timing.
