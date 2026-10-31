# Wind: the main bend (`wind_bend`)

How far a whole plant leans downwind, and how that lean is shared out between a 0.5 m shrub and a
30 m redwood standing in the same weather. Written for iteration 1 (issue #89), which merges
`fix/plant-lean-share` (0.46.1: `f45fac5`, `1e4c379`): the lean as a fixed share of the plant's own
height, and the presets' `wind.sway` retuned to a 0.9-1.6 band (D-071, D-072). Flutter, gusts, gust
fronts and shakes are out of scope here; they are unchanged.

## Sources

- Botanical / model: Minamino and Tateno 2014, "Tree branching: Leonardo da Vinci's rule versus
  biomechanical models" (`10.1371_journal.pone.0093535`), on McMahon and Kronauer's elastic
  similarity; Eloy et al. 2017, "Wind loads and competition for light sculpt trees into
  self-similar structures" (`10.1038_s41467-017-00995-6`).
- Shipping game: Sousa, "Vegetation Procedural Animation and Shading in Crysis", GPU Gems 3
  chapter 16 (Crytek, CryENGINE 2; web, not in the corpus). Pelzer, "Rendering Countless Blades of
  Waving Grass", GPU Gems 1 chapter 7 (the Codecreatures Benchmark; corpus).
- Technical: Zioma, "GPU-Generated Procedural Wind Animations for Trees", GPU Gems 3 chapter 6
  (EA DICE; web), and GPU Gems 1 chapter 7 for the per-vertex cost model.

## Craft principle

A plant that is built like its larger relatives bends like them, by the same *fraction* of its own
length. Minamino and Tateno state McMahon and Kronauer's hypothesis:

> "In the elastic similarity hypothesis, the deflection at the tip of a branch is assumed to be
> proportional to the overall length of the branch [8]. In other words, the deflection angle of an
> arbitrary microsection of a branch (with the constant relative length) is determined by its
> relative position on the branch, independent of branch length."

So under one wind a shrub and a pine lean through the same angle; the pine's tip travels further in
metres because the pine is longer. What separates species is how stiff they are for their size, the
"stiffness" Crysis gives each instance. Eloy et al. tie that to the load: the bending moment at a
branch base from wind on its foliage is

> "M_k = (α/2) ρ U² S_fol N_k ℓ_k"

that is, proportional to the path length ℓ to the foliage it carries, which is what lets a model
measure the lean against the plant's own distance from its root.

## How shipping games do it

Crysis (Sousa, GPU Gems 3 ch. 16) splits the motion in two and measures the main bend against the
mesh's own height:

> "In our approach, we divide animation into two parts: (1) the main bending, which animates the
> entire vegetation along the wind direction; and (2) the detail bending, which animates the
> leaves. [...] each instance has its own stiffness"

> "We generate the main bending by using the xy components of the wind vector, which gives us the
> wind direction and its strength, using the vegetation mesh height as a scale to apply a
> directional vertex deformation. Note that care must be taken to limit the amount of deformation;
> otherwise, the results will not look believable."

> "We accomplish the main vegetation bending by displacing vertices' xy positions along the wind
> direction, using normalized height to scale the amount of deformation. [...] We achieve this by
> computing the vertex's distance to the mesh center and using this distance for rescaling the new
> displaced normalized position."

Its bend factor, `fBF = vPos.z * fBendScale; fBF += 1; fBF *= fBF; fBF = fBF * fBF - fBF`, expands to
`2h + 5h² + 4h³ + h⁴`: linear at the root, steeper toward the top. The Codecreatures grass (GPU Gems 1
ch. 7) moves "only the upper vertices of the grass objects" and computes one translation per
object so a meadow is "one draw call".

## Model adopted

`wind_bend` (`src/shading/common.wesl`), after the merge:

```
rel   = world - origin                       // the vertex from the plant's root, world units
lean  = MAIN_BEND * sway * scale_y * |rel|   // MAIN_BEND = 0.04, sway = TreeParams::wind.sway
moved = rel + push * lean * FloraWind::bend + shake * lean
out   = origin + moved * |rel| / |moved|     // bend, do not stretch (Crysis' rescale)
```

The 0.46.0 term was `MAIN_BEND * sway * (scale_y * trunk.height) * (local_y / trunk.height)²`: the
same value at the top of a mature, unscaled plant, a square of the trunk-normalised height below it,
and a function of the *preset's* trunk height (a young tree's top leaned a smaller share). The new
term is a function of the vertex's own distance from the root, so the lean is a share of the plant's
own height at every age, and a low vertex far from the trunk (a willow strand, a spreading limb)
sways by its distance as a branch of that length would. `sway` is the species' stiffness, retuned to
a multiplier band of 0.9-1.6 (D-072, approved by the owner 2026-10-15), so a bush no longer out-leans
a pine: the old 0.2-1.5 spread decided who moved.

## Parameters

| Name | Value | Source |
|---|---|---|
| `MAIN_BEND` | 0.04 | unchanged by the merge; design default |
| `TreeParams::wind.sway` | 0.9-1.6 per preset (willow 1.6, palm 1.4, birch and poplar 1.1, maple, cherry, apple, japanese maple, shrub, fern 1.0, the rest 0.9) | D-072, owner-approved retune |
| `FloraWind::bend` | caller's | the weather |

## Validation targets

1. The lean at the top of an unscaled mature plant equals 0.46.0's (`MAIN_BEND * sway * height`).
2. Two presets lean shares of their own height within the sway band: the most-swaying species at
   most 2x the least (`a_shrub_and_a_pine_sway_by_near_the_same_share_of_their_own_height`).
3. A plant drawn at `Transform` scale `s` leans the same share of its own (scaled) height as at
   scale 1 (elastic similarity across sizes; a foot world scales every tree by 3.28 and must not lean
   3.28 times as far). **Not met on the branch**: see Deviations.
4. No CPU cost: shader-only, the forest bench within 5% per mode.
5. No geometry moves: the gallery (wind off) is identical, GPU growth parity unchanged.

## What great looks like

A forest in a steady wind reads as one weather: shrubs, saplings and the tallest pines lean by the
same few per cent of their own height, the willow and palm visibly the most supple, conifers and the
oak firm, nothing bobbing more than its neighbours because it is small. The trunk stays rooted (no
slide at the ground), the crown keeps its volume (no stretch), and a tree scaled up by a scatter or
a world unit sways like a bigger tree of its kind, not like a rubber one.

## Deviations

- **D-073 (bug, blocked):** `lean` multiplies `scale_y` into a length (`|rel|`) that is already in
  world units, so the share of its own height a plant leans grows with its `Transform` scale: a
  `Scatter` forest (scale 0.85-1.15 by default) leans 0.85-1.15 times the share, a foot world
  (`FloraUnits` 3.28, trees scaled by 3.28) 3.28 times, a centimetre world 100 times (the bend's
  rescale then lays the tree nearly flat). 0.46.0 did not have this (its term used the trunk's
  height in the mesh's own units). Measured (two mature pines, steady wind, side view, crown top
  shift over height): 0.46.0 3.14% at scale 1 and 3.10% at scale 3; 0.46.1 3.43% and 10.34%.
  The fix is `lean = MAIN_BEND * sway * |rel|` (and dropping the `scale_y` argument), but the test
  `the_lean_comes_from_the_vertexs_own_distance_from_the_root` finds the lean line by
  `contains("scale_y")` and fails without it; the loop may not edit a test, so this waits for the
  owner (issue #90).
- Validation target 3 is therefore not met in 0.46.1.
- The profile below the top changed from quadratic in the trunk-normalised height to linear in the
  distance from the root (Crysis' bend factor is linear at the root too, `2h + ...`). Approved with
  D-071.
- README's "a bonsai hardly at all" no longer held (the bonsai's sway is 0.9 since D-072); the
  wind section now says "a willow or a palm moves more than an oak or a pine".

## Pending index

- GPU Gems 3 chapters 6 and 16 are web pages, not in home-still; a `paper_download` cannot fetch a
  book chapter. Add them if a PDF of GPU Gems 3 is ingested.
- McMahon and Kronauer 1976, "Tree structures: deducing the principle of mechanical design"
  (J. Theor. Biol.), the elastic similarity source itself, is quoted here only through Minamino and
  Tateno.
