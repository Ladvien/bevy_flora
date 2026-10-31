# Weeping strands: a willow whose branches hang to the ground (D-052)

The system: `BranchParams::strands` (`StrandParams`), a step of `generate_skeleton` in `src/skeleton.rs`
that hangs slender twigs from the outer part of every branch, and the `Willow` preset that uses it.
D-052 has been open since iteration 29 of the original loop: the willow reads as a low mound on a bare
post (`mature_willow`, `qa/visual_findings.md`), eight preset variants (sub-branch counts, scale,
recursion, position bias, leaf density, length) all gave the mound, so it is a generator feature, not a
preset value. Iteration 7 of this loop looked at `mature_willow` and `inspect --preset willow` and read
the same thing: arches of bare wood on a flat-capped pole with a thin scatter of leaves, no curtain.

## Sources

- Botanical (model): "Gravity sensing and responses in the
  coordination of the shoot gravitropic setpoint angle", doi `10.1111/nph.18474` (web, read at
  <https://pmc.ncbi.nlm.nih.gov/articles/PMC9828789/>; the paper cites Moulia et al. 2022, "The shaping of
  plant axes and crowns through tropisms and elasticity", doi `10.1111/nph.17913`, paywalled here). Habit of
  the species: Missouri Botanical Garden, Plant Finder, *Salix babylonica*
  (<https://www.missouribotanicalgarden.org/PlantFinder/PlantFinderDetails.aspx?kempercode=c148>); URI
  Landscape Horticulture, "weeping willow" identification sheet
  (<https://web.uri.edu/wp-content/uploads/sites/2078/Salix-babylonica.pdf>).
- Shipping game (middleware): the SpeedTree Modeler manual (Unity), "Branch generator properties", Forces,
  Shape and Prune
  (<https://docs.unity3d.com/speedtree-modeler/manual/branch-generator-properties.html>).
- Technical: `book_real_time_rendering_4th_akenine`, Akenine-Moller, Haines and Hoffman, Real-Time
  Rendering, 4th edition (vegetation as alpha-textured cards, overdraw against geometry); this crate's own
  `lod.rs` (`min_radius` drops thin wood and its descendants at the far levels).

## Craft principle

A weeping tree is not a tree with more gravity. A lateral branch takes its direction from a setpoint
against the gravity vector, and a weeping one has that setpoint pointing down:

> "Stems grow vertically upward, whereas lateral organs, such as the lateral branches in shoots, tend to
> grow at a specific angle according to a gravity vector known as the gravitropic setpoint angle (GSA)."

> "Loss of a highly conserved sterile alpha motif domain gene (WEEP) results in pendulous branch growth in
> peach trees." (the title of Hollender et al., cited by the same paper)

The habit is a property of the shoots, not of the limbs: Missouri Botanical Garden describes the willow as
"a stout trunk topped by a graceful broad-rounded crown of branches that sweep downward to the ground. It
grows to 30-50' (sometimes to 60') tall and as wide", and the URI sheet names the part that hangs: "Twig:
Very slender, smooth, olive-green to pale yellowish brown, hanging or drooping for long distances, almost
rope-like", "Form: Easily identified due to their long, graceful branches that sweep towards the ground."
The crate's limbs already arch and droop (`gravity_strength`, `up_attraction`); what is missing is the
long, thin, rope-like twig that keeps hanging past the arch, because every node here is one segment whose
length is a share of its parent's and whose bend is capped, so nothing falls the whole way to the ground
(D-052: the crown is "about two thirds of the trunk's height").

## How shipping games do it

SpeedTree models a weeping crown as a chain of branch generators, the last of which is a twig under a
gravity force whose profile is a curve along the branch, and which can be pruned against the ground. From the
Modeler manual:

> "Gravity | Positive values pull branches down, negative values push branches up (in world space). Gravity
> is equivalent to a downward pointing direction force."

> "Forces ... The value set here is multiplied by the strength of the force before being applied to the
> branch. The profile curve controls how much force is applied along the branch."

> "Length | + % of parent: A length equal to this value multiplied by the length of the parent is added to the
> Absolute value above to set the length of the branch."

> "Prune ... Ground: Removes any branch that intersects the ground plane."

So the middleware's willow is: many child twigs per branch, each a length relative to its parent, hung by a
downward force, and cut where they would meet the ground. Real-Time Rendering says what such a crown costs
when it is drawn: "To avoid overdraw, professional packages such as SpeedTree represent and simplify a
model with large meshes of alpha-textured sets of leaves and limbs. While geometry processing then takes
more time, this is more than offset by lower overdraw costs." A curtain is the case where that matters:
hundreds of thin strands, each carrying a row of leaf cards, are far cheaper in fill than the same leaf area as
one large alpha card, and its wood is the part that the levels of detail can drop (`lod.rs` removes wood under
`min_radius` with its descendants, so a strand is the first thing a far level loses).

## Model adopted

`StrandParams` (a field of `BranchParams`, off by default, on for the willow). After the skeleton is built and
seated (every branch on its parent's bent axis, above the floor), each branch gets
`density * length * (1 - start)` strands, drawn from their own random stream keyed by the branch's id
(`SeededRng::from_parts(seed ^ STRAND_SALT, id)`, so a tree without strands draws nothing, and one with them
keeps every other branch where it was). A strand is a `NodeKind::Sub` node one level below its parent:

- it leaves the parent's bent axis at a fraction `t` in `[start, 1]` (`attach_t`, so `Skeleton::aged` and the
  growth model place it as any sub-branch: born when the parent reaches `t`, grown for 0.8 of the parent's
  duration);
- its direction is straight down within `spread` degrees (the setpoint angle of a weeping shoot is past
  horizontal), turned by the node's gravity bend as every branch is, so a strand that leaves slanted curves
  plumb;
- its length is `length` times the trunk height times a draw in `[0.5, 1]`, and where `floor_avoidance` is on
  it ends `clearance` metres above `floor_level` (SpeedTree's Prune > Ground, but a trim, not a removal: a
  strand that would reach the ground is shortened to stop above it);
- its radius is the twig floor (`min_twig_radius` at the base, half at the tip, never thicker than the wood it
  grows from): a strand is a rope, not a branch.

Leaves, wind, levels of detail, felling, fire and `GpuGrowth` need nothing new: a strand is a node, and every
consumer already reads nodes.

## Parameters

| Field | Default | Willow | Meaning |
|---|---|---|---|
| `strands.density` | 0.0 (off) | tuned in the gallery | strands per metre of branch, hung along its outer part |
| `strands.start` | 0.4 | 0.35 | where on a branch they begin, a share of its length |
| `strands.length` | 0.5 | tuned | the longest a strand hangs, a share of the trunk's height |
| `strands.spread` | 12.0 | tuned | degrees off plumb |
| `strands.clearance` | 0.15 | tuned | metres above `floor_level` a strand stops (floor avoidance on) |

## Validation targets

1. A tree with `strands.density == 0` is bit-identical to 0.46.1: the other 19 presets' `stats.tsv` columns
   1-10 do not move, for every scenario that does not name the willow.
2. Every strand hangs: its tip is lower than its start, within `spread` plus the bend's turn of plumb.
3. No strand ends below `floor_level + clearance - tip radius` with floor avoidance on; none starts off its
   parent's wood (the junction check of the existing tests: its start is on the parent's axis at `attach_t`).
4. D-052's own acceptance, on `mature_willow` and `seed_willow_1`: the lowest leaf vertex is below 30% of the
   tree's height, and the crown is wider in its bottom third than in its middle.
5. Same seed, same tree (two builds identical); at age 4 and 8 the strands exist only once their parent has
   reached `attach_t`, so growth does not jump.
6. Cost: willow `build_tree` time and vertex counts before and after (the strands are new wood and new
   leaves, a deliberate increase, quoted); every other preset unchanged; the forest bench's three modes
   within 5% (the pools in it are oaks, pines and maples).
7. CPU and GPU growth agree (`compare_gallery` on `age_willow_6`), no mesh flag, no hole at a strand's root.

## What great looks like

A weeping willow in one glance: a short stout trunk, a broad dome as wide as the tree is tall, and from the
rim of the dome and from every arch a hanging screen of long thin strands that stops a hand's breadth above
the grass, so that the tree reads as a column of pale green fringe with the dome above it, and light comes
through the fringe in lines. The strands are not parallel combs: each is a little off plumb, a little curved,
of its own length, and the wind swings them as one (the shaders already bend a low vertex far from the trunk
more, `wind.md`).

## Deviations

- The defaults of `StrandParams` and the willow's values are design defaults (`qa/visual_findings.md`,
  `## Proposed`), chosen against the validation targets above and the species' proportions (crown as wide
  as the tree is tall, branches sweeping to the ground); no paper gives a strand density.
- A strand is shortened to stop above the ground rather than removed (SpeedTree's Prune > Ground removes
  the branch): a curtain with holes where limbs are low reads worse than one with a ragged hem.
- The willow preset's own limb values (`presets.rs: willow`) may change where the first build shows a
  limb problem the strands do not fix; each change is recorded under `## Proposed` with `old -> new`.

## Pending index

- Weber and Penn 1995, "Creation and rendering of realistic trees" (doi `10.1145/218380.218427`), whose
  parameter table has a weeping willow (negative attraction up): not in the corpus, paywalled; not read.
- Moulia et al. 2022 (`10.1111/nph.17913`): paywalled, read only through the citing paper's summary.
- Hollender et al., the WEEP paper on weeping peach: known only by the title the citing paper lists, not read.
- The EDIS fact sheet ENH-734 (`journals.flvc.org/edis/article/download/142822/148196`) for the willow's
  proportions: not fetched.
