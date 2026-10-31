# Leaf places on a growing branch: every leaf keeps its place (CPU growth = GPU growth)

The system: `add_distributed_leaves` in `src/foliage.rs` (`FoliagePlacement::AllBranches`: oak, willow, maple,
spruce, pine, japanese maple, shrub, cherry, apple, juniper and fern, 11 of the 20 presets) and the
`leaf_count` that `src/gpu/state.rs` writes for `GpuGrowth`. Iteration 7 found it while tuning the willow
(`age_willow_6` read 0.66 to 1.29 of 255 of difference between the CPU tree and the GPU-grown one once the
crown was fuller) and logged it as backlog item 3: "age-stable leaf places for every `AllBranches` node".
`StrandParams` already follows the rule (iteration 7), every other branch does not.

What happens today. A branch of length `L` carries `n = floor(density * L * height_mod)` leaves (at least one)
and spreads them over itself: leaf `i` of `n` sits at `along = 0.3 + t * t * 0.7`, `t = (i + u_i) / n`. The
branch grows, `n` goes up by one, and **every** leaf of it moves to a new `t`. The GPU-grown tree shows the
first `n` leaves of the *mature* branch at their mature `t = (i + u_i) / N`. Measured with a throwaway probe
over `TreePreset::ALL` (`tests/zz_probe.rs`, deleted; ages 0 to 20 in steps of 0.1, every branch that is
neither new nor grown; `along` is the leaf's place along its branch times the branch's length):

| preset | CPU leaf moves per 0.1-year step: mean / p95 / max | CPU leaf against the GPU's: mean / p95 / max |
|---|---|---|
| oak | 1.8 / 14.8 / 27.5 cm | 8.0 / 34.7 / 92.5 cm |
| pine | 2.0 / 16.5 / 33.1 cm | 10.1 / 42.8 / 91.0 cm |
| maple | 2.0 / 17.5 / 35.6 cm | 9.9 / 41.5 / 100.5 cm |
| spruce | 1.5 / 13.4 / 30.1 cm | 13.7 / 57.6 / 130.9 cm |
| willow (strands are exempt already) | 1.0 / 8.7 / 29.1 cm | 3.4 / 22.6 / 59.6 cm |
| cherry, apple | 1.2 / 10.4 / 24.4 cm, 1.3 / 10.7 / 24.6 cm | 5.6 / 21.7 / 56.3 cm, 5.6 / 20.7 / 65.0 cm |

Oak leaf cards are 33 cm. A leaf that hops 15 cm each time its branch gains a leaf, in a tree whose age
changes every frame (`rebuild_trees` rebuilds the CPU tree whenever `TreeAge` changes), shimmers; and the
same tree grown on the GPU puts a leaf up to a metre from where the CPU tree puts it. The brief's two
failures, "growth never jumps" and "a GPU-grown tree that does not match the CPU one".

## Sources

- Botanical (model): Mirabet, Besnard, Vernoux and Boudaoud, "Noise and robustness in phyllotaxis", PLoS
  Computational Biology 8(2): e1002389 (doi `10.1371/journal.pcbi.1002389`, home-still stem
  `10.1371_journal.pcbi.1002389`); de Reffye, Edelin, Francon, Jaeger and Puech, "Plant
  models faithful to botanical structure and development", SIGGRAPH 1988 (doi `10.1145/378456.378505`,
  stem `10.1145_378456.378505`).
- Shipping game (middleware): the SpeedTree Modeler manual (Unity), "Growth"
  (<https://docs.unity3d.com/speedtree-modeler/manual/growth.html>, read in full) and "Leaf mesh generator
  properties", its Growth group (<https://docs.unity3d.com/speedtree-modeler/manual/leaf-mesh-generator-properties.html>).
- Technical: Akenine-Moller, Haines and Hoffman, Real-Time Rendering 4th edition
  (`book_real_time_rendering_4th_akenine`: "A simple method that avoids popping altogether is to use what we
  call alpha LODs ... it is experienced as much more continuous than the discrete geometry LOD method, and so
  avoids popping"); this crate's `src/gpu/frame.rs` (the
  formula `GpuGrowth` places a vertex by) and `src/growth.rs` (`Skeleton::aged`).

## Craft principle

Leaves are born in sequence at the tip of a growing shoot and then stay where they were born:

> "Lateral organs (leaves and flowers) are successively initiated as primordia at the shoot apex yielding a
> regular arrangement on the stem known as phyllotaxis." (Mirabet et al.)

> "... become a so called internode at the extremity of which one or several leaves appear with new so called
> lateral buds at their axil and a new so called apical bud at the end of the internode" (de Reffye et al.,
> the growth rule of the model: a leaf comes with the internode that carries it)

> "The accordance with the botanical model insures that the leaves and fruits are inserted in the right places
> on the branches (according to the age of the tree). This implies that the model is also accurate when one
> looks at a precise part of the tree, instead of looking at its overall shape." (de Reffye et al.)

So a leaf's place on its branch is a fact of the leaf, not a function of how many other leaves the branch has
at the moment. Older leaves are near the base, younger ones toward the tip (acropetal order), and the
growing tip adds leaves; nothing already born moves along the wood because a newer leaf appeared.

## How shipping games do it

SpeedTree's growth (Cinema only; the real-time modelers ship a fully grown tree) works from the grown tree, as
`GpuGrowth` does, and times a leaf by where it stands on the grown branch:

> "In general, you create the model in the fully grown state and use these properties to control how it gets
> there." (Leaf mesh generator, Growth)

> "**In place**: The leaf will start growing exactly when the parent is long enough to reach its starting
> point in the fully grown model. Start % has no effect. This style makes the animation appear to trace out
> the model." (Leaf mesh generator, Growth, Timing, Style)

> "**Grow** - The model will develop into its final form gradually with branches and leaves beginning to grow
> once their parents have started growing." (Growth Wizard)

A leaf's place is a property of the grown model, a young branch shows the leaves its length has reached, and
none of them hops along the branch when another one starts. The same page's "Smoothing" ("Increase this value
to prevent wild leaf motion during growth") names the artefact: wild leaf motion. Real-time games that grow a
tree (a planted sapling, a crop) do it as this crate's `GpuGrowth` does: the fully grown mesh is the only mesh,
and a vertex shader shows a younger tree from per-node state.

## Model adopted

A branch's leaf `i` has one place, `along_i = 0.3 + 0.7 * t_i^2` with `t_i = (i + u_i) / N`, where `u_i` is the
leaf's draw (the same at every age: the node's stream is `SeededRng::from_parts(seed, node.id)` and the
draws per leaf do not depend on the count) and `N` is the leaf count of the **grown** branch
(`distributed_leaf_count` of `length / progress`). A young branch shows its first
`distributed_leaf_count(length)` leaves at those places, so its leaves are the inner part of the grown
branch's and the new ones appear toward the tip as it lengthens. For a grown branch `N = n`, so a mature tree,
and every tree at an age where every branch is grown, is unchanged bit for bit. This is the rule a strand
already has (`StrandParams`, iteration 7), and the one `GpuGrowth` shows today: `state.rs` writes
`leaf_count = distributed_leaf_count(young length)` and the shader shows the first `leaf_count` leaves at
`start + length * c`, `c` measured once on the mature mesh. Nothing in `src/gpu/` or the shaders changes.

`along` is a fraction of the branch, as `frame.rs` places a leaf (`start + length * tag.axis`), so while a
branch lengthens its leaves move outward with its wood, in proportion to their place on it. That is growth,
and it is continuous: a leaf moves by `along * (change of length)`, centimetres per step, never by a
re-spread.

## Parameters

None added. `FoliageParams::density`, `use_crown_density`, `height_falloff` and `leaf_size` set `N` and the
leaf as before; `GrowthParams::leaf_threshold` still decides when a node shows leaves at all.

## Validation targets

- A young branch's leaves are the first ones of the grown branch, at the same fractions of its length
  (`<= 1e-3`), for every `AllBranches` preset, at several ages (a test that fails before: an oak leaf sits up to
  92 cm from the place it has on the grown branch).
- Mature trees: `stats.tsv` columns 1-10 identical for every scenario whose branches are all grown; the young
  ones change only in the bounding box (leaf positions), never in a vertex or triangle count.
- CPU leaf against GPU leaf, as the probe above measures it: p95 under 1 cm and max under 1 cm (float
  noise), every `AllBranches` preset, and `grown_leaves_and_flowers_match_the_cpu_meshers` runs over
  `AllBranches` presets (it skips them today).
- CPU leaf moves per growth step: p95 under 5 cm per 0.1 year (the branch's own elongation), against 15 to 17.
- `compare_gallery` on the seven CI scenarios (`grow_oak_4`, `grow_oak_6`, `age_spruce_6`, `smoothyoung_spruce_4`,
  `age_willow_6`, `smoothyoung_oak_4`, `pipeyoung_pine_5`): no worse than 0.581 mean / 1.205 worst before
  the weeping strands, 0.694 / 1.452 on `main`; expected better.
- No new mesh flag; forest bench and build probe within 5% (the change moves where leaves go, not how many).

## What great looks like

A tree grown on the CPU at ages 3, 4, 5, 6 in a row and the same four ages on the GPU are the same picture;
a limb gaining a leaf does not shuffle the leaves it already has; in a growth clip a crown fills in from the
inside out as the tips reach further and each new leaf opens at the end of the leaf-bearing part of a limb.
The young crown must not read worse than today's: leaves sit on their wood (C5 detached or embedded leaves
in `bevy-flora-visual-qa`), the silhouette is still the species' (C8), no bare limb ends look cut off.

## Deviations

- A leaf's place is a fraction of its branch, not an absolute distance from the base ("In place" timing, as a
  sub-branch has: `attach_t / parent.progress` in `growth.rs`). Placing leaves in place would put the
  anchor at `a_i / progress` of the young node, which `frame.rs` cannot do (it has `start`, `length` and the
  mature offset, not the axis's direction and bend): a design choice, taken so the CPU tree equals the GPU one
  with no change to the shaders or the node buffer. The cost is that a growing branch's leaves move outward
  with its wood while its sub-branches hold their attach points; both are continuous.
- A young branch's tip is bare of leaves until it has grown them (the GPU path already shows it so; the CPU
  path used to spread the young leaves to the tip).
- No default changes.

## Pending index

- Prusinkiewicz and Lindenmayer, *The Algorithmic Beauty of Plants* (chapter on herbaceous plants: leaves
  and internodes in development; not in the corpus as text, `paper_search` finds only reviews).
- Hallé, Oldeman and Tomlinson, *Tropical Trees and Forests* (architectural models), paywalled.
- SpeedTree's real-time modelers have no growth: no shipped real-time source for per-leaf growth timing.
