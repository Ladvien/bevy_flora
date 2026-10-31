//! How well a tree is: [`TreeCondition`], a fact about one tree that a game sets, reads back and sees.
//!
//! A game that wants a sick tree, a withering stand or a copse that dies in one season inserts
//! [`TreeCondition`] on the tree's entity (a [`ProceduralTree`](crate::ProceduralTree) or a
//! [`PooledTree`](crate::PooledTree)) and finds it again through [`FloraIndex`](crate::FloraIndex):
//! [`IndexedTree::condition`](crate::IndexedTree::condition) on every tree a query returns, or
//! [`FloraIndex::condition_of`](crate::FloraIndex::condition_of) for one entity.
//!
//! The tree is drawn in its condition. A sick crown is thinned and mottled with dry leaves, a dying
//! one is mostly bare and brown ([`TreeCondition::defoliation`], [`TreeCondition::withering`]); the
//! wood, the fruit and the flowers are as they were. The class rides in the [`MeshTag`] of every
//! mesh the tree is drawn with, above the glide tag of a pooled look (`glide.rs`), so trees of one
//! mesh and material still batch whatever their conditions; the shaders (`condition_*` in
//! `common.wesl`) hide the leaves and move the colours of the rest. A game that wants a look of its
//! own on top recolours the tree ([`RecolorTree`](crate::RecolorTree), or a
//! [`PooledLook`](crate::PooledLook) of a pool's own looks); the two compose.
//!
//! Pure CPU like `shape.rs` (it derives `Component`), except `sync_condition_tags`, which writes
//! the meshes' `MeshTag` (a data component, as `Mesh3d` is).

use bevy::color::Color;
use bevy::ecs::prelude::*;
use bevy::mesh::{Mesh3d, MeshTag};
use bevy::reflect::Reflect;
use bevy::time::Time;

/// The dry leaf colour a sick or dying tree's leaves move toward, whatever their own colour:
/// `FloraUniform::withered` is its ratio to the species' leaf colour. A design default
/// (`docs/research/tree-condition.md`).
pub(crate) const WITHERED_LEAF: Color = Color::srgb(0.47, 0.31, 0.11);

/// Where a tree's condition starts in the [`MeshTag`] of its meshes: the bits below it are the glide
/// tag of a pooled look (at most `MAX_GLIDE_LOOKS`, 64), the two bits from it the class. `common.wesl`'s
/// `CONDITION_SHIFT` is this number.
pub(crate) const TAG_SHIFT: u32 = 8;

/// The bits of a [`MeshTag`] that are a pooled look's.
pub(crate) const TAG_LOOK_MASK: u32 = (1 << TAG_SHIFT) - 1;

/// The tag bit that says a tree's decline is on its way (a [`ConditionGlide`] is running): the shaders then
/// read the level in the bits above it instead of the class. `common.wesl`'s
/// `CONDITION_GLIDING_BIT`.
const GLIDING_BIT: u32 = 1 << (TAG_SHIFT + 2);

/// Where the level of a gliding decline starts in a mesh's tag, 8 bits to 255 (`LEVEL_STEPS` is the
/// top step in use). `common.wesl`'s `CONDITION_LEVEL_SHIFT` and `CONDITION_STEPS`.
const LEVEL_SHIFT: u32 = TAG_SHIFT + 3;
const LEVEL_STEPS: f32 = 254.0;

/// How long a change of a tree's [`TreeCondition`] takes to be drawn, in seconds. A resource
/// the plugin adds with 0, which draws a change at once: a sick tree has lost its leaves in the frame
/// after the insert. Above 0 a change is drawn as a **glide** (`docs/research/condition-glide.md`): the
/// share of the crown's leaves that is lost rises, and the leaves that are left dry, continuously from
/// what the tree shows now to what the new class shows, along a smoothstep, whatever the tree's own
/// class was (a change half way through starts where the tree is). A tree that is made healthy
/// recovers the same way, its leaves returning; so does one whose component is removed.
///
/// What a game *reads* does not wait: [`FloraIndex`](crate::FloraIndex) and
/// [`IndexedTree::condition`](crate::IndexedTree::condition) say the class the game set the moment
/// it was set; only the picture glides. A tree that appears with its condition (a forest loaded from
/// a save, a pool whose assets arrive after the trees) shows it at once, and so does any tree
/// when the glide is 0. The time is the app's [`Time`], so a paused game does not decline.
///
/// Read when a change is seen (the frame it is made, in [`PostUpdate`](bevy::app::PostUpdate)), so
/// the value that counts for a tree is the one the resource has then: a lightning strike that should
/// take a second sets 1 and inserts [`TreeCondition::Dying`] in the same frame.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_flora::{ConditionGlide, TreeCondition};
/// # fn set(mut commands: Commands, tree: Entity) {
/// commands.insert_resource(ConditionGlide { seconds: 40.0 });
/// commands.entity(tree).insert(TreeCondition::Dying); // thins over 40 seconds, not at once
/// # }
/// ```
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct ConditionGlide {
    /// Seconds a change takes to be drawn; 0 or less draws it at once.
    pub seconds: f32,
}

/// What a tree's meshes show of its [`TreeCondition`]: the class the tag's bits say (the game's
/// own, the target of a glide), the level on screen, and whether it is on its way there. On a tree
/// that has a condition, or whose decline is still going out after the component was removed; added
/// by `sync_condition_tags` when a tree with a condition is first seen drawn.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct ConditionShown {
    /// The class the tag's class bits carry: what the game set (healthy once it removed the component).
    pub(crate) class: TreeCondition,
    /// The level on screen, 0 healthy to 1 dying, as the tag says it (the quantised value while gliding).
    pub(crate) level: f32,
    /// A glide is running (`ConditionGliding` is on the entity).
    gliding: bool,
}

impl ConditionShown {
    /// Settled on `class`: its anchor level, no glide.
    fn settled(class: TreeCondition) -> Self {
        Self { class, level: class.level(), gliding: false }
    }

    /// The bits of a mesh's tag above the look's that carry this state.
    pub(crate) fn tag_bits(&self) -> u32 {
        if self.gliding {
            self.class.tag_bits() | GLIDING_BIT | (level_step(self.level) << LEVEL_SHIFT)
        } else {
            self.class.tag_bits()
        }
    }
}

/// The glide of a tree's decline in flight: where it started, where it is going, how long it has run
/// and how long it lasts. On the entity only while it runs, in sparse storage, so a forest of
/// settled trees, ill or well, has nothing to visit each frame.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[component(storage = "SparseSet")]
pub(crate) struct ConditionGliding {
    from: f32,
    to: f32,
    elapsed: f32,
    seconds: f32,
}

impl ConditionGliding {
    /// The level `elapsed` seconds into the glide, and whether it has arrived: a smoothstep from `from`
    /// to `to`.
    fn at(&self) -> (f32, bool) {
        let t = (self.elapsed / self.seconds).clamp(0.0, 1.0);
        let eased = t * t * (3.0 - 2.0 * t);
        (self.from * (1.0 - eased) + self.to * eased, t >= 1.0)
    }
}

/// A level, 0 to 1, as the tag's step (0 to `LEVEL_STEPS`), the nearest.
fn level_step(level: f32) -> u32 {
    (level.clamp(0.0, 1.0) * LEVEL_STEPS).round() as u32
}

/// A tag's step as the level it stands for (`condition_level` in `common.wesl`).
fn step_level(step: u32) -> f32 {
    (step.min(LEVEL_STEPS as u32) as f32) / LEVEL_STEPS
}

/// The share of a crown's leaves a decline at `level` has lost: through the classes' own shares,
/// `Healthy` at 0, `Sick` at a half and `Dying` at 1 (`level_defoliation` in `common.wesl`).
pub(crate) fn defoliation_at(level: f32) -> f32 {
    let level = level.clamp(0.0, 1.0);
    let (sick, dying) = (TreeCondition::Sick.defoliation(), TreeCondition::Dying.defoliation());
    if level <= 0.5 {
        let t = level * 2.0;
        0.0 * (1.0 - t) + sick * t
    } else {
        let t = level * 2.0 - 1.0;
        sick * (1.0 - t) + dying * t
    }
}

/// How far the leaves that stay have withered on average at `level`: the class values at the
/// anchors (0, a half, 1), so linear (`level_withering` in `common.wesl`).
pub(crate) fn withering_at(level: f32) -> f32 {
    level.clamp(0.0, 1.0)
}

/// The condition of a tree, in the three classes of a forest-health survey's crown assessment
/// (`docs/research/tree-condition.md`): a healthy crown, one in decline, one close to death. Ordered
/// from well to ill, so `condition >= TreeCondition::Sick` means "not healthy".
///
/// **Insert it on the tree's entity**, `commands.entity(tree).insert(TreeCondition::Dying)`, and
/// [`FloraIndex`](crate::FloraIndex) shows it from the next [`PostUpdate`](bevy::app::PostUpdate);
/// the tree is drawn in it from then on (see the module's docs). Insert [`TreeCondition::Healthy`] or
/// remove the component to clear it: a tree without one is healthy, so a forest of a hundred
/// thousand pays nothing for the trees that are well. To react to a change, query
/// `Changed<TreeCondition>`.
///
/// The condition belongs to the **tree's entity**, not to a [`TreePool`](crate::TreePool) its meshes
/// come from: a [`PooledTree`](crate::PooledTree) that changes variant or pool, is taken off the entity
/// and put back, or whose pool reloads, keeps it. A tree set before it is indexed (a pooled tree whose
/// pool has not loaded yet) shows it as soon as it is. Despawning the tree takes it with it.
///
/// A felled tree leaves the index ([`Felled`](crate::Felled)): [`FloraIndex::condition_of`] answers
/// `None` for it, and the component stays on its entity as it was when the tree was cut, for a
/// `Query<&TreeCondition>` to read; the felled tree keeps its look as it falls. There is no
/// `Dead` condition: a dead tree is the game's to fell ([`FellTree`](crate::FellTree)), replace, or
/// leave standing.
///
/// The look needs the flora materials ([`FloraMaterialSupport`](crate::FloraMaterialSupport)): a
/// device that draws trees with plain materials (no vertex-stage storage buffers, WebGL2) shows
/// the state and not the look, as it shows no glide. A hull crown ([`CrownStyle::Hull`](crate::CrownStyle))
/// withers but keeps all its facets.
///
/// [`FloraIndex::condition_of`]: crate::FloraIndex::condition_of
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
#[reflect(Component)]
pub enum TreeCondition {
    /// A full crown: the state of a tree with no `TreeCondition`.
    #[default]
    Healthy,
    /// A crown in decline: thinned by [`defoliation`](Self::defoliation), its leaves half way to dry.
    /// The tree is still standing and may recover.
    Sick,
    /// A crown close to death: mostly bare, what leaves it has dry. The class of the survey's
    /// severe grade, ~80% of the crown gone.
    Dying,
}

impl TreeCondition {
    /// The share of a crown's leaves this condition has lost, 0 to 1: nothing, about a third, about
    /// four fifths (the survey's slightly to moderately and its severely declining stands,
    /// `docs/research/tree-condition.md`). Which leaves are lost is the tree's own, the same ones at
    /// every level of detail and in the shadow maps, and the same whatever the tree's age.
    /// Flowers and fruit are not leaves and are not lost.
    pub const fn defoliation(self) -> f32 {
        match self {
            Self::Healthy => 0.0,
            Self::Sick => 0.35,
            Self::Dying => 0.8,
        }
    }

    /// How far the leaves that stay have gone from their colour to a dry ochre-brown, 0 to 1 on
    /// average: each leaf goes its own way, up to 0.4 either side of this (never past all or none).
    pub const fn withering(self) -> f32 {
        match self {
            Self::Healthy => 0.0,
            Self::Sick => 0.5,
            Self::Dying => 1.0,
        }
    }

    /// The bits of a mesh's [`MeshTag`] that carry this condition (`common.wesl`'s `condition_level`
    /// reads them back).
    pub(crate) const fn tag_bits(self) -> u32 {
        (self as u32) << TAG_SHIFT
    }

    /// Where the class sits on the decline's level, 0 to 1: healthy, half way, dying. A decline
    /// between classes is a level between these ([`defoliation_at`], [`withering_at`]).
    pub(crate) const fn level(self) -> f32 {
        match self {
            Self::Healthy => 0.0,
            Self::Sick => 0.5,
            Self::Dying => 1.0,
        }
    }
}

/// `leaf_decline_moment` in `common.wesl`: a leaf's draw for its tree's decline, 0 to 1, from its
/// phase (`sway.w`). A leaf is lost when it is below [`TreeCondition::defoliation`]. It is not the
/// season's moment, so the leaves a sick tree loses are not the ones that would turn first.
pub(crate) fn leaf_decline_moment(phase: f32) -> f32 {
    let x = phase * 4.61 + 0.719;
    x - x.floor()
}

/// A mesh's tag for a pooled look's glide tag `look` and the tree's condition (none: healthy), as it is
/// drawn now if it is (`shown`, which carries the level of a decline on its way).
pub(crate) fn tag_for(look: u32, condition: Option<&TreeCondition>, shown: Option<&ConditionShown>) -> u32 {
    let bits = shown.map(ConditionShown::tag_bits).or_else(|| condition.map(|condition| condition.tag_bits()));
    (look & TAG_LOOK_MASK) | bits.unwrap_or(0)
}

/// What a tree's condition has done to its leaves, for what the CPU draws from the tree's leaf mesh
/// (the leaves that fall from it): `leaf_defoliated` and `leaf_withering` of `common.wesl`,
/// mirrored, so a leaf the shaders have hidden does not fall and a leaf that falls is as dry as
/// those left on the tree (the burst carries how far, the shader the colour:
/// `ATTRIBUTE_FALL_FACE`'s w).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Decline {
    lost: f32,
    withering: f32,
}

impl Decline {
    /// For a tree in `condition` (none: healthy). A hull crown loses none of its facets.
    pub(crate) fn new(condition: Option<&TreeCondition>, hull: bool) -> Self {
        let condition = condition.copied().unwrap_or_default();
        Self { lost: if hull { 0.0 } else { condition.defoliation() }, withering: condition.withering() }
    }

    /// For a tree whose crown shows `level` of decline (`ConditionShown::level`). A hull crown loses
    /// none of its facets.
    pub(crate) fn at(level: f32, hull: bool) -> Self {
        Self { lost: if hull { 0.0 } else { defoliation_at(level) }, withering: withering_at(level) }
    }

    /// Whether the leaf with this phase (`sway.w`) is one the tree has lost.
    pub(crate) fn has_lost(&self, phase: f32) -> bool {
        self.lost > 0.0 && leaf_decline_moment(phase) < self.lost
    }

    /// How far the leaf with this phase has withered, 0 (its colour) to 1 (the dry leaf colour).
    pub(crate) fn withered(&self, phase: f32) -> f32 {
        if self.withering == 0.0 {
            return 0.0;
        }
        let x = phase * 2.83 + 0.37;
        (self.withering + 0.8 * ((x - x.floor()) - 0.5)).clamp(0.0, 1.0)
    }
}

/// What a tree's drawing does when its [`TreeCondition`] is seen to be `to`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Begin {
    /// Nothing: the tree already shows, or is on its way to, `to`.
    Keep,
    /// Show `to` at once.
    Settle,
    /// Glide from this level to `to`'s.
    Glide(f32),
}

/// Decide what `to` does to a tree that shows `shown` (none: not drawn with a condition yet). A tree
/// seen for the first time glides, from healthy, only if it was `standing` before the condition was
/// set (a tree that appears with its condition shows it); a glide takes `seconds`, 0 or less is at once.
fn begin(shown: Option<&ConditionShown>, to: TreeCondition, seconds: f32, standing: bool) -> Begin {
    match shown {
        Some(shown) if shown.class == to => Begin::Keep,
        Some(shown) if seconds > 0.0 => Begin::Glide(shown.level),
        None if standing && to != TreeCondition::Healthy && seconds > 0.0 => Begin::Glide(0.0),
        _ => Begin::Settle,
    }
}

/// Put `bits` (a condition's, above the look's) on every mesh of a tree, leaving the look's tag as it
/// is; a mesh with no tag gets one only when there is something to say.
fn show(commands: &mut Commands, meshes: &mut Query<Option<&mut MeshTag>, With<Mesh3d>>, children: &Children, bits: u32) {
    for child in children.iter() {
        match meshes.get_mut(child) {
            Ok(Some(mut tag)) => {
                let value = (tag.value & TAG_LOOK_MASK) | bits;
                if tag.value != value {
                    tag.value = value;
                }
            }
            Ok(None) if bits != 0 => {
                commands.entity(child).insert(MeshTag::new(bits));
            }
            _ => {}
        }
    }
}

/// Put each tree's condition in the [`MeshTag`] of its meshes: when the component is added or
/// changed, and when the tree's children are (a rebuild, a new level of detail, a pool's spawn).
/// The bits below the condition are left as they are (a pooled look's glide tag). A tree that
/// lost the component, or was set healthy, is cleared, or recovers over [`ConditionGlide`]'s seconds.
/// A change of class begins a glide (`advance_condition_glides` runs it). Only trees that have the
/// component are visited, so a forest of healthy trees pays nothing; a tree whose meshes are tagged
/// by what spawned them ([`tag_for`]) is found already right.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_condition_tags(
    mut commands: Commands,
    settings: Res<ConditionGlide>,
    ill: Query<(Entity, Ref<TreeCondition>, Ref<Children>, Option<&ConditionShown>), Or<(Changed<TreeCondition>, Changed<Children>)>>,
    mut removed: RemovedComponents<TreeCondition>,
    standing: Query<(&Children, Option<&ConditionShown>)>,
    has: Query<(), With<TreeCondition>>,
    mut meshes: Query<Option<&mut MeshTag>, With<Mesh3d>>,
) {
    // The component taken off first, so a removal and an insertion in one frame end at the insertion.
    for tree in removed.read() {
        if has.contains(tree) {
            continue;
        }
        let Ok((children, shown)) = standing.get(tree) else {
            if let Ok(mut entity) = commands.get_entity(tree) {
                entity.remove::<(ConditionShown, ConditionGliding)>();
            }
            continue;
        };
        match shown {
            // Recovering: the leaves come back over the glide.
            Some(shown) if shown.level > 0.0 && settings.seconds > 0.0 => {
                let now = ConditionShown { class: TreeCondition::Healthy, level: step_level(level_step(shown.level)), gliding: true };
                commands
                    .entity(tree)
                    .insert((now, ConditionGliding { from: shown.level, to: 0.0, elapsed: 0.0, seconds: settings.seconds }));
                show(&mut commands, &mut meshes, children, now.tag_bits());
            }
            _ => {
                commands.entity(tree).remove::<(ConditionShown, ConditionGliding)>();
                show(&mut commands, &mut meshes, children, 0);
            }
        }
    }
    for (tree, condition, children, shown) in &ill {
        let to = *condition;
        let how = if condition.is_changed() || shown.is_none() {
            // Standing before the condition was set: the tree's meshes were not added this frame.
            begin(shown, to, settings.seconds, condition.is_changed() && !children.is_added())
        } else {
            Begin::Keep
        };
        let now = match how {
            Begin::Keep => shown.copied().unwrap_or_else(|| ConditionShown::settled(to)),
            Begin::Settle => ConditionShown::settled(to),
            Begin::Glide(from) => ConditionShown { class: to, level: step_level(level_step(from)), gliding: true },
        };
        match how {
            Begin::Keep => {}
            Begin::Settle => {
                commands.entity(tree).insert(now).remove::<ConditionGliding>();
            }
            Begin::Glide(from) => {
                commands
                    .entity(tree)
                    .insert((now, ConditionGliding { from, to: to.level(), elapsed: 0.0, seconds: settings.seconds }));
            }
        }
        show(&mut commands, &mut meshes, &children, now.tag_bits());
    }
}

/// Run the glides that are in flight: the level on screen of every gliding tree moves along its smoothstep
/// by the frame's time, and a tree's meshes are tagged again when the level moves a step of the
/// tag's 255 (about every 80 ms over a 20 s glide). A glide that has arrived is the class's own tag
/// again, bit for bit; a recovery whose component is gone leaves the tree untagged. Only trees that
/// glide are visited.
pub(crate) fn advance_condition_glides(
    mut commands: Commands,
    time: Res<Time>,
    mut gliding: Query<(Entity, &mut ConditionGliding, &mut ConditionShown, Option<&Children>, Has<TreeCondition>)>,
    mut meshes: Query<Option<&mut MeshTag>, With<Mesh3d>>,
) {
    let delta = time.delta_secs();
    for (tree, mut glide, mut shown, children, has_condition) in &mut gliding {
        glide.elapsed += delta;
        let (level, arrived) = glide.at();
        let bits = if arrived {
            if has_condition {
                *shown = ConditionShown::settled(shown.class);
                commands.entity(tree).remove::<ConditionGliding>();
                shown.tag_bits()
            } else {
                commands.entity(tree).remove::<(ConditionShown, ConditionGliding)>();
                0
            }
        } else {
            let level = step_level(level_step(level));
            if level == shown.level {
                continue;
            }
            shown.level = level;
            shown.tag_bits()
        };
        if let Some(children) = children {
            show(&mut commands, &mut meshes, children, bits);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FellTree, FloraIndex, IndexedTree, LodSettings, PoolColors, PooledLook, PooledTree, ProceduralTree, RecolorTree, TreeLod, TreePool,
        TreePreset,
        build_pool,
    };
    use crate::felling::Felled;
    use crate::shading::FloraMaterial;
    use bevy::app::App;
    use bevy::asset::{AssetApp, AssetId, AssetPlugin, Assets};
    use bevy::color::Color;
    use bevy::math::Vec2;
    use bevy::mesh::Mesh;
    use bevy::pbr::{MeshMaterial3d, StandardMaterial};
    use bevy::time::TimeUpdateStrategy;
    use bevy::transform::components::Transform;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((bevy::MinimalPlugins, AssetPlugin::default(), bevy::transform::TransformPlugin));
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_plugins(crate::FloraPlugin);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 30.0)));
        app
    }

    /// A pool of two variants of `preset`, grown, in the app's assets.
    fn pool(app: &mut App, preset: TreePreset) -> bevy::asset::Handle<TreePool> {
        let built = build_pool(&preset.params(), 2, f32::INFINITY, &LodSettings::default(), &mut app.world_mut().resource_mut::<Assets<Mesh>>());
        app.world_mut().resource_mut::<Assets<TreePool>>().add(built)
    }

    fn condition_of(app: &App, tree: Entity) -> Option<TreeCondition> {
        app.world().resource::<FloraIndex>().condition_of(tree)
    }

    fn settle(app: &mut App) {
        app.update();
        app.update();
    }

    #[test]
    fn a_condition_set_on_a_tree_reads_back_through_the_index_as_set_changed_and_cleared() {
        let mut app = app();
        let oak = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), Transform::from_xyz(10.0, 0.0, 0.0))).id();
        let birch = app.world_mut().spawn((ProceduralTree(TreePreset::Birch.params()), Transform::from_xyz(-10.0, 0.0, 0.0))).id();
        let handle = pool(&mut app, TreePreset::Pine);
        let pine = app.world_mut().spawn((PooledTree { pool: handle, variant: 1 }, Transform::from_xyz(0.0, 0.0, 20.0))).id();
        settle(&mut app);
        for tree in [oak, birch, pine] {
            assert_eq!(condition_of(&app, tree), Some(TreeCondition::Healthy), "a tree with no condition is healthy");
        }
        let before = *app.world().resource::<FloraIndex>().get(oak).unwrap();

        // Set on a procedural tree and on a pooled one; the third is left alone.
        app.world_mut().entity_mut(oak).insert(TreeCondition::Sick);
        app.world_mut().entity_mut(pine).insert(TreeCondition::Dying);
        settle(&mut app);
        assert_eq!(condition_of(&app, oak), Some(TreeCondition::Sick));
        assert_eq!(condition_of(&app, pine), Some(TreeCondition::Dying));
        assert_eq!(condition_of(&app, birch), Some(TreeCondition::Healthy));
        let index = app.world().resource::<FloraIndex>();
        assert_eq!(index.get(pine).unwrap().condition, TreeCondition::Dying);
        // The condition is the only thing it changed about the oak.
        assert_eq!(IndexedTree { condition: TreeCondition::Healthy, ..*index.get(oak).unwrap() }, before);
        // The queries hand back the condition with the tree.
        assert_eq!(index.nearest(Vec2::new(0.5, 20.0), 2.0).map(|t| (t.entity, t.condition)), Some((pine, TreeCondition::Dying)));
        let ill: Vec<Entity> = index.iter().filter(|t| t.condition >= TreeCondition::Sick).map(|t| t.entity).collect();
        assert_eq!(ill.len(), 2);
        assert!(ill.contains(&oak) && ill.contains(&pine));
        let near_the_oak: Vec<_> = index.within(Vec2::new(10.0, 0.0), 3.0).map(|t| (t.entity, t.condition)).collect();
        assert_eq!(near_the_oak, vec![(oak, TreeCondition::Sick)]);

        // Moved and regrown from another seed, its entry is made again: it is still sick.
        app.world_mut().get_mut::<Transform>(oak).unwrap().translation.x = 12.0;
        app.world_mut().get_mut::<ProceduralTree>(oak).unwrap().0.seed = 9;
        settle(&mut app);
        let moved = *app.world().resource::<FloraIndex>().get(oak).unwrap();
        assert!((moved.base.x - 12.0).abs() < 1e-3, "the oak stands at x = {}", moved.base.x);
        assert_eq!(moved.condition, TreeCondition::Sick, "a moved and rebuilt tree");

        // Changed, then cleared by inserting Healthy, and by taking the component off.
        app.world_mut().entity_mut(oak).insert(TreeCondition::Dying);
        app.world_mut().entity_mut(pine).insert(TreeCondition::Healthy);
        settle(&mut app);
        assert_eq!(condition_of(&app, oak), Some(TreeCondition::Dying));
        assert_eq!(condition_of(&app, pine), Some(TreeCondition::Healthy));
        app.world_mut().entity_mut(oak).remove::<TreeCondition>();
        settle(&mut app);
        assert_eq!(condition_of(&app, oak), Some(TreeCondition::Healthy), "no component, a healthy tree");

        // Taken off and put back in one frame, it ends at what was put back.
        app.world_mut().entity_mut(oak).insert(TreeCondition::Sick);
        settle(&mut app);
        app.world_mut().entity_mut(oak).remove::<TreeCondition>();
        app.world_mut().entity_mut(oak).insert(TreeCondition::Dying);
        settle(&mut app);
        assert_eq!(condition_of(&app, oak), Some(TreeCondition::Dying));

        // A despawned tree is gone with its condition.
        app.world_mut().despawn(oak);
        settle(&mut app);
        assert_eq!(condition_of(&app, oak), None);
    }

    #[test]
    fn a_pooled_trees_condition_is_its_own_and_survives_what_the_pool_does() {
        let mut app = app();
        let handle = pool(&mut app, TreePreset::Pine);
        let spawn = |app: &mut App, x: f32| {
            app.world_mut().spawn((PooledTree { pool: handle.clone(), variant: 0 }, Transform::from_xyz(x, 0.0, 0.0))).id()
        };
        let (marked, neighbour) = (spawn(&mut app, 0.0), spawn(&mut app, 20.0));
        app.world_mut().entity_mut(marked).insert(TreeCondition::Dying);
        settle(&mut app);
        // Per instance: the pool's other tree, from the same variant, is untouched.
        assert_eq!(condition_of(&app, marked), Some(TreeCondition::Dying));
        assert_eq!(condition_of(&app, neighbour), Some(TreeCondition::Healthy));

        // A change of variant re-indexes the tree; it is still dying.
        app.world_mut().get_mut::<PooledTree>(marked).unwrap().variant = 1;
        settle(&mut app);
        assert_eq!(condition_of(&app, marked), Some(TreeCondition::Dying), "after a change of variant");

        // The pool reloads: every tree of it is indexed again; the condition is read again.
        let _ = app.world_mut().resource_mut::<Assets<TreePool>>().get_mut(&handle);
        settle(&mut app);
        assert_eq!(condition_of(&app, marked), Some(TreeCondition::Dying), "after the pool was modified");
        assert_eq!(condition_of(&app, neighbour), Some(TreeCondition::Healthy));

        // Taken out of its pool and put back (a game's own recycle): out of the index in between, and
        // the condition on the entity all the while.
        let pooled = app.world_mut().entity_mut(marked).take::<PooledTree>().unwrap();
        settle(&mut app);
        assert_eq!(condition_of(&app, marked), None);
        assert_eq!(app.world().get::<TreeCondition>(marked), Some(&TreeCondition::Dying));
        app.world_mut().entity_mut(marked).insert(pooled);
        settle(&mut app);
        assert_eq!(condition_of(&app, marked), Some(TreeCondition::Dying), "after it was put back");
    }

    #[test]
    fn a_condition_set_before_the_tree_is_indexed_applies_when_it_is() {
        let mut app = app();
        // A pool that does not exist yet: its handle is reserved, the trees that use it wait.
        let handle = app.world().resource::<Assets<TreePool>>().reserve_handle();
        let tree = app
            .world_mut()
            .spawn((PooledTree { pool: handle.clone(), variant: 0 }, TreeCondition::Sick, Transform::from_xyz(4.0, 0.0, 4.0)))
            .id();
        settle(&mut app);
        assert_eq!(condition_of(&app, tree), None, "no pool, no trunk to index");
        // Changed while it waits: the change is not dropped.
        app.world_mut().entity_mut(tree).insert(TreeCondition::Dying);
        settle(&mut app);
        assert_eq!(condition_of(&app, tree), None);

        let built = build_pool(&TreePreset::Pine.params(), 1, f32::INFINITY, &LodSettings::default(), &mut app.world_mut().resource_mut::<Assets<Mesh>>());
        app.world_mut().resource_mut::<Assets<TreePool>>().insert(handle.id(), built).unwrap();
        settle(&mut app);
        assert_eq!(condition_of(&app, tree), Some(TreeCondition::Dying), "it shows what was last set");
    }

    #[test]
    fn a_felled_trees_condition_does_not_panic_and_stays_on_its_entity() {
        let mut app = app();
        let handle = pool(&mut app, TreePreset::Pine);
        let oak = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeCondition::Sick, Transform::default())).id();
        let pine = app
            .world_mut()
            .spawn((PooledTree { pool: handle, variant: 0 }, TreeCondition::Dying, Transform::from_xyz(30.0, 0.0, 0.0)))
            .id();
        settle(&mut app);
        assert_eq!((condition_of(&app, oak), condition_of(&app, pine)), (Some(TreeCondition::Sick), Some(TreeCondition::Dying)));

        for tree in [oak, pine] {
            app.world_mut().entity_mut(tree).insert(FellTree { speed: 6.0, ..Default::default() });
        }
        // Set while the tree falls, in the frame it is cut, and at rest: none of it may panic.
        for frame in 0..400 {
            if frame == 3 {
                app.world_mut().entity_mut(oak).insert(TreeCondition::Dying);
            }
            app.update();
        }
        for tree in [oak, pine] {
            assert!(app.world().get::<Felled>(tree).is_some_and(|f| f.at_rest), "the tree did not come to rest");
            assert_eq!(condition_of(&app, tree), None, "a felled tree is not in the index");
        }
        // What it was when it was cut stays on the entity, a pooled tree's included (it left its pool).
        assert_eq!(app.world().get::<TreeCondition>(oak), Some(&TreeCondition::Dying));
        assert_eq!(app.world().get::<TreeCondition>(pine), Some(&TreeCondition::Dying));
        // Set, changed and removed on the lying tree.
        app.world_mut().entity_mut(oak).insert(TreeCondition::Healthy);
        app.world_mut().entity_mut(pine).remove::<TreeCondition>();
        settle(&mut app);
        assert_eq!(app.world().get::<TreeCondition>(oak), Some(&TreeCondition::Healthy));
        assert!(app.world().get::<TreeCondition>(pine).is_none());
        assert_eq!(app.world().resource::<FloraIndex>().len(), 0);
    }

    #[test]
    fn a_recolour_and_a_condition_in_one_frame_both_apply_and_equal_looks_still_share_a_material() {
        const BLUE: PoolColors =
            PoolColors { wood: Color::srgb(0.10, 0.20, 0.60), leaves: Color::srgb(0.30, 0.55, 0.95), unlit: false };
        // Two oaks recoloured alike; with `condition` the first is also marked in the same frame.
        // Returns the app and the two trees.
        let run = |condition: bool| {
            let mut app = app();
            app.insert_resource(crate::FloraMaterialSupport);
            let trees: Vec<Entity> = (0..2)
                .map(|i| {
                    let at = Transform::from_xyz(15.0 * i as f32, 0.0, 0.0);
                    app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeLod::default(), at)).id()
                })
                .collect();
            settle(&mut app);
            for &tree in &trees {
                app.world_mut().write_message(RecolorTree { tree, colors: BLUE });
            }
            if condition {
                app.world_mut().entity_mut(trees[0]).insert(TreeCondition::Dying);
            }
            settle(&mut app);
            (app, trees)
        };
        let materials = |app: &App, tree: Entity| -> Vec<(AssetId<FloraMaterial>, Color)> {
            let children = app.world().get::<bevy::ecs::hierarchy::Children>(tree).expect("a built tree has children");
            let assets = app.world().resource::<Assets<FloraMaterial>>();
            let mut found = Vec::new();
            for child in children.iter() {
                if let Some(handle) = app.world().get::<MeshMaterial3d<FloraMaterial>>(child) {
                    let material = assets.get(&handle.0).expect("a live material");
                    let wanted = if material.extension.leaf { BLUE.leaves } else { BLUE.wood };
                    found.push((handle.0.id(), wanted));
                    assert_eq!(material.base.base_color, wanted, "the recolour applied (leaf: {})", material.extension.leaf);
                }
            }
            found
        };

        let (control, _) = run(false);
        let (app, trees) = run(true);
        let (marked, other) = (materials(&app, trees[0]), materials(&app, trees[1]));
        assert!(marked.len() >= 6, "every level of detail of the marked tree is recoloured");
        assert_eq!(condition_of(&app, trees[0]), Some(TreeCondition::Dying));
        assert_eq!(condition_of(&app, trees[1]), Some(TreeCondition::Healthy));
        // The two trees still hold the very same materials, and the world holds no more of them than
        // the recolour alone makes.
        assert_eq!(marked, other, "trees of equal colours share their materials, marked or not");
        let live = |app: &App| app.world().resource::<Assets<FloraMaterial>>().len();
        assert_eq!(live(&app), live(&control), "a condition made no material of its own");
    }

    /// The tags of every mesh a tree is drawn with (a mesh with none counts 0).
    fn tags(app: &App, tree: Entity) -> Vec<u32> {
        let world = app.world();
        let Some(children) = world.get::<Children>(tree) else { return Vec::new() };
        children
            .iter()
            .filter(|&child| world.get::<Mesh3d>(child).is_some())
            .map(|child| world.get::<MeshTag>(child).map_or(0, |tag| tag.value))
            .collect()
    }

    #[test]
    fn the_meshes_of_a_tree_carry_its_condition_as_set_changed_rebuilt_and_cleared() {
        let mut app = app();
        let oak = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeLod::default())).id();
        let other = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), Transform::from_xyz(20.0, 0.0, 0.0))).id();
        settle(&mut app);
        let all = |app: &App, tree: Entity, bits: u32| {
            let found = tags(app, tree);
            !found.is_empty() && found.iter().all(|&tag| tag == bits)
        };
        assert!(tags(&app, oak).len() >= 6, "wood and leaves at every level of detail");
        assert!(all(&app, oak, 0) && all(&app, other, 0), "a tree with no condition has untagged meshes");

        app.world_mut().entity_mut(oak).insert(TreeCondition::Sick);
        settle(&mut app);
        assert!(all(&app, oak, TreeCondition::Sick.tag_bits()));
        assert!(all(&app, other, 0), "no other tree is touched");

        app.world_mut().entity_mut(oak).insert(TreeCondition::Dying);
        settle(&mut app);
        assert!(all(&app, oak, TreeCondition::Dying.tag_bits()));

        // Regrown from another seed, its meshes are made again: they are still dying.
        app.world_mut().get_mut::<ProceduralTree>(oak).unwrap().0.seed = 9;
        settle(&mut app);
        assert!(all(&app, oak, TreeCondition::Dying.tag_bits()), "{:?}", tags(&app, oak));

        // Healthy, and the component taken off, both clear it; taken off and put back in one frame
        // ends at the put back.
        app.world_mut().entity_mut(oak).insert(TreeCondition::Healthy);
        settle(&mut app);
        assert!(all(&app, oak, 0));
        app.world_mut().entity_mut(oak).insert(TreeCondition::Dying);
        settle(&mut app);
        app.world_mut().entity_mut(oak).remove::<TreeCondition>();
        settle(&mut app);
        assert!(all(&app, oak, 0));
        app.world_mut().entity_mut(oak).insert(TreeCondition::Sick);
        settle(&mut app);
        app.world_mut().entity_mut(oak).remove::<TreeCondition>().insert(TreeCondition::Dying);
        settle(&mut app);
        assert!(all(&app, oak, TreeCondition::Dying.tag_bits()));
    }

    #[test]
    fn a_pooled_trees_meshes_keep_their_looks_tag_beside_the_condition() {
        const RED: PoolColors = PoolColors { wood: Color::srgb(0.3, 0.1, 0.1), leaves: Color::srgb(0.8, 0.1, 0.1), unlit: false };
        let mut app = app();
        let mut built = build_pool(&TreePreset::Pine.params(), 2, f32::INFINITY, &LodSettings::default(), &mut app.world_mut().resource_mut::<Assets<Mesh>>());
        built.add_look(RED);
        let handle = app.world_mut().resource_mut::<Assets<TreePool>>().add(built);
        let (a, b) = (PooledTree { pool: handle.clone(), variant: 0 }, PooledTree { pool: handle, variant: 1 });
        // Sick from the start (the tag is made with the meshes), and not at all.
        let sick = app.world_mut().spawn((a, TreeCondition::Sick)).id();
        let well = app.world_mut().spawn((b, Transform::from_xyz(9.0, 0.0, 0.0))).id();
        settle(&mut app);
        let (look, bits) = (|tag: u32| tag & TAG_LOOK_MASK, |tag: u32| tag & !TAG_LOOK_MASK);
        let first = tags(&app, sick);
        assert!(first.len() >= 6 && first.iter().all(|&tag| bits(tag) == TreeCondition::Sick.tag_bits()), "{first:?}");
        let glide = look(first[0]);
        assert!(glide != 0 && first.iter().all(|&tag| look(tag) == glide), "the look's tag is still there: {first:?}");
        assert!(tags(&app, well).iter().all(|&tag| bits(tag) == 0));

        // Another look: the tag of that look, still sick.
        app.world_mut().entity_mut(sick).insert(PooledLook(1));
        settle(&mut app);
        let moved = tags(&app, sick);
        let glide_now = look(moved[0]);
        assert!(glide_now != 0 && glide_now != glide, "the other look's tag: {moved:?}");
        assert!(moved.iter().all(|&tag| look(tag) == glide_now && bits(tag) == TreeCondition::Sick.tag_bits()), "{moved:?}");

        // Worse, then well: the look's tag stays.
        app.world_mut().entity_mut(sick).insert(TreeCondition::Dying);
        settle(&mut app);
        assert!(tags(&app, sick).iter().all(|&tag| look(tag) == glide_now && bits(tag) == TreeCondition::Dying.tag_bits()));
        app.world_mut().entity_mut(sick).remove::<TreeCondition>();
        settle(&mut app);
        assert!(tags(&app, sick).iter().all(|&tag| tag == glide_now), "{:?}", tags(&app, sick));
    }

    #[test]
    fn a_declined_leaf_is_lost_by_its_own_draw_and_withers_by_its_class() {
        let healthy = Decline::new(None, false);
        let sick = Decline::new(Some(&TreeCondition::Sick), false);
        let dying = Decline::new(Some(&TreeCondition::Dying), false);
        let hull = Decline::new(Some(&TreeCondition::Dying), true);
        let phases: Vec<f32> = (0..4000).map(|k| k as f32 * 0.00157).collect();
        let lost = |decline: &Decline| phases.iter().filter(|&&p| decline.has_lost(p)).count() as f32 / phases.len() as f32;
        assert_eq!(lost(&healthy), 0.0);
        assert!((lost(&sick) - TreeCondition::Sick.defoliation()).abs() < 0.03, "{}", lost(&sick));
        assert!((lost(&dying) - TreeCondition::Dying.defoliation()).abs() < 0.03, "{}", lost(&dying));
        assert_eq!(lost(&hull), 0.0, "a hull crown keeps its facets");
        // The ones a sick tree loses are among those a dying one has lost.
        assert!(phases.iter().all(|&p| !sick.has_lost(p) || dying.has_lost(p)));

        // A healthy leaf has not withered; the others have, by their class's share on average, a
        // dying tree's further than a sick one's, none past the dry colour.
        assert!(phases.iter().all(|&p| healthy.withered(p) == 0.0));
        let mean = |decline: &Decline| phases.iter().map(|&p| decline.withered(p)).sum::<f32>() / phases.len() as f32;
        assert!((mean(&sick) - TreeCondition::Sick.withering()).abs() < 0.03, "{}", mean(&sick));
        assert!(mean(&dying) > 0.85 && mean(&dying) <= 1.0, "{}", mean(&dying));
        assert!(phases.iter().all(|&p| (0.0..=1.0).contains(&sick.withered(p)) && (0.0..=1.0).contains(&dying.withered(p))));
        // Each leaf of a sick tree goes its own way: not all alike.
        let spread = phases.iter().map(|&p| sick.withered(p)).fold((1.0f32, 0.0f32), |(lo, hi), t| (lo.min(t), hi.max(t)));
        assert!(spread.1 - spread.0 > 0.6, "{spread:?}");
    }

    /// The level a mesh's tag carries (its class's if no glide runs) and whether a glide runs.
    fn tag_level(tag: u32) -> (u32, bool) {
        if tag & GLIDING_BIT != 0 {
            ((tag >> LEVEL_SHIFT) & 255, true)
        } else {
            (((tag >> TAG_SHIFT) & 3) * 127, false)
        }
    }

    /// An app whose glides last `seconds`, with an oak standing and drawn.
    fn gliding_oak(seconds: f32) -> (App, Entity) {
        let mut app = app();
        app.world_mut().insert_resource(ConditionGlide { seconds });
        let oak = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeLod::default())).id();
        settle(&mut app);
        assert!(tags(&app, oak).len() >= 6, "the oak is drawn");
        (app, oak)
    }

    /// The first mesh's tag, and whether all the tree's meshes agree on it.
    fn tag_of(app: &App, tree: Entity) -> u32 {
        let found = tags(app, tree);
        assert!(found.iter().all(|&tag| tag == found[0]), "every mesh of a tree says the same: {found:?}");
        found[0]
    }

    #[test]
    fn the_levels_pass_through_the_classes_values_and_are_monotone_between() {
        for class in [TreeCondition::Healthy, TreeCondition::Sick, TreeCondition::Dying] {
            assert_eq!(defoliation_at(class.level()), class.defoliation(), "{class:?}'s share at its own level");
            assert_eq!(withering_at(class.level()), class.withering(), "{class:?}'s withering at its own level");
            assert_eq!(step_level(level_step(class.level())), class.level(), "{class:?}'s level survives the tag's steps exactly");
        }
        let steps: Vec<(f32, f32)> = (0..=254).map(|k| (defoliation_at(step_level(k)), withering_at(step_level(k)))).collect();
        assert!(steps.windows(2).all(|w| w[1].0 >= w[0].0 && w[1].1 >= w[0].1), "no step takes leaves back or rehydrates them");
        // Out-of-range levels are clamped, not extrapolated.
        assert_eq!((defoliation_at(-1.0), defoliation_at(2.0)), (0.0, TreeCondition::Dying.defoliation()));
    }

    #[test]
    fn a_declined_crown_at_a_level_has_lost_the_leaves_a_worse_level_loses_and_more() {
        let phases: Vec<f32> = (0..2000).map(|i| i as f32 * 0.37 + 0.011).collect();
        let levels = [0.0, 0.1, 0.25, 0.5, 0.75, 1.0];
        for pair in levels.windows(2) {
            let (better, worse) = (Decline::at(pair[0], false), Decline::at(pair[1], false));
            assert!(phases.iter().all(|&p| !better.has_lost(p) || worse.has_lost(p)), "{pair:?}: a lost leaf stays lost as the level rises");
        }
        let share = |level: f32| {
            let decline = Decline::at(level, false);
            phases.iter().filter(|&&p| decline.has_lost(p)).count() as f32 / phases.len() as f32
        };
        for level in levels {
            assert!((share(level) - defoliation_at(level)).abs() < 0.03, "{level}: {}", share(level));
        }
        assert!(Decline::at(0.7, true).lost == 0.0, "a hull crown keeps its facets at every level");
    }

    #[test]
    fn a_change_of_condition_glides_to_its_class_and_arrives_as_the_class_tag() {
        let (mut app, oak) = gliding_oak(2.0);
        assert_eq!(tag_of(&app, oak), 0);
        app.world_mut().entity_mut(oak).insert(TreeCondition::Dying);
        let mut levels = Vec::new();
        for frame in 0..90 {
            app.update();
            let tag = tag_of(&app, oak);
            // What a game reads does not wait for the picture.
            assert_eq!(condition_of(&app, oak), Some(TreeCondition::Dying), "frame {frame}");
            assert_eq!(tag & !TAG_LOOK_MASK & (3 << TAG_SHIFT), TreeCondition::Dying.tag_bits(), "the class bits say the class at once");
            levels.push(tag_level(tag));
        }
        // Starts where the tree was, rises by smoothstep, never falls, and every step is small.
        assert!(levels[0].1 && levels[0].0 <= 2, "starts from healthy: {:?}", levels[0]);
        assert!(levels.windows(2).all(|w| w[1].0 >= w[0].0), "monotone: {levels:?}");
        assert!(levels.windows(2).all(|w| w[1].0 - w[0].0 <= 12), "no frame moves it by more than a few percent: {levels:?}");
        // Two seconds at 30 frames a second: half way is half way (smoothstep(0.5) is 0.5).
        let halfway = levels[30].0;
        assert!((113..=141).contains(&halfway), "half way through, half way up: {halfway}");
        // Arrived: exactly what a tree set at once carries, and the glide is gone.
        assert_eq!(tag_of(&app, oak), TreeCondition::Dying.tag_bits());
        assert!(app.world().get::<ConditionGliding>(oak).is_none());
    }

    #[test]
    fn a_change_in_the_middle_of_a_glide_starts_where_the_tree_is_and_a_recovery_glides_back() {
        let (mut app, oak) = gliding_oak(3.0);
        app.world_mut().entity_mut(oak).insert(TreeCondition::Dying);
        for _ in 0..63 {
            app.update();
        }
        let (before, _) = tag_level(tag_of(&app, oak));
        assert!(before > 150 && before < 235, "most of the way to dying: {before}");
        // Changed to Sick (level 0.5, 127) from past it: the leaves come back from where they are, no jump.
        app.world_mut().entity_mut(oak).insert(TreeCondition::Sick);
        app.update();
        let (after, gliding) = tag_level(tag_of(&app, oak));
        assert!(gliding && after.abs_diff(before) <= 6, "continuous across the change: {before} then {after}");
        for _ in 0..120 {
            app.update();
        }
        assert_eq!(tag_of(&app, oak), TreeCondition::Sick.tag_bits(), "ends settled on the new class");

        // Healthy again: the leaves come back, over the glide, to no bits at all.
        app.world_mut().entity_mut(oak).insert(TreeCondition::Healthy);
        let mut levels = Vec::new();
        for _ in 0..120 {
            app.update();
            levels.push(tag_level(tag_of(&app, oak)).0);
        }
        assert!(levels[0] >= 120 && levels.windows(2).all(|w| w[1] <= w[0]), "recovering: {levels:?}");
        assert_eq!(tag_of(&app, oak), 0, "recovered");

        // A removed component recovers the same way and leaves nothing behind.
        app.world_mut().entity_mut(oak).insert(TreeCondition::Dying);
        for _ in 0..120 {
            app.update();
        }
        assert_eq!(tag_of(&app, oak), TreeCondition::Dying.tag_bits());
        app.world_mut().entity_mut(oak).remove::<TreeCondition>();
        app.update();
        let (start, gliding) = tag_level(tag_of(&app, oak));
        assert!(gliding && start >= 240, "starts from where it was: {start}");
        for _ in 0..120 {
            app.update();
        }
        assert_eq!(tag_of(&app, oak), 0);
        assert!(app.world().get::<ConditionShown>(oak).is_none() && app.world().get::<ConditionGliding>(oak).is_none());
    }

    #[test]
    fn a_tree_that_appears_with_its_condition_shows_it_at_once_and_so_does_a_glide_of_nothing() {
        let mut app = app();
        app.world_mut().insert_resource(ConditionGlide { seconds: 30.0 });
        let loaded = app.world_mut().spawn((ProceduralTree(TreePreset::Oak.params()), TreeCondition::Dying, TreeLod::default())).id();
        let handle = pool(&mut app, TreePreset::Pine);
        let pooled = app.world_mut().spawn((PooledTree { pool: handle, variant: 0 }, TreeCondition::Sick, Transform::from_xyz(20.0, 0.0, 0.0))).id();
        settle(&mut app);
        assert_eq!(tag_of(&app, loaded), TreeCondition::Dying.tag_bits(), "a procedural tree that appears dying");
        assert_eq!(tag_of(&app, pooled) & !TAG_LOOK_MASK, TreeCondition::Sick.tag_bits(), "a pooled one that appears sick");
        assert!(app.world().get::<ConditionGliding>(loaded).is_none() && app.world().get::<ConditionGliding>(pooled).is_none());

        // A glide of nothing is a change at once, as it has always been.
        app.world_mut().insert_resource(ConditionGlide { seconds: 0.0 });
        app.world_mut().entity_mut(loaded).insert(TreeCondition::Healthy);
        settle(&mut app);
        assert_eq!(tag_of(&app, loaded), 0);
    }

    #[test]
    fn a_pooled_trees_glide_survives_a_look_change_and_a_rebuild_keeps_it() {
        const RED: PoolColors = PoolColors { wood: Color::srgb(0.3, 0.1, 0.1), leaves: Color::srgb(0.8, 0.1, 0.1), unlit: false };
        let mut app = app();
        app.world_mut().insert_resource(ConditionGlide { seconds: 4.0 });
        let mut built = build_pool(&TreePreset::Pine.params(), 2, f32::INFINITY, &LodSettings::default(), &mut app.world_mut().resource_mut::<Assets<Mesh>>());
        built.add_look(RED);
        let handle = app.world_mut().resource_mut::<Assets<TreePool>>().add(built);
        let tree = app.world_mut().spawn((PooledTree { pool: handle, variant: 0 }, Transform::default())).id();
        settle(&mut app);
        app.world_mut().entity_mut(tree).insert(TreeCondition::Dying);
        for _ in 0..45 {
            app.update();
        }
        let (level, gliding) = tag_level(tag_of(&app, tree));
        assert!(gliding && level > 20 && level < 235, "gliding: {level}");

        // Another look in the middle of it: the look's tag, the glide's bits.
        app.world_mut().entity_mut(tree).insert(PooledLook(1));
        app.update();
        let tag = tag_of(&app, tree);
        let (now, gliding) = tag_level(tag);
        assert!(tag & TAG_LOOK_MASK != 0, "the other look's tag: {tag:#x}");
        assert!(gliding && now.abs_diff(level) <= 6, "the glide went on through the look change: {level} then {now}");
        assert_eq!(tag & (3 << TAG_SHIFT), TreeCondition::Dying.tag_bits());
        for _ in 0..180 {
            app.update();
        }
        assert_eq!(tag_of(&app, tree) & !TAG_LOOK_MASK, TreeCondition::Dying.tag_bits(), "arrived");
    }

    #[test]
    fn a_pause_stops_a_glide_and_a_settled_tree_has_none_running() {
        let (mut app, oak) = gliding_oak(2.0);
        app.world_mut().entity_mut(oak).insert(TreeCondition::Dying);
        for _ in 0..20 {
            app.update();
        }
        let (level, _) = tag_level(tag_of(&app, oak));
        // A paused game does not decline.
        app.world_mut().resource_mut::<bevy::time::Time<bevy::time::Virtual>>().pause();
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(tag_level(tag_of(&app, oak)).0, level, "paused");
        app.world_mut().resource_mut::<bevy::time::Time<bevy::time::Virtual>>().unpause();
        for _ in 0..90 {
            app.update();
        }
        assert_eq!(tag_of(&app, oak), TreeCondition::Dying.tag_bits());
        // Settled trees are not visited by the glide system at all.
        let mut visited = app.world_mut().query_filtered::<Entity, With<ConditionGliding>>();
        assert_eq!(visited.iter(app.world()).count(), 0);
    }
}
