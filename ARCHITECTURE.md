# bevy_flora Architecture

Pure-Rust Bevy 0.20 crate that generates parametric, stylized trees and grows them over time.

## Pipeline

```
TreeParams
  → generate_skeleton        (skeleton.rs: Node tree, parent links, deterministic from seed;
                              gravity sets each branch's `bend`, children sit on the bent parent axis)
  → assign_birth_times       (growth.rs: birth + duration per node, from topology)
  → Skeleton::aged(age)      (drops unborn nodes; scales length/radius by progress)
  → apply_pipe_radius        (optional, mesh/branch.rs)
  → skeleton_mesh            (mesh/: trunk, collars, tubes, optional smoothing)
  → collect_leaf_points → foliage_mesh   (foliage.rs)
  → MeshData → bevy Mesh     (mesh/mod.rs)
```

`pipeline::build_tree(params, age)` runs all of it and returns `TreeMeshes`
(`trunk`, `foliage`, aged `skeleton`, `mature_at`).

## Growth model

Progress is linear. A primary is born when the trunk reaches its attachment
height; a sub-branch is born when its parent reaches `attach_t`. Radii thicken
over `duration * thickening_lag`. Leaves appear after `leaf_threshold` progress
and scale to full size. Each node draws leaf randomness from its own RNG stream
(`SeededRng::from_parts(seed, node.id)`), so aging never re-rolls existing leaves.
`Skeleton::aged` keeps each node's full-grown `child_count`, so tip placements
(`TerminalBranches`, `TipClusters`) pick the same branches at every age and a
leaf never disappears as the tree grows.

## Leaf look (no shader)

Everything that makes leaves look good is baked into the mesh, so the stock
`StandardMaterial` is enough and nothing runs per fragment beyond normal PBR.

- **Leaf-shaped cards.** `CrossedPlanes`, `SingleQuad` and `StarBurst` emit an
  eight-point pointed oval (`LEAF_OUTLINE`, about half the area of the old square
  card) as a triangle fan, twice with opposite winding. Cutting the silhouette into
  the geometry keeps hidden-surface removal intact, which an alpha-masked texture
  (`discard`) would not on tile-based GPUs. `CrossedPlanes` and `SingleQuad` seat the
  card's base on the leaf point and reach one `size` out along the basis `up`, so a
  leaf grows out of its twig instead of being threaded on it.
- **Crown-bent normals.** `foliage_mesh` lerps every normal toward the direction out
  of the crown centre (mean leaf position) by `FoliageParams::normal_bend`, so the
  crown shades as one soft volume instead of per-card light and dark. Bent normals
  intentionally disagree with face winding; winding is checked at `normal_bend = 0`.
- **Per-leaf tint.** `LeafPoint::tint` is drawn from its own RNG stream
  (`seed ^ LEAF_TINT_SALT`), so it never moves another draw or re-rolls with age,
  and `foliage_mesh` writes it as a vertex colour (`MeshData::colors`,
  `Mesh::ATTRIBUTE_COLOR`) scaled by `FoliageParams::color_variation`.

## Mesh conventions

Front faces are counter-clockwise; the helpers in `mesh/rings.rs` own the winding
convention. Never fix winding in an individual mesher.

A branch's shape is `Node::axis` (bent by `Node::bend`) sampled at
`Node::axis_points()` points: 6 for a lightly bent branch, up to 16 when gravity
turns its direction by more than ~4.6° between points, so a long drooping limb is
smooth. The tube mesher, child placement and leaf placement (`Node::point_at`)
all use the same count, so wood, sub-branches and leaves agree. Sub-branches
spread around the parent's bent tangent where they attach (a drooping limb hands
its twigs the direction it has there, so they hang). Generated radii are floored
by `BranchParams::min_twig_radius` at the base and half of it at the tip, and
never exceed the wood they grow from (`skeleton::floored_radii`), so twigs taper
but never shrink to hairlines. Sub-branches spreading toward the trunk and
tip-cluster leaves landing inside it are mirrored away (`skeleton::trunk_depth`);
after seating and the gravity bend, a sub-branch whose wood still enters the trunk
gets its bend relaxed, or is shortened (`Node::keep_out_of_trunk`).
Each primary's tube and collar get a `root_segment` reaching into the trunk
(`primary_root`); a branch hugging the trunk (less than half of its direction
pointing away from it) gets a closed, rounded `root_dome` instead, since a long
extension would show as a ridge down the trunk. The dome has four rings and a cap
with twice the tube's side count, joined to the tube's own base ring, so it is
round even on a hexagonal branch. Either way
junctions have no open ends. Smoothing (Taubin λ|μ, so it does not shrink the
wood) welds coincident vertices (UV seams) and pins open boundaries, the ground
ring, branch-end caps and the parent surface under each child's seat, so it
never tears, lifts, retracts or detaches the wood. Wood finer than ~100 weld
tolerances (hairline twigs) stays as generated, and normals are recomputed only
around moved vertices, pooled across seam twins, so unmoved pieces keep their
generated shading. Root segments are added after it, so junctions keep their shape.

## Bevy integration (plugin.rs)

- `ProceduralTree(TreeParams)` + `TreeAge(f32)` (default `INFINITY` = mature) + optional `Growing { rate }`.
- `FloraPlugin` rebuilds meshes on `Changed<ProceduralTree>` / `Changed<TreeAge>`, keeping a wood child and a leaf child. An empty trunk (age 0) leaves the wood child hidden with no mesh; the mesh is created once the trunk has geometry.
- The full-grown skeleton is cached per tree (`TreeRender.full`) and regenerated only when `ProceduralTree` changes; a `TreeAge`-only frame (growth) runs `pipeline::build_aged` on the cache. Materials are written only when a colour actually changed.
- `TreeGenerated` message reports current height/radius after each rebuild.
- `TreeRender.paint` says how a tree is drawn: `Paint::Standard` (CPU-rebuilt meshes, `StandardMaterial`) or `Paint::Growth` (GPU growth, below). `sync_growth_mode` drops the render state of a tree whose `GpuGrowth` appeared, went, or whose device support changed, and the matching rebuild system recreates it.

## GPU growth (`gpu/`)

A tree with `GpuGrowth` is built once at maturity and grown in the vertex shader. Why that works:

- `Skeleton::aged` changes only `start`, `length`, the two radii and `progress`; `direction` and `bend` never change, so every tube ring has the same frame (`RingBasis::from_axis(direction)`) and the same axis shape at every age (`gpu::tests::ageing_never_turns_or_bends_wood`). A tube vertex is therefore `start + length * c + radius * o` with `c` (axis offset in node lengths) and `o` (unit radial offset) measured once on the mature mesh. Leaves are `start + length * c + leaf_scale * d`.
- `mesh.rs` builds the mature meshes with the CPU meshers (`skeleton_mesh_with` records which node and piece every run of vertices belongs to; `foliage_mesh_with` records each leaf's first vertex), tags every vertex (`frame.rs`: `Class`, `Tag`) and measures its offset on the final, smoothed positions, so smoothing rides along as a deformation. The wood always carries the trunk's top cap and (pointed trunks) its tip cone; the shader hides the one the age does not have.
- `state.rs` writes the only per-frame upload: a header, 5 `vec4` per node (`NodeState`: start, length, radii, leaf scale and count, collar ends and radii, root point, piece flags) and 2 per trunk ring (centre, radius, root bulge, twist). It calls the same helpers the meshers call (`aged_skeleton`, `collar_geometry`, `primary_root`, `root_segment_emitted`, `node_leaf_scale`, `trunk_rings`), so the shader and the CPU path agree on which pieces exist at an age. The buffer is a `ShaderBuffer`, rewritten in place (`queue.write_buffer`); the materials are not touched.
- `growth_common.wesl` mirrors `frame.rs::position` (the Rust reference the tests check against the CPU meshers at many ages); `growth.wesl` and `growth_prepass.wesl` are Bevy's vertex stages with the position replaced, the prepass one also serving shadow maps. `GrowthExtension` binds the buffer at slot 100 of an `ExtendedMaterial<StandardMaterial, _>` and appends the three vertex attributes (locations 8-10) to the layout the base material chose.
- The trunk's ring twist is `trunk.twist * t` with `t` scaled by progress, so unlike the branches it changes with age; trunk offsets are stored with the twist taken out and the shader turns them by the ring's current twist.
- `GpuGrowthSupport` is set from the device's limits (`storage_buffers_are_unsupported`) once the renderer exists; without it, trees with `GpuGrowth` use the CPU path.
