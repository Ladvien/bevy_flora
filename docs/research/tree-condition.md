# Tree condition: a tree a game can call sick or dying, and read back

Iteration 5, 2026-10-31. Issue #87 (Sanctuary: "signs that are places": a sick tree, a withering stand,
a copse that dies in one season and not the next). The ask is a fact the game's logic can read, set
per tree and read back through `FloraIndex`; whether a condition also changes how the tree looks is
named in the issue as a separate matter and is the next slice (see `## Model adopted`).

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
- Second slice, not built here: the look. Where it goes, so the first slice does not close the door:
  per-instance data a pooled forest can carry without breaking its batches is Bevy's `MeshTag` on the
  mesh entities (one `u32` in the instance's uniform, so equal-mesh equal-material instances still
  batch), read in the leaf and wood fragment stages and the leaf vertex stage; the shape of the effect
  is Timberborn's (foliage colour moves green to ochre, nothing else changes) plus the survey's
  (defoliation: a share of leaves hidden by the per-leaf hash `FloraFire` and `leaf_hidden` already
  use, ~30-50% sick, ~80% dying). That is a shader change on every path (leaf, wood, impostor, GPU
  growth) with a gallery to judge it in, a pass of its own.

## Parameters

None. `TreeCondition` has no tunable. The classes' dieback shares are documented, not stored: nothing
reads them until the look slice.

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

## What great looks like

A game's blight system picks the trees it wants with `index.within(centre, radius)`, inserts
`TreeCondition::Dying` on each, and a frame later `index.iter().filter(|t| t.condition ==
TreeCondition::Dying)` is exactly those trees, among 10 or 100 000 pooled ones, with nothing to keep in
step: fell one, recycle a variant, reload the pool, recolour the stand, and the condition stays true.
The pooled forest's frame does not notice the feature.

## Deviations

No default changed. The API differs from the issue's example (`SetTreeCondition` message, and
`condition_of -> TreeCondition`): a component set by `insert`, and `Option<TreeCondition>` because a
felled or unindexed tree has no standing condition to report. Design defaults, recorded on the issue as
an assumed decision. `IndexedTree` gains a pub field, a breaking change: the next release is a minor one.

## Pending index

- Dobbertin and Brang 2001 (transparency and next-year mortality), cited through
  `10.1007_s10342-005-0085-3`, not in the corpus.
- Timberborn's own models and `NaturalResourceLifecycleModel`: known only through the mod's notes on the
  decompiled game (a web page); no first-party source is indexed.
- `distill_search` answered with a GPU-busy 500 for two of the twelve queries of this iteration; the
  retries returned. Nothing in this note waits on it.
