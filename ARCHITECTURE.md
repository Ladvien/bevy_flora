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
- `TreeGenerated` message reports current height/radius after each rebuild.
