//! Where the per-node state of every tree that grows on the GPU lives.
//!
//! The state of all such trees shares a few large storage buffers (chunks). A tree owns a
//! [`GrowthSlice`], a run of `vec4`s inside one chunk, and its meshes carry the slice's start
//! (`MeshTag`), which the vertex shader adds to every index it reads. Because the buffer is
//! not the tree's own, trees with equal looks bind the same buffer and so can share their
//! materials (see [`GrowthMaterials`](super::render::GrowthMaterials)).
//!
//! A chunk's [`ShaderBuffer`] has no CPU copy and is never modified as an asset: Bevy would
//! re-upload the whole buffer, and every material that binds it would be re-prepared. A tree's
//! state goes to the GPU as a write of just its slice: [`GrowthSlice::write`] queues it in the
//! store, [`extract_growth_state`] hands the queue to the render world, and
//! [`upload_growth_state`] copies it to the chunk's buffer. Only the slices of trees whose state
//! changed are written, and each is written once.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use bevy::asset::{AssetId, Assets, Handle, RenderAssetUsages};
use bevy::ecs::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::renderer::RenderQueue;
use bevy::render::Extract;
use bevy::render::storage::{GpuShaderBuffer, ShaderBuffer};

use super::state::GrowthState;

/// `vec4`s in a chunk (2 MiB): room for the state of tens of trees. A tree larger than this
/// gets a chunk of its own.
pub(crate) const CHUNK_VEC4S: u32 = 1 << 17;

/// Bytes in a `vec4`.
const VEC4_BYTES: u64 = 16;

/// The unused runs of one chunk, first fit, kept sorted by start with neighbours merged.
#[derive(Debug)]
pub(crate) struct FreeList {
    #[cfg(test)]
    capacity: u32,
    /// `(start, length)`, ascending and never touching.
    runs: Vec<(u32, u32)>,
}

impl FreeList {
    pub(crate) fn new(capacity: u32) -> Self {
        Self {
            #[cfg(test)]
            capacity,
            runs: if capacity > 0 { vec![(0, capacity)] } else { Vec::new() },
        }
    }

    /// Take `len` consecutive slots from the first run that holds them; returns the first.
    pub(crate) fn allocate(&mut self, len: u32) -> Option<u32> {
        let at = self.runs.iter().position(|&(_, run)| run >= len)?;
        let (start, run) = self.runs[at];
        if run == len {
            self.runs.remove(at);
        } else {
            self.runs[at] = (start + len, run - len);
        }
        Some(start)
    }

    /// Give back `len` slots from `start`, which must have been allocated.
    pub(crate) fn release(&mut self, start: u32, len: u32) {
        let at = self.runs.partition_point(|&(s, _)| s < start);
        debug_assert!(self.runs.get(at).is_none_or(|&(s, _)| s >= start + len), "freed slots overlap a free run");
        debug_assert!(at == 0 || self.runs[at - 1].0 + self.runs[at - 1].1 <= start, "freed slots overlap a free run");
        self.runs.insert(at, (start, len));
        // Merge with the run after, then the run before.
        if let Some(&(next, next_len)) = self.runs.get(at + 1)
            && start + len == next
        {
            self.runs[at].1 += next_len;
            self.runs.remove(at + 1);
        }
        if at > 0 {
            let (before, before_len) = self.runs[at - 1];
            if before + before_len == start {
                self.runs[at - 1].1 += self.runs[at].1;
                self.runs.remove(at);
            }
        }
    }

    /// Whether nothing is allocated.
    #[cfg(test)]
    pub(crate) fn is_unused(&self) -> bool {
        self.runs == [(0, self.capacity)]
    }
}

struct Chunk {
    buffer: Handle<ShaderBuffer>,
    slots: FreeList,
}

#[derive(Default)]
struct Inner {
    chunks: Vec<Chunk>,
    /// State waiting for extraction, by (buffer, first `vec4`). Writing a slice again before
    /// it was extracted replaces the earlier write.
    pending: HashMap<(AssetId<ShaderBuffer>, u32), Vec<[f32; 4]>>,
}

/// The chunks every tree that grows on the GPU keeps its state in.
///
/// A chunk lives as long as the app: an emptied chunk is reused by the next tree, so the
/// buffers are at their high-water mark.
#[derive(Resource, Clone, Default)]
pub(crate) struct GrowthStore(Arc<Mutex<Inner>>);

impl GrowthStore {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        // Nothing here panics while holding the lock, so a poisoned lock is still consistent.
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Reserve `len` `vec4`s, in a chunk that has room or a new one.
    pub(crate) fn allocate(&self, len: usize, buffers: &mut Assets<ShaderBuffer>) -> GrowthSlice {
        let len = u32::try_from(len).expect("a tree's growth state fits in a buffer");
        let mut inner = self.lock();
        let found = inner
            .chunks
            .iter_mut()
            .enumerate()
            .find_map(|(index, chunk)| Some((index, chunk.slots.allocate(len)?)));
        let (chunk, base) = found.unwrap_or_else(|| {
            let capacity = CHUNK_VEC4S.max(len);
            let buffer = buffers.add(ShaderBuffer::with_size(
                u64::from(capacity) * VEC4_BYTES,
                RenderAssetUsages::RENDER_WORLD,
            ));
            let mut slots = FreeList::new(capacity);
            let base = slots.allocate(len).expect("a new chunk holds the tree it was made for");
            inner.chunks.push(Chunk { buffer, slots });
            (inner.chunks.len() - 1, base)
        });
        let buffer = inner.chunks[chunk].buffer.clone();
        drop(inner);
        GrowthSlice { store: self.clone(), chunk, base, len, buffer }
    }

    /// Number of chunks made so far.
    #[cfg(test)]
    pub(crate) fn chunk_count(&self) -> usize {
        self.lock().chunks.len()
    }

    /// Drop everything queued for the GPU, as the render world does, and say how many slices
    /// that was.
    #[cfg(test)]
    pub(crate) fn take_queued(&self) -> usize {
        let mut inner = self.lock();
        let queued = inner.pending.len();
        inner.pending.clear();
        queued
    }

    /// Whether every chunk is empty.
    #[cfg(test)]
    pub(crate) fn is_unused(&self) -> bool {
        let inner = self.lock();
        inner.chunks.iter().all(|chunk| chunk.slots.is_unused()) && inner.pending.is_empty()
    }
}

/// A tree's run of `vec4`s in a chunk. Dropping it (the tree's render state went away) gives
/// the run back for the next tree.
pub(crate) struct GrowthSlice {
    store: GrowthStore,
    chunk: usize,
    base: u32,
    len: u32,
    buffer: Handle<ShaderBuffer>,
}

impl GrowthSlice {
    /// Index of the slice's first `vec4` in its chunk: what the meshes' `MeshTag` carries.
    pub(crate) fn base(&self) -> u32 {
        self.base
    }

    /// Number of `vec4`s.
    pub(crate) fn len(&self) -> usize {
        self.len as usize
    }

    /// The chunk's buffer, which the materials of the tree bind.
    pub(crate) fn buffer(&self) -> &Handle<ShaderBuffer> {
        &self.buffer
    }

    /// Queue `state` for the GPU, in place of anything not yet taken.
    pub(crate) fn write(&self, state: &GrowthState) {
        let mut inner = self.store.lock();
        let data = inner.pending.entry((self.buffer.id(), self.base)).or_default();
        data.clear();
        data.resize(self.len(), [0.0; 4]);
        state.encode(data);
    }

    /// The state queued and not yet taken by the render world (all of this in a headless app).
    #[cfg(test)]
    pub(crate) fn queued(&self) -> Option<Vec<[f32; 4]>> {
        self.store.lock().pending.get(&(self.buffer.id(), self.base)).cloned()
    }
}

impl Drop for GrowthSlice {
    fn drop(&mut self) {
        let mut inner = self.store.lock();
        // The state of a tree that is gone must not land on whoever owns the slots next.
        inner.pending.remove(&(self.buffer.id(), self.base));
        inner.chunks[self.chunk].slots.release(self.base, self.len);
    }
}

/// Render world: state taken from the main world at extraction, waiting for its chunk's GPU
/// buffer. Extraction is where the two worlds meet, so a frame draws the state of the frame
/// the main world just finished, as it does for every other component and asset.
#[derive(Resource, Default)]
pub(crate) struct GrowthUploads(Vec<(AssetId<ShaderBuffer>, u32, Vec<[f32; 4]>)>);

/// Extraction: move what the main world queued into the render world. Writes of live slices
/// never overlap (a freed slice's queued write is dropped with it), so their order does not matter.
pub(crate) fn extract_growth_state(store: Extract<Res<GrowthStore>>, mut uploads: ResMut<GrowthUploads>) {
    let mut inner = store.lock();
    uploads.0.extend(inner.pending.drain().map(|((id, base), data)| (id, base, data)));
}

/// Render world: copy the extracted state into the chunks' GPU buffers, each slice by itself.
/// A slice whose chunk has no GPU buffer yet waits for it.
pub(crate) fn upload_growth_state(
    mut uploads: ResMut<GrowthUploads>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    queue: Res<RenderQueue>,
    mut bytes: Local<Vec<u8>>,
) {
    uploads.0.retain(|(id, base, data)| {
        let Some(gpu) = buffers.get(*id) else { return true };
        bytes.clear();
        bytes.extend(data.iter().flatten().flat_map(|value| value.to_ne_bytes()));
        queue.write_buffer(&gpu.buffer, u64::from(*base) * VEC4_BYTES, &bytes);
        false
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_free_list_hands_out_disjoint_runs_and_reuses_freed_ones() {
        let mut list = FreeList::new(100);
        let a = list.allocate(30).unwrap();
        let b = list.allocate(50).unwrap();
        let c = list.allocate(20).unwrap();
        assert_eq!((a, b, c), (0, 30, 80));
        assert_eq!(list.allocate(1), None, "full");

        list.release(b, 50);
        assert_eq!(list.allocate(51), None, "the hole is 50 long");
        let d = list.allocate(10).unwrap();
        let e = list.allocate(40).unwrap();
        assert_eq!((d, e), (30, 40), "first fit into the hole");
        assert_eq!(list.allocate(1), None);

        for (start, len) in [(a, 30), (d, 10), (e, 40), (c, 20)] {
            list.release(start, len);
        }
        assert!(list.is_unused(), "neighbours merge back into one run: {list:?}");
        assert_eq!(list.allocate(100), Some(0));
    }

    #[test]
    fn freed_runs_merge_whatever_order_they_come_back_in() {
        for order in [[0, 1, 2], [2, 1, 0], [1, 0, 2], [1, 2, 0], [0, 2, 1], [2, 0, 1]] {
            let mut list = FreeList::new(30);
            let runs: Vec<_> = (0..3).map(|_| list.allocate(10).unwrap()).collect();
            for i in order {
                list.release(runs[i], 10);
            }
            assert!(list.is_unused(), "order {order:?}: {list:?}");
        }
    }

    fn state_of(nodes: usize) -> GrowthState {
        let mut state = GrowthState::default();
        state.nodes = vec![Default::default(); nodes];
        state
    }

    #[test]
    fn slices_do_not_overlap_and_a_full_chunk_starts_another() {
        let store = GrowthStore::default();
        let mut buffers = Assets::<ShaderBuffer>::default();
        let big = CHUNK_VEC4S as usize / 3;
        let slices: Vec<_> = (0..7).map(|_| store.allocate(big, &mut buffers)).collect();
        assert_eq!(store.chunk_count(), 3, "three fit in a chunk");
        for (i, a) in slices.iter().enumerate() {
            for b in &slices[i + 1..] {
                let apart = a.buffer().id() != b.buffer().id()
                    || a.base() + a.len() as u32 <= b.base()
                    || b.base() + b.len() as u32 <= a.base();
                assert!(apart, "slices overlap");
            }
        }
        drop(slices);
        assert!(store.is_unused());
        let again = store.allocate(big, &mut buffers);
        assert_eq!((store.chunk_count(), again.base()), (3, 0), "an emptied chunk is reused");
    }

    #[test]
    fn a_tree_larger_than_a_chunk_gets_a_chunk_of_its_own_size() {
        let store = GrowthStore::default();
        let mut buffers = Assets::<ShaderBuffer>::default();
        let len = CHUNK_VEC4S as usize + 5;
        let slice = store.allocate(len, &mut buffers);
        assert_eq!(buffers.get(slice.buffer()).unwrap().buffer_size(), len as u64 * VEC4_BYTES);
        let small = store.allocate(10, &mut buffers);
        assert_ne!(small.buffer().id(), slice.buffer().id(), "no room left beside it");
    }

    #[test]
    fn a_dropped_slice_takes_its_queued_state_with_it() {
        let store = GrowthStore::default();
        let mut buffers = Assets::<ShaderBuffer>::default();
        let state = state_of(3);
        let first = store.allocate(state.vec4_len(), &mut buffers);
        first.write(&state);
        assert_eq!(first.queued().unwrap().len(), state.vec4_len());
        drop(first);
        assert!(store.is_unused(), "nothing queued for a freed slice");

        // The next owner of the same slots sees only its own writes.
        let second = store.allocate(state.vec4_len(), &mut buffers);
        assert_eq!(second.base(), 0);
        assert_eq!(second.queued(), None);
    }

    #[test]
    fn writing_a_slice_again_replaces_the_queued_state() {
        let store = GrowthStore::default();
        let mut buffers = Assets::<ShaderBuffer>::default();
        let mut state = state_of(2);
        let slice = store.allocate(state.vec4_len(), &mut buffers);
        slice.write(&state);
        state.cone_lift = 7.0;
        slice.write(&state);
        assert_eq!(slice.queued().unwrap()[0][3], 7.0);
        assert_eq!(store.lock().pending.len(), 1);
    }
}
