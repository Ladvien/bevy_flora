# Tree condition: a tree a game can call sick or dying, and read back, and see

Iteration 5, 2026-10-31, and iteration 6 (the look). Issue #87 (Sanctuary: "signs that are places": a
sick tree, a withering stand, a copse that dies in one season and not the next). The ask is a fact the
game's logic can read, set per tree and read back through `FloraIndex`; iteration 5 built that (state
only). Whether a condition also changes how the tree looks is named in the issue as a separate matter;
iteration 6 builds it (`## Model adopted`, "Slice 2"): the issue's own reason for the feature is "so the
player can read the forest's state as a symptom rather than as scenery".

## Sources

- Botanical: the visual assessment of tree crowns in forest-health surveys. "Tree growth as indicator of
  tree vitality and of tree reaction to environmental stress: a review" (stem `10.1007_s10342-005-0085-3`);
  "Traditional and Novel Indicators of Climate Change Impacts on European Forest Trees" (Forests 2017,
  stem `10.3390_f8040137`); "Divergent climate-growth responses in radial growth of Chinese pine
  forests" (stem `10.3389_fpls.2026.1809811`) for the dieback grades; "Tree mode of death and mortality
  risk factors across Amazon forests" (stem `10.1038_s41467-020-18996-3`) for what comes after.
- Shipping game: Timberborn's plants, alive, dying, dead, as a mod's notes on the decompiled game record
  them (<https://github.com/timbermods/dam-good-maps/pull/742>, written 2026-10-30; second hand, not in
  the corpus). Gregory, "Game Engine Architecture", 3rd edition, chapter 16 (stem begins `Game Engine
  Architecture, Third Edition`), on what a game object is.
- Technical: Tasnim and Zhao, "The Essence of Entity Component System" (stem
  `10.1145_3748522.3779910`); Bevy 0.20 `bevy_ecs` `Changed<T>` and `RemovedComponents<T>`
  (`src/query/filter.rs`, `src/lifecycle.rs` of the crate version this repository builds against).
- Slice 2, botanical, what a declining crown looks like: the same three papers (the survey's visual
  indicators are defoliation, dieback and "symptoms displayed on leaves and branches",
  `10.3390_f8040137`), and "Monitoring the effects of air pollution on forest condition in Europe: is
  crown defoliation an adequate indicator?" (iForest 2010, DOI `10.3832/IFOR0538-003`; in the catalog,
  not yet converted, see `## Pending index`).
- Slice 2, technical: Bevy 0.20 `bevy_mesh::MeshTag` (`src/components.rs`) and
  `bevy_pbr::mesh_functions::get_tag` (`src/render/mesh_functions.wesl`); this crate's own use of the tag
  for gliding looks (`src/glide.rs`).

## Craft principle

Forest health is read off the crown, in classes, and the class predicts the tree's fate. The survey
literature: "Crown defoliation is the most widely used parameter for the assessment of tree vitality in
large-scale terrestrial surveys. It is a visual indicator of the relative amount of foliage in the crown
of the assessed tree compared to that in the crown of a reference standard tree." (`10.3390_f8040137`).
Classes, not a continuous score, are what the field uses: "visual assessment of tree crowns and
classification into transparency or defoliation classes has become a standard procedure in many parts of
the world", and the class is not decoration: "Dobbertin and Brang (2001) found that next-year mortality
rate increased exponentially with transparency." (`10.1007_s10342-005-0085-3`). A graded decline looks
like this, by stand: "Relatively healthy stands exhibited ~10% crown dieback ... Slightly declining
stands showed ~30% crown dieback ... Moderately declining stands exhibited ~50% crown dieback ...
Severely declining stands showed ~80% crown dieback" (`10.3389_fpls.2026.1809811`).

So a tree has an ordinal condition that runs healthy, declining, near death, and it is an attribute of
that tree (one tree in a stand can be at ~80% while its neighbours are at ~10%). Death itself is not a
condition of a standing crown but an outcome with a mode: "Dead trees were diagnosed as having died
standing or non-standing (broken or uprooted)" (`10.1038_s41467-020-18996-3`); in this crate the dead
tree is the game's to resolve (fell it with `FellTree`, replace it, or leave it standing bare), which is
why the condition has no fourth, `Dead`, class.

Slice 2 reads the same literature for what is drawn. Defoliation is a share of the crown's leaves, so a
sick tree is a thinned crown, not a recoloured one: defoliation "is a visual indicator of the relative
amount of foliage in the crown of the assessed tree compared to that in the crown of a reference
standard tree" (`10.3390_f8040137`), and the stand grades above are those shares of the
crown (~30% slightly declining, ~50% moderately, ~80% severely). A thinned crown is also where a
decline shows first and longest: "defoliation by insects becomes first visible in crown transparency
while stem growth reaction occurs with delay. On the other hand, extreme summer drought ... affects
stem growth almost immediately, while foliage reduction becomes only visible months later"
(`10.1007_s10342-005-0085-3`): the visible sign of a sick tree is its foliage. The leaves that stay are
the survey's "symptoms displayed on leaves" (`10.3390_f8040137`), which this crate draws as leaf colour
withering toward a dry ochre-brown (a design default: the corpus names the symptom, not a colour; the
iForest paper in `## Pending index` is the one to read for it).

## How shipping games do it

Timberborn, a shipped game, keeps exactly three states of a living plant and makes the middle one a
state the simulation sets, not an effect the renderer invents. From the mod notes on its decompiled
`NaturalResourceLifecycleModel`: "A plant is dying while any of its timers runs. A timer runs from the
moment its cause starts, until the cause ends (then the timer resets) or the plant dies." The game
draws the state with a model of its own ("Every species has Mature, MatureDry, MatureDead, Seedling,
SeedlingDry and SeedlingDead models."), and a dry model is the living one with its foliage colour moved:
"A Dry model is the living one with the same parts and the same UVs. Its foliage vertex colours turn from
green to ochre, and the foliage draws in a little. A grown plant loses no leaves". The state is a fact
the game logic owns and the look follows it; the state was never a property of a shared model.

The object model it rests on is the usual one: "a game object is essentially a collection of
attributes (the current state of the object) and behaviors (how the state changes over time and in
response to events)" (Gregory, ch. 16). A tree's health is one of its attributes, so it belongs to the
tree (the entity), not to the pool of meshes and materials that many trees share.

Timberborn's dry model is exactly the second half of that: the plant's parts and UVs are the living
one's and only the foliage colour moves, "green to ochre", and "A grown plant loses no leaves". This
crate's decline goes one step further than Timberborn's, because the survey's defoliation is the
field's own first indicator: a sick or dying tree also loses a share of its leaves (the models are
per-tree meshes there; here the shader hides leaves per card, so one pooled mesh serves every class).
Per-instance data that must not break batching is what Bevy's `MeshTag` is for: "If you aren't using a
`GpuComponentArrayBuffer`, the tag is free for you to use for whatever purpose you wish", read in the
shader with `get_tag(instance_index)`; this crate already rides a pooled look's glide index on it.

## Model adopted

- `TreeCondition { Healthy, Sick, Dying }`, a `Component` (`src/condition.rs`; pure CPU like
  `shape.rs`), ordered `Healthy < Sick < Dying` (the defoliation classes), `Default` is `Healthy`.
  The grades are the survey's: healthy ~10% dieback, sick ~30-50%, dying ~80% and next-year mortality
  rising (the numbers are for the look slice, the enum is the class).
- It lives on the tree's entity, a `ProceduralTree` or a `PooledTree`. A tree without the component is
  healthy (nothing is added to the 100 000 trees of a pooled forest). Setting is inserting
  (`commands.entity(tree).insert(TreeCondition::Sick)`), clearing is removing it or inserting
  `Healthy`; Bevy's change detection is the notification (`Changed<TreeCondition>`), so there is no
  `SetTreeCondition` message: a message that only inserts a component is a second way to do one thing.
  Tasnim and Zhao: "An entity can be created or destroyed; a component can be added to or removed from
  an entity." (the lifecycle events a component state rides on).
- `FloraIndex` mirrors it: `IndexedTree::condition`, so every query that returns trees
  (`within`, `nearest`, `get`, `iter`) says what each one is, and `FloraIndex::condition_of(entity) ->
  Option<TreeCondition>` (`None`: not a standing tree the index holds). `index_trees` re-reads it on
  `Changed<TreeCondition>` and `RemovedComponents<TreeCondition>` (removals first, so a remove and an
  insert in one frame end at the insert; a despawned tree is already out of the index, so its removal
  record changes nothing), and reads it whenever it inserts a tree, so a condition set before the tree
  is indexed (a pooled tree whose pool has not loaded) applies when it is.
- Pooled trees: per instance. The pool is meshes and materials; the condition is on the entity, so a
  `PooledTree` that changes variant or pool, a `TreePool` that reloads, and a `PooledTree` taken off and
  put back all keep it. A tree despawned takes it with it. No rule of the crate clears it.
- Felled: the felled tree leaves `FloraIndex` (it is lying wood now), so `condition_of` answers `None`;
  the component stays on the entity as it was at the cut, readable with a `Query<&TreeCondition>`.
  Setting one on a felled tree changes the component and nothing else.
- Slice 2, the look (iteration 6), built:
  - The class travels in bits 8 and 9 of the `MeshTag` of every mesh the tree draws with (wood, leaves,
    every level of detail, the impostor), above the look's glide tag in the low byte
    (`MAX_GLIDE_LOOKS` is 64, so the byte is not full). `sync_condition_tags` writes it when the
    component changes or the tree's children do; `spawn_pooled_trees` and `sync_pooled_looks` carry it
    when they write a look's tag, so a look change keeps the class. Instances of equal mesh and
    material still batch (the tag is per-instance data), so a mixed forest draws in the batches it did.
  - A tree without the component has tag bits 0: nothing about it changes, and the sync system's
    queries only touch trees that have the component.
  - Defoliation: the leaf vertex stage of the main pass, the prepass and shadow maps, and the
    GPU-growth twins hides a share of the leaves, chosen by a per-leaf draw from the leaf's phase
    (`leaf_decline_moment`, not the season's moment, so the leaves a sick tree loses are not the ones
    that would turn first). Sick: 0.35, dying: 0.8 (`TreeCondition::defoliation`). Hidden leaves cast
    no shadow, so a thin crown lets dappled light through. Only leaf cards are hidden: flowers and
    fruit (vertex alpha below 0.5) are the game's to count (`TreeFruit`), so they are drawn as they
    were, and `FallingLeaves` skips the cards that are hidden (`Card::on_tree`).
  - Withering: each leaf's colour moves toward the dry leaf colour, by `FloraUniform::withered`, the
    ratio of the dry colour (design default, srgb 0.47, 0.31, 0.11) to the species' leaf colour, as
    `autumn` is the ratio of the autumn colour; sick 0.5 and dying 1.0 of the way
    (`TreeCondition::withering`), each leaf by its own draw (0.6 to 1.4 of that), so a sick crown is
    mottled olive and brown among the green and a dying one is uniformly dry. Wood is not touched.
  - Impostors: the baked picture is neutral grey, so the same two effects are done per patch of
    texels (the way the season's patches are): the leafy texels wither and the crown's coverage
    thins by the class's share, with the alpha-to-coverage it already uses.
  - A `FloraSeason` autumn or leaf fall and a condition compose (the condition's tint multiplies the
    season's, its hidden leaves are hidden whatever the season does). A device with no flora
    materials (`FloraMaterialSupport` absent) draws plain materials and no class, as it does no glide.

## Parameters

`TreeCondition::defoliation()` (healthy 0, sick 0.35, dying 0.8) and `TreeCondition::withering()` (0,
0.5, 1.0): public, documented, and mirrored by constants in `common.wesl`. The dry leaf colour
(`WITHERED_LEAF`) is a design default.

## Validation targets

- A condition set on a `ProceduralTree` and on a `PooledTree` reads back through `FloraIndex::get`,
  `condition_of`, `within` and `nearest`, as set, changed and cleared.
- A pooled tree keeps it through a variant change, through its `PooledTree` being removed and put back,
  and through the pool asset being modified; one set before its pool exists applies when the pool does.
- Setting and reading one on a felled tree does not panic; the index no longer holds the tree and the
  component is still there.
- A `RecolorTree` and a condition change on the same tree in one frame both apply, and trees of equal
  colours still share one material (`SharedFlora`): the count of live `FloraMaterial`s is what the
  recolour alone gives.
- Nothing about geometry or drawing moves: a narrowed gallery (a few presets, mature and young) has
  `stats.tsv` columns 1-10 and every PNG identical before and after; the pooled forest bench (the one
  mode that indexes 10 000 trees) within noise of main, interleaved A/B.
- Slice 2: a tree with no condition (or `Healthy`) draws as it did: all 492 existing gallery PNGs and
  `stats.tsv` rows byte-identical to main's before and after; the mesh children of a tree carry the
  class in their `MeshTag` whichever way the tree is made (procedural, pooled, on the GPU), keep the look's
  tag bits, and lose it when the component goes or `Healthy` is set; a sick and a dying gallery scenario
  of every kind of crown (broadleaf, conifer, willow, fruit tree, fern) are told from each other and
  from a healthy one in the full frame and in the close-up; CPU and GPU growth agree on them (the
  parity comparison); at 100 m the impostor shows the same class; a mixed forest costs what a healthy
  one does (the forest bench, interleaved, within 5%).

## What great looks like

A game's blight system picks the trees it wants with `index.within(centre, radius)`, inserts
`TreeCondition::Dying` on each, and a frame later `index.iter().filter(|t| t.condition ==
TreeCondition::Dying)` is exactly those trees, among 10 or 100 000 pooled ones, with nothing to keep in
step: fell one, recycle a variant, reload the pool, recolour the stand, and the condition stays true.

The pooled forest's frame does not notice the feature.

Slice 2: a game sets `Dying` on a stand's trees and the player, walking up the road, reads it before any
text does: from 150 m the copse is a duller, thinner patch among the green ones; at 30 m its crowns are
mostly sky and brown leaves, with a few green ones left; a sick tree in the same stand is thinned by a
third and mottled yellow-olive, and nobody takes it for the autumn: it is off in summer too, on a pine
as on a maple. The wood is as it was, the shadows are dappled by the same gaps, and the tree sways as
the others do.

## Deviations

No default changed. The API differs from the issue's example (`SetTreeCondition` message, and
`condition_of -> TreeCondition`): a component set by `insert`, and `Option<TreeCondition>` because a
felled or unindexed tree has no standing condition to report. Design defaults, recorded on the issue as
an assumed decision. `IndexedTree` gains a pub field, a breaking change: the next release is a minor one.

Slice 2 changes no default. The shares (0.35, 0.8), the withering (0.5, 1.0) and the dry colour are
design defaults, chosen within the survey's grades (sick between slightly and moderately declining,
dying the severe grade) and judged in the gallery; a game that wants others today recolours the tree
(`RecolorTree`, `PooledLook`). The public `FloraUniform` gains a field (`withered`), a breaking change
with `IndexedTree::condition` in the same release.

## Pending index

- Dobbertin and Brang 2001 (transparency and next-year mortality), cited through
  `10.1007_s10342-005-0085-3`, not in the corpus.
- Timberborn's own models and `NaturalResourceLifecycleModel`: known only through the mod's notes on the
  decompiled game (a web page); no first-party source is indexed.
- `distill_search` answered with a GPU-busy 500 for two of the twelve queries of iteration 5 and for all
  of iteration 6's (the card was full: bge-m3 wanted 5 000 MB, 2 500 free); the sources of slice 2 were
  read with `markdown_read` and `paper_search`. Nothing in this note waits on it.
- `10.3832/IFOR0538-003` (iForest 2010, crown defoliation as an indicator): in the catalog, conversion
  asked of scribe this iteration and not returned within the call; its text is not quoted here.
- A first-party source on how shipped open-world games draw a dying tree (SpeedTree's health or dry
  variants; Timberborn's own models): the corpus has none, and the mod notes quoted above are second
  hand.
