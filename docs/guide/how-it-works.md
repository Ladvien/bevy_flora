# How it works

The pipeline from parameters to a Bevy `Mesh`, and the invariants the meshers rely on.

`TreeParams` → skeleton (a node tree of trunk, leader, primaries, splits and sub-branches) → birth
times → the skeleton at a given age → wood mesh and leaf points → `MeshData` → Bevy `Mesh`.

Four things hold across every stage, and a change that breaks one is a bug rather than a trade-off:

- **No re-rolling.** A branch's birth age is derived from where it sits on its parent, and a leaf's
  moment from the branch's growth, so aging a tree adds and grows geometry but never moves or replaces
  what is already there. The same value makes the same tree at every age, on every run.
- **Winding.** Every face is wound outward, so backface culling and the shadow pass see a closed solid.
  `mesh/rings.rs` is where that is decided, and `crown_hull.rs` and `mesh/cut.rs` follow it.
- **Closed wood.** Branches meet the trunk through flared collars and root segments, forks taper into
  their halves, and cut faces close a mesh so no hole is ever left in it (felling, bucking, a notch).
- **One tree is one skeleton.** The meshes, the levels of detail, the impostor, `TreeShape` and
  `TreeLod` are all read from the same aged skeleton, which is why a pooled tree and a `ProceduralTree`
  put every leaf in the same place and why `GpuGrowth` can deform the mature mesh to match the CPU one.

[ARCHITECTURE.md](../../ARCHITECTURE.md) has the module map, the cost of each stage and the invariants in
full. `src/`'s module docs are the next level down, and each names the research note in `docs/research/`
that a design came from.
