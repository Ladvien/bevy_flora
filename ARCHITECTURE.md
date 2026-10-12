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

## Leaf look (baked into the mesh)

The mesh carries what the leaf shader reads (see Leaf shading and wind): leaf-shaped
cards, crown-bent normals, a per-leaf vertex colour (tint x `color_variation`, hue x
`hue_variation`), `uv1` (x = upper side, y = crown visibility from
`FoliageParams::occlusion`) and the sway attribute. The meshes also draw correctly
with a plain `StandardMaterial`, just without the leaf shader.

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
- The full-grown skeleton is cached per tree (`TreeRender.full`) and regenerated only when `ProceduralTree` changes; a `TreeAge`-only frame (growth) runs `pipeline::build_aged` on the cache. `rebuild_trees` works in three phases: it collects the trees whose parameters or age changed, builds their meshes with `build_in_batches` on the compute task pool (a few per thread at a time, bit-identical to the serial result, inline for fewer than two), then applies the results one by one. Materials are never written in place: a tree resolves its two `FloraMaterial`s from `SharedFlora` by look (colour, leaf flag, the species part of the uniform), so trees with equal looks (same colours, trunk height, wind response and leaf shading; the seed is irrelevant) share them, and a tree whose look changes is given other shared materials.
- `TreeGenerated` message reports current height/radius after each rebuild.
- `TreeRender.paint` says how a tree is drawn: `Paint::Flora` (CPU-rebuilt meshes, shared `FloraMaterial`s), `Paint::Standard` (CPU-rebuilt, plain `StandardMaterial`s, where `FloraMaterialSupport` is absent) or `Paint::Growth` (GPU growth, below). `sync_growth_mode` drops the render state of a tree whose `GpuGrowth` appeared, went, or whose device support changed, and the matching rebuild system recreates it.

## GPU growth (`gpu/`)

A tree with `GpuGrowth` is built once at maturity and grown in the vertex shader. Why that works:

- `Skeleton::aged` changes only `start`, `length`, the two radii and `progress`; `direction` and `bend` never change, so every tube ring has the same frame (`RingBasis::from_axis(direction)`) and the same axis shape at every age (`gpu::tests::ageing_never_turns_or_bends_wood`). A tube vertex is therefore `start + length * c + radius * o` with `c` (axis offset in node lengths) and `o` (unit radial offset) measured once on the mature mesh. Leaves are `start + length * c + leaf_scale * d`.
- `mesh.rs` builds the mature meshes with the CPU meshers (`skeleton_mesh_with` records which node and piece every run of vertices belongs to; `foliage_mesh_with` records each leaf's first vertex), tags every vertex (`frame.rs`: `Class`, `Tag`) and measures its offset on the final, smoothed positions, so smoothing rides along as a deformation. The wood always carries the trunk's top cap and (pointed trunks) its tip cone; the shader hides the one the age does not have.
- `state.rs` writes the only per-frame upload: a header, 5 `vec4` per node (`NodeState`: start, length, radii, leaf scale and count, collar ends and radii, root point, piece flags) and 2 per trunk ring (centre, radius, root bulge, twist). It calls the same helpers the meshers call (`aged_skeleton`, `collar_geometry`, `primary_root`, `root_segment_emitted`, `node_leaf_scale`, `trunk_rings`), so the shader and the CPU path agree on which pieces exist at an age. The buffer is a `ShaderBuffer`, rewritten in place (`queue.write_buffer`); the materials are not touched.
- `growth_common.wesl` mirrors `frame.rs::position` (the Rust reference the tests check against the CPU meshers at many ages); `growth.wesl` and `growth_prepass.wesl` are Bevy's vertex stages with the position replaced, the prepass one also serving shadow maps. `GrowthExtension` (an `ExtendedMaterial<StandardMaterial, _>`) binds the node buffer at slot 100, the `FloraUniform` at 101 and `FloraWindBuffer` at 102, and appends the three growth attributes (locations 8-10), plus the leaf sway attribute (11) on leaf materials, to the layout the base material chose.
- The trunk's ring twist is `trunk.twist * t` with `t` scaled by progress, so unlike the branches it changes with age; trunk offsets are stored with the twist taken out and the shader turns them by the ring's current twist.
- `GpuGrowthSupport` is set from the device's limits (`storage_buffers_are_unsupported`) once the renderer exists; without it, trees with `GpuGrowth` use the CPU path.


## Leaf shading and wind (`shading/`)

Every tree the plugin draws uses `FloraMaterial = ExtendedMaterial<StandardMaterial, FloraExtension>` (also `GrowthMaterial` and `ImpostorMaterial`, which carry the same `FloraUniform` at binding 101 and call the same shader library, `common.wesl`). Without the renderer's plugins, or with `FloraPlugin` added before them, `build` registers only the asset type, `FloraMaterialSupport` is never inserted, trees get plain `StandardMaterial`s and `finish` warns once when a renderer exists (the plugin was added before it) and says nothing when there is none.

- **The wind is not in the materials.** `FloraUniform` (binding 101) holds what belongs to a tree and its species: `tree` (1 / trunk height), `response` (`TreeParams::wind`), `leaf` and `tint` (`LeafShading`). The weather is the same for every tree, so it lives once, in `FloraWindBuffer` (a `ShaderBuffer` of 5 `vec4`: `wind`, `motion`, `rate`, `front`, `front_shape`, packed from `FloraWind` by `wind_vectors`), bound at binding 102 by `FloraExtension`, `GrowthExtension` and `ImpostorMaterial`. `sync_wind` writes it in place (`resource_changed::<FloraWind>`; a same-size `ShaderBuffer` write is a `queue.write_buffer`, which prepares no material), so changing the wind modifies no material asset. That matters because Bevy re-specializes every entity of a material whenever the material asset is modified: with the wind in the materials a pooled forest of 20 000 trees took 212 ms a frame while the wind changed every frame, and now 3.3 ms. Devices without storage buffers in the vertex stage (WebGL2) cannot bind it: `detect_flora_support` then leaves `FloraMaterialSupport` out, trees get plain materials and pools no impostors. `wood()` zeroes only `leaf` and `tint`: wood bends with the species' `sway`. The struct in `common.wesl` must list the fields in the Rust order.
- **Wind** (`wind_bend`, `flutter_offset`) follows Crysis' vegetation animation (GPU Gems 3, chapter 16): a main bend that grows with the square of the height over the trunk and keeps every vertex's distance from the trunk, and per-leaf flutter with smooth triangle waves of unrelated frequencies. A `GustFront` is a Gaussian band, `strength * exp(-(along / width)^2)` with `along = dot(origin.xz - front.origin, direction) - speed * (time - start)`, added to the wind's strength for flutter and to its push along the front's own direction for the bend; `start` is `Time::elapsed_secs_wrapped`, which is the `globals.time` the shaders read. It runs in the vertex stage of `vertex.wesl` and of `prepass.wesl` (the shadow maps), in the GPU growth shaders and in `impostor.wesl`, so shadows follow the motion. There are no motion vectors.
- **Leaf look** (`fragment.wesl`, `leaf_surface`, `transmitted_light`): upper and lower side from `uv1.x`, veins from the card's UVs (never on `NeedleCluster` and `ClusterSphere`, whose UVs are not a leaf), crown visibility from `uv1.y` as diffuse and specular occlusion, and Unreal's two-sided foliage term, the back side's wrapped N·L times a forward-scattering GGX lobe times the leaf colour and tint, for every directional light. Leaf materials add the `FLORA_LEAF` shader def and the vertex attribute `ATTRIBUTE_LEAF_SWAY` (location 11) in `specialize_flora`. The specular reflectance of a leaf fades as the camera faces a directional light (`leaf_gloss_toward_light`: 1 until `-V·L` reaches 0.4, 0 from 0.8, the strongest light wins), so a backlit card does not mirror the sun into the camera; the impostor does the same for its leaf texels (baked translucency). Views with the sun beside or behind the camera are unchanged.
- WGSL compile errors in these shaders only show as `ERROR bevy_render pipeline_cache` lines when a real render runs (`forest --screenshot`, `gallery --gpu`), never in a headless test. `class` is a reserved WGSL word.

## Bark (`shading/textures.rs`, `mesh/rings.rs`, `src/textures/`)

Wood wears a bundled CC0 scan (`BarkScan`, `TreeParams::bark`; the files and their licences are in `src/textures/CREDITS.md`, produced by `tools/prepare_textures.py`, which is not packaged). The `textures` feature (on by default) compiles the JPEGs in; without it every scan resolves to `None` and wood is plain colour, exactly as before.

- **Wood UVs are metric and anchored to the mature tree.** `emit_ring` takes a `RingUv { v, u_tiles }`; `v` is the ring's distance along the fully grown wood in tile lengths (`mesh::branch::BarkLayout::ring`), `u` runs a whole number of tiles around (`u_tiles_around`: `round(2π · mature base radius / tile width)`, at least 1), so the seam is invisible. Mature sizes are the aged ones over their progress (`mature_size`), and a trunk ring's `v` is its index fraction times the trunk height, so every vertex keeps its texture coordinate at every age: bark never slides as a tree grows, and the aged CPU mesh agrees with the mature mesh `GpuGrowth` shrinks (`gpu::tests::assert_same_bark`). Collars take the tube's `u_tiles` and `v` by distance along the branch, so the bark carries on across the junction; root segments and domes sit at `v = 0` (hidden in the trunk, or a knot); caps lay their disc out at the bark's real size. Under the pipe model an aged node's radii are not its mature radii scaled, so the tile count and cap size follow the aged radii over their progress: bark may re-tile by a tile while such a tree grows, and differs slightly from `GpuGrowth`'s.
- **`uv1.x` of wood is the vertex's own radius in metres** (`uv1.y` is 1). The fragment shader fades the bark in with it: smooth below `BarkParams::smooth_below`, the whole scan above `fissured_above`, smoothstep between (`bark_surface` in `common.wesl`), so a branch gains relief as it thickens. `growth.wesl` replaces it with `grown_wood_radius` (`growth_common.wesl`; the per-kind scale `grown_position` uses, factored out as `piece_scale`; mirrored by `frame::wood_radius`), so GPU growth fades the bark in as the wood thickens.
- **The base colour is the preset over the scan's mean** (per linear channel, at most 8x; `BarkTexture::base_color`), and the shader multiplies it by `mix(mean, scan colour, fade)`, so wood averages the preset colour at every fade and far trees, impostors (which bake the preset colour) and untextured trees keep their colour. Normals are perturbed with Schüler's cotangent frame from screen-space derivatives (`cotangent_frame`): the meshes carry no tangents. The map is OpenGL's (green up) and `v` runs down the image, so the bitangent is flipped. The relief fades out toward the silhouette (`smoothstep(0.05, 0.4, N·V)`): there a bumped normal turns away from the camera and the light, and the crevices read as black speckle.
- `FloraTextures` decodes each scan on first use (`Image::from_buffer`, CPU mip chain: `with_mip_chain` for the colour map, a vector-averaging chain for the normal map), keeps the handles, and logs one `warn!` naming the file if a scan fails to decode (that scan's trees are plain colour). Materials bind the maps at 103 (colour, with the sampler at 105) and 104 (normal), `FLORA_DETAIL` is on when both are set and the mesh has both UV sets (`FloraKey::detail`, `specialize_flora`), and the scan is part of the `SharedFlora` look, so trees share one material per scan and colour. The plain-material fallback (`Paint::Standard`, `PoolMaterials::Standard`) ignores scans.

## Forests: levels of detail, pools, impostors (`lod.rs`, `pool.rs`, `impostor/`)

A forest cannot give every tree its own meshes (a few thousand oaks of 20 000 triangles saturate the GPU). The forest path builds a few variants once, shares their meshes and materials between every tree placed from them, and draws less of a tree the further away it is. It never touches the CPU or `GpuGrowth` paths; pooled trees are static.

- **Levels of detail are built from the skeleton** (`lod::build_lods`), not by simplifying a mesh, so the wood comes from the same meshers as level 0 and no dependency is needed. Level 0 is `build_aged` unchanged. A coarse level `prune`s the aged skeleton (every non-trunk node thinner than a radius is dropped with its descendants, parent indices remapped, `child_count` kept so terminal-branch decisions do not move, and the nodes whose tips reach furthest along each axis are kept with their ancestors so the silhouette does not shrink), meshes it with `skeleton_mesh_coarse` (a side cap, a ring cap that subsamples the full-resolution axis so every ring is still on the true curve, no collars, a flat disc instead of a root dome, no smoothing, no adaptive sides) on a trunk with half the height segments per level (a trunk with root flares keeps its sides, or the lobes flatten), and takes every `n`th leaf point of the *full* aged tree plus the leaves that reach furthest where the rest fall short, each scaled by up to `sqrt(n)` as far as the room to level 0's bounding box allows. `lod::tests` hold every preset to: triangles strictly fall from level to level; level 2 at most 15% of level 0 (or 500 triangles for the palm, bonsai and dead tree); the bounding box within 10% per axis; valid indices.
- **Instancing is Bevy's.** `TreePool` owns the two materials every variant shares, wood and leaves (`PoolMaterials`: `FloraMaterial`s where the plugin registered the renderer's side, `StandardMaterial`s otherwise; made lazily by `make_pool_materials`, so `build_pool` takes no materials) and, per variant, three wood meshes, up to three leaf meshes and an `Aabb` per level (so `calculate_bounds` does not walk 100 000 copies of a mesh). `spawn_pooled_trees` gives each `PooledTree` a child per mesh and `VisibilityRange` band (`pool::band`: each end a 2 m margin around the distance, the next level's start equal to this level's end, the last end infinite), at most 4096 trees a frame. Entities that share a `Mesh3d` and a `MeshMaterial3d` are batched into one draw by the renderer; `pool::tests` check that 100 trees of two variants draw 12 meshes and 2 materials.
- **Impostors** (`impostor/`): `ImpostorFrame::new(height, radius)` frames a tree's bounding cylinder as the orthographic camera sees it 10° above the horizon, and gives the upright quad that covers the picture. `bake.rs` renders each variant's level 1 meshes, eight copies each turned an eighth of a turn about Y, twice (once per picture), one orthographic camera per copy and picture, each into its own 256x256 viewport of a 2048x512 target. Row 0 holds the colours (`ImpostorBakeMaterial::colour`, render layer 31), row 1 the surface (`ImpostorBakeMaterial::surface`, layer 30): the normal in the tree's own frame, octahedral-encoded, in rg, the crown visibility (`uv1.y`, or 1 for wood) in b and the translucency (1 for leaves, 0 for wood) in a. Nothing is lit; the scene has no light. Cameras, copies and layers are at `BAKE_ORIGIN`, far from the game's world. The clear is transparent both in the main pass and in the output (the upscaling pass clears the target with the camera's `output_mode`, which would otherwise paint the app's sky colour into the atlas), and every camera writes with `blend_state: Some(BlendState::REPLACE)`: `None` means "replace for the first camera of the target, alpha blending for the rest", which drops every surface texel written with alpha 0 (the wood, D-055). No MSAA, no tonemapping and no debanding. At most `MAX_CONCURRENT_BAKES` (4) variants bake at once: every camera costs Metal a few command buffers a frame, and more than 4096 in flight lose the device. A `Screenshot` reads the target back; a capture with fewer opaque colour pixels than 1% of the atlas (the first frames of a new scene render nothing while pipelines compile) is discarded and retaken. The target is read through an sRGB view, so the bytes are sRGB-encoded: `linearize_surface` decodes the surface row (alpha was never encoded), `atlas_image` / `surface_image` build six mip levels in linear light weighted by the colour row's coverage (`surface_mip_chain` averages the encoded normals and the numbers, and keeps a texel with nothing under it at zero), and the pool gets an `ImpostorAtlas` (colour image, surface image, view count, quad size, centre height, material, quad mesh). `impostor.wesl` builds the quad in the vertex stage: the tree's position and scale from `world_from_local`, the quad's right vector along the camera's own right (`up × toward-camera`, ground plane only), the view from the camera's yaw in the tree's local frame, the same `wind_bend` as the meshes, and Bevy's `visibility_range_dither` in the fragment stage. The fragment stage decodes the normal, turns it from the tree's frame into the world with the tree's yaw, and lights it with `apply_pbr_lighting` (the scene's own directional lights, shadows where the cascades reach, ambient light, fog) with the crown visibility as diffuse occlusion and as the directional shadow factor, plus `transmitted_light` scaled by the baked translucency. The material has no prepass or shadow pipeline (`enable_prepass` and `enable_shadows` are `false`), culls nothing, and is alpha-masked. The quad's `Aabb` is set by hand and reaches `size.x / 2` in every horizontal direction, because the quad turns about Y while the culling box turns with the tree.
- Plugin order: `impostor::build` registers `ImpostorMaterial` and the `ImpostorSupport` marker only when the renderer's plugins and `PbrPlugin` are already there, like `gpu::build`; the bake systems run only while the marker exists, and a tree waits for its variant's bake to settle (success or give-up) before it gets children, so a first spawn is a few frames late and later spawns are not.
- Measured numbers, and why windowed numbers were not used, are in `CHANGELOG.md` (0.5.0 and 0.6.0, 'Measured'). Shader compile errors show only in a real render (`cargo run --release --example forest -- --mode pool --trees 400 --screenshot out.png`), never in a headless test.
