//! How well a tree is: [`TreeCondition`], a fact about one tree that a game sets and reads back.
//!
//! A game that wants a sick tree, a withering stand or a copse that dies in one season inserts
//! [`TreeCondition`] on the tree's entity (a [`ProceduralTree`](crate::ProceduralTree) or a
//! [`PooledTree`](crate::PooledTree)) and finds it again through [`FloraIndex`](crate::FloraIndex):
//! [`IndexedTree::condition`](crate::IndexedTree::condition) on every tree a query returns, or
//! [`FloraIndex::condition_of`](crate::FloraIndex::condition_of) for one entity.
//!
//! This is state, not looks: the crate draws a sick tree as it draws a healthy one. A game that wants
//! it to look different recolours it ([`RecolorTree`](crate::RecolorTree), or a
//! [`PooledLook`](crate::PooledLook) of a pool's own looks).
//!
//! Pure CPU, like `shape.rs`: it derives `Component` and nothing here touches rendering.

use bevy::ecs::prelude::*;
use bevy::reflect::Reflect;

/// The condition of a tree, in the three classes of a forest-health survey's crown assessment
/// (`docs/research/tree-condition.md`): a healthy crown, one in decline, one close to death. Ordered
/// from well to ill, so `condition >= TreeCondition::Sick` means "not healthy".
///
/// **Insert it on the tree's entity**, `commands.entity(tree).insert(TreeCondition::Dying)`, and
/// [`FloraIndex`](crate::FloraIndex) shows it from the next [`PostUpdate`](bevy::app::PostUpdate).
/// Insert [`TreeCondition::Healthy`] or remove the component to clear it: a tree without one is
/// healthy, so a forest of a hundred thousand pays nothing for the trees that are well. To react to a
/// change, query `Changed<TreeCondition>`.
///
/// The condition belongs to the **tree's entity**, not to a [`TreePool`](crate::TreePool) its meshes
/// come from: a [`PooledTree`](crate::PooledTree) that changes variant or pool, is taken off the entity
/// and put back, or whose pool reloads, keeps it. A tree set before it is indexed (a pooled tree whose
/// pool has not loaded yet) shows it as soon as it is. Despawning the tree takes it with it.
///
/// A felled tree leaves the index ([`Felled`](crate::Felled)): [`FloraIndex::condition_of`] answers
/// `None` for it, and the component stays on its entity as it was when the tree was cut, for a
/// `Query<&TreeCondition>` to read. Setting one on a felled tree does nothing else. There is no
/// `Dead` condition: a dead tree is the game's to fell ([`FellTree`](crate::FellTree)), replace, or
/// leave standing.
///
/// [`FloraIndex::condition_of`]: crate::FloraIndex::condition_of
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
#[reflect(Component, Default, PartialEq)]
pub enum TreeCondition {
    /// A full crown: the state of a tree with no `TreeCondition`.
    #[default]
    Healthy,
    /// A crown in decline: thinning, off its colour. The tree is still standing and may recover.
    Sick,
    /// A crown close to death: mostly bare or browned.
    Dying,
}
