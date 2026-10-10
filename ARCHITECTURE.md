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

## Mesh conventions

Front faces are counter-clockwise; the helpers in `mesh/rings.rs` own the winding
convention. Never fix winding in an individual mesher.

A branch's shape is `Node::axis` (bent by `Node::bend`); the tube mesher, child
placement and leaf placement (`Node::point_at`) all read it, so wood, sub-branches
and leaves agree. Sub-branches spreading toward the trunk and tip-cluster leaves
landing inside it are mirrored away (`skeleton::trunk_depth`). Each primary's
tube and collar get a `root_segment` reaching into the trunk (`primary_root`),
so junctions have no open ends. Smoothing (Taubin λ|μ, so it does not shrink the
wood) welds coincident vertices (UV seams) and pins open boundaries, the ground
ring, branch-end caps and the parent surface under each child's seat, so it
never tears, lifts, retracts or detaches the wood. Wood finer than ~100 weld
tolerances (hairline twigs) stays as generated, and normals are recomputed only
around moved vertices, pooled across seam twins, so unmoved pieces keep their
generated shading. Root segments are added after it, so junctions keep their shape.

## Bevy integration (plugin.rs)

- `ProceduralTree(TreeParams)` + `TreeAge(f32)` (default `INFINITY` = mature) + optional `Growing { rate }`.
- `FloraPlugin` rebuilds meshes on `Changed<ProceduralTree>` / `Changed<TreeAge>`, keeping a wood child and a leaf child. An empty trunk (age 0) leaves the wood child hidden with no mesh; the mesh is created once the trunk has geometry.
- `TreeGenerated` message reports current height/radius after each rebuild.
