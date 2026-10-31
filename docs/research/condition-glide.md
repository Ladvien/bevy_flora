# Condition glide: a tree that falls ill over seconds, not in one frame

Iteration 10, 2026-10-31. Backlog item 2 of iterations 6 to 9 ("a gradual decline"). `TreeCondition` (iteration
5-6, issue #87) is three classes the game inserts on a tree; the draw changes in the frame after the insert:
a sick oak loses 35% of its leaves at once and a dying one 80%, a pop of up to a thousand leaves. A game can
fake a decline with two class changes; it cannot fake a smooth one, and Sanctuary's own ask in #87 was "a copse
that dies in one season and not the next", which is a season, not a frame. This iteration draws a change of
class as a glide.

## Sources

- Botanical: how a crown actually declines, and over what time. "From the comfort zone to crown dieback:
  Sequence of physiological stress thresholds in mature European beech trees" (Science of the Total
  Environment, stem `10.1016_j.scitotenv.2020.141792`); "European beech dieback after premature leaf
  senescence during the 2018 drought in northern Switzerland" (Plant Biology, stem `10.1111_plb.13467`);
  Granier et al., the 2003 drought's premature leaf shedding in beech (Annals of Forest Science, stem
  `10.1051_forest:2006042`). The survey grades the classes come from stay as in `tree-condition.md`.
- Shipping game: Unreal Engine 5.7's Procedural Vegetation Editor (Epic's documentation,
  <https://dev.epicgames.com/documentation/unreal-engine/procedural-vegetation-editor-in-unreal-engine>,
  read 2026-10-31); Whatley, "Toward Photorealism in Virtual Botany" (GPU Gems 2, chapter 1, Far Cry's
  vegetation), as Real-Time Rendering 4th edition quotes it (stem `book_real_time_rendering_4th_akenine`,
  page 879); Timberborn's dry model, as `tree-condition.md` quotes it.
- Technical: Luebke et al., "Level of Detail for 3D Graphics" (stem `book_level_of_detail_3d_graphics_luebke`,
  pages 140-141, on popping and blending between states); Bevy 0.20 `MeshTag`, as `tree-condition.md`
  quotes it; this crate's `condition.rs`, `shading/common.wesl`, `glide.rs`.

## Craft principle

A crown does not go from healthy to dying between two frames and not in a step of three classes either. The
beech study that gives the sequence calls it a sequence over progressive stress: "In support of our first
hypothesis, important responses of the investigated trees to progressive drought, such as reduced
transpiration rate, early defoliation, native embolism and crown dieback were coherent across our study
plots". The loss is graded in time: "mean crown dieback in the year after the drought exceeded 20%", and in
the Swiss survey "mortality increased monotonously over time". The earliest sign is leaves shed while they are
still green, over weeks: "it was the first time, over a 30 year-period (since 1976, [9]), that we measured a
direct reduction of canopy leaf area index due to premature leaf shedding during August. The fallen leaves
were still green". So the thing to draw is a crown whose share of lost leaves rises continuously and whose
remaining leaves dry continuously, between the survey's anchors (a third, four fifths), and a game chooses
how long that takes (a season for a drought, seconds for a lightning strike).

## How shipping games do it

The engines this crate is measured against treat a plant's health as a number, not as a class. Unreal's
vegetation tool makes the health of a plant an offset the artist turns: "a dying brown branch with a light
condition of 0.2 and a healthy green branch with 0.8 — enabling the light condition makes the distributor
pick unhealthy foliage where light was low and healthy foliage where light was high; adjust the Offset
parameter to make the plant overall healthier". Timberborn keeps three states and swaps models ("Every
species has Mature, MatureDry, MatureDead, Seedling, SeedlingDry and SeedlingDead models."), which is the
pop this iteration removes between its dry and dead states: a game with three discrete looks and a timer for
the move between them. For the dissolve of a leaf cloud, Far Cry's way, in Real-Time Rendering's words:
"The advantage of using this technique standalone is that it is experienced as much more continuous than the
discrete geometry LOD method, and so avoids popping. [...] To fade out distant vegetation, Whatley [1876]
discusses how a noise texture can be used for screen-door transparency. This has the effect of a dissolve".
That is what this crate's decline already is, per leaf: each leaf has its own draw, and it is lost when the
draw is below the tree's share. Raising the share smoothly is the same dissolve, driven by time instead of
distance.

## Model adopted

- **One scalar, `level`, 0 (healthy) to 1 (dying), with the survey's classes as anchors**: `Healthy` 0, `Sick`
  0.5, `Dying` 1. Between anchors, withering is linear in the level (0, 0.5, 1.0: exactly the class values
  at the anchors) and the lost share is piecewise linear through the class shares (0, 0.35, 0.8). At an
  anchor every value equals the class's, so a settled tree is drawn exactly as before.
- **A change of `TreeCondition` is drawn as a glide of `level`** from where the tree is drawn now to the new
  class's, over `ConditionGlide::seconds` (a resource; 0, the default, draws a change at once, as every
  caller has had it). The curve is a smoothstep (it starts and stops gently, like the leaf ease-in of
  iteration 9). A change in the middle of a glide starts from the level the tree shows then, so the draw is
  continuous whatever the game does; setting the class a glide is already heading for changes nothing.
- **What the game reads is the class it set**: `FloraIndex` and `condition_of` answer the target at once,
  not the level on screen (the state is the game's, the look follows it: `tree-condition.md`).
- **A tree that appears with its condition shows it at once.** A glide starts only for a tree that was
  already standing (its meshes existed before the frame the condition changed): a saved forest loaded with
  a thousand dying trees does not spend a minute dying on load.
- **The tag carries the level**: the class stays in bits 8-9 of the mesh's `MeshTag` (the target); bit 10 says
  "gliding" and bits 11-18 hold the level in 255 steps (`k / 254`: 0.5 is exactly 127). While the bit is set
  the shaders read the level, otherwise the class (as the level `class / 2`). The batch of a pool's trees
  is unchanged: trees of one mesh and material still batch, whatever their level.
- **Only trees that glide are visited each frame**, through a component that exists for the glide's length
  (sparse storage); a forest of settled trees, ill or not, costs what it cost.
- **The falling leaves follow the level**: `Decline` takes the level on screen, so a leaf that falls from a
  gliding tree is as dry as those left on it and a leaf the shader has hidden does not fall.
- **The impostor follows too** (its leafy texels wither toward the dry colour and lose a share by the
  tag's level), so a far tree declines with the near one.

## Parameters

- `ConditionGlide::seconds`: 0 (at once). Design default, kept so no existing caller's draw changes.
- The ease: smoothstep. Design default.
- The level's resolution: 8 bits (255 steps). At the step a 3 000-leaf oak loses 7-8 leaves (0.8 of the
  crown over 254 steps); over a 20 s glide that is one step per 0.08 s.
- The anchors (0, 0.5, 1) and the shares and witherings at them: unchanged from iteration 6.

## Validation targets

- A glide is monotone and reaches its target exactly; the tag at the end is the class's, bit for bit, as
  before the glide existed (the tag tests of `condition.rs` hold unchanged).
- Halfway through a glide from healthy to dying a tree has lost about 0.8 x smoothstep(0.5) of its leaves
  (0.4); the lost leaves are the ones a dying tree loses (the first of its draw order): none is lost that
  the target keeps, and a leaf, once lost, stays lost until the glide reverses.
- A change mid-glide is continuous: the level on the first frame of the new glide is the level of the last
  frame of the old.
- A tree set before it is drawn, and a tree that glides at 0 seconds, is at its class at once.
- Settled trees: the forest bench (pool, naive, gpu-live) within 5%; the gallery's 526 images byte for byte
  (no gallery scenario glides).
- On screen: a young-to-mature oak photographed at levels 0, 0.25, 0.5, 0.75, 1 reads as one tree thinning
  and drying, no step between neighbouring frames of a recorded glide large enough to see as a pop; CPU and
  `GpuGrowth` trees show the same level (the shared `common.wesl`).

## What great looks like

A stand of oaks hit by a blight: over a minute the crowns thin from the top outward in no order the eye can
find, the greens go olive and then the ochre of a dry leaf, individual leaves are gone with no moment at
which the tree changes; the far trees, impostors, do the same. Reverse it (`Healthy`) and the crowns thicken
back as the leaves return. A lightning-struck tree (1-2 s) is the same code with a smaller number.

## Deviations

No default changed (`ConditionGlide::seconds` is 0). `TreeCondition`'s own `defoliation` and `withering`
are as they were. The `MeshTag` layout gains bits 10-18 above the class; bits 0-9 mean what they meant.

## Pending index

- Whatley's chapter itself (GPU Gems 2, chapter 1) is quoted second-hand through Real-Time Rendering; the
  corpus has the book, not the chapter.
- A time series of crown defoliation in one declining tree (the beech papers give a sequence and yearly
  grades, not a per-week curve): the smoothstep is a design default until one is found.
- Unreal's page documents an offset for a plant's health at generation time; whether any shipped game
  animates a health value at runtime is not documented in what the corpus or the web gave.
