//! Building trees off the main thread: [`BuildInBackground`].

use bevy::ecs::prelude::*;
use bevy::reflect::Reflect;
use bevy::tasks::futures::check_ready;
use bevy::tasks::{AsyncComputeTaskPool, Task, TaskPool};

/// Build this tree's meshes in the background, on Bevy's async compute pool, instead of in the
/// frame it is spawned or its [`ProceduralTree`](crate::ProceduralTree) or
/// [`TreeAge`](crate::TreeAge) changes: a game that plants trees as the player walks (a streamed
/// chunk, a sapling the player plants) does not hitch while they are built.
///
/// A new tree draws nothing (it has no [`TreeRender`](crate::TreeRender) yet) until its first
/// build is done, usually a frame or a few later, and [`TreeGenerated`](crate::TreeGenerated) is
/// sent then; after a change it keeps its old meshes until the new ones are in. A change while a
/// build is under way starts a new one in its place (a change of age waits for it, and the tree
/// is then brought to its age). A [`Growing`](crate::Growing) tree is rebuilt in the background
/// at every age too, so it shows each age a frame or two late; grow many trees with
/// [`GpuGrowth`](crate::GpuGrowth), which this also works with (its one build, of the mature
/// tree, goes to the background; ageing it stays in the frame, as it costs a few kilobytes of
/// writes). Pools are unaffected: [`build_pool`](crate::build_pool) is a call of its own.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct BuildInBackground;

/// The task building a [`BuildInBackground`] tree; `R` is what the CPU or the GPU path builds.
/// Replacing or removing it (or despawning the tree) drops the task, so its result is never
/// applied.
#[derive(Component)]
pub(crate) struct PendingBuild<R: Send + 'static>(Task<R>);

impl<R: Send + 'static> PendingBuild<R> {
    /// Start `build` on the async compute pool.
    pub(crate) fn spawn(build: impl FnOnce() -> R + Send + 'static) -> Self {
        Self(AsyncComputeTaskPool::get_or_init(TaskPool::default).spawn(async move { build() }))
    }

    /// The result, once the build is done.
    pub(crate) fn poll(&mut self) -> Option<R> {
        check_ready(&mut self.0)
    }
}
