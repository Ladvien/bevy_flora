# Research corpus

The reading list per domain, by home-still stem (`distill_search` / `markdown_read`), or by URL for
a web source the corpus does not hold. Each system's own note (`docs/research/<system>.md`) quotes
what it rests on; this file only says what exists. Newest additions last in each section.

## Tree architecture and biomechanics

- `10.1371_journal.pone.0093535`: Minamino and Tateno 2014, "Tree branching: Leonardo da Vinci's
  rule versus biomechanical models" (the uniform stress and elastic similarity hypotheses).
- `10.1038_s41467-017-00995-6`: Eloy, Fournier, Lacointe and Moulia 2017, "Wind loads and
  competition for light sculpt trees into self-similar structures" (MECHATREE; wind-induced
  bending stress per branch).
- `10.3389_fpls.2022.971690`: "Stronger wind, smaller tree: testing tree growth plasticity through a
  modeling approach" (2022).
- `10.3389_fpls.2016.01095`: "Why be a shrub? A basic model and hypotheses for the adaptive values of
  a common growth form" (2016).
- `10.48044_jauf.2024.020`: "Tree trimming effects on 3-dimensional crown structure and tree
  biomechanics" (2024; measured sway frequency and displacement).
- `10.1038_s41467-025-60262-x`: "The global spectrum of tree crown architecture" (2025).
- `10.1038_s41467-023-44004-5`: "Edge effects on tree architecture exacerbate biomass loss of
  fragmented Amazonian forests" (2023).
- `10.1111_cgf.12282`: "Inverse procedural modelling of trees".
- `10.1145_3502220`: "Procedural urban forestry".
- `10.1111_cgf.13106`: "Interactive modeling and authoring of climbing plants" (2017; wind
  bending and breaking of branches).
- `10.48550_arxiv.2204.03237`: Yang, Wang and Huo 2022, "Rule-based Procedural Tree Modeling
  Approach" (survey of rule-based tree models; branch surfaces as generalized cylinders swept along
  the skeleton).
- `sig2024_Interactive_Invigoration_Volumetric_Modeling_of_Trees_with_Strands`: "Interactive
  Invigoration: Volumetric Modeling of Trees with Strands" (SIGGRAPH 2024; strand-based branch
  meshes against generalized cylinders, mesh generation times).
- `10.48550_arxiv.2306.09310`: Raistrick et al. 2023, "Infinite Photorealistic Worlds using
  Procedural Generation" (Infinigen: procedural trees and plants in a whole-scene generator).

## Plant growth and branching

- `10.1038_s41580-025-00851-2`: "Mechanisms of auxin action in plant growth and development".
- `10.1038_s41467-020-15893-7`: "Arabidopsis FHY3 and FAR1 integrate light and strigolactone
  signaling to regulate branching".
- `10.1038_s41598-017-14831-w`: "Climate change accelerates growth of urban trees in metropolises
  worldwide".

## Phenology (leaf colour, leaf fall, blossom)

- `10.1038_s41586-022-05092-3`: "Warm springs alter timing but not total growth of temperate
  deciduous trees".
- `10.1038_s41559-022-01668-4`: "An earlier start of the thermal growing season enhances tree growth
  in cold humid areas but not in dry areas".
- `10.1038_s41467-022-28490-7`: "Forest fragmentation impacts the seasonality of Amazonian evergreen
  canopies".
- `10.1038_s41467-026-68989-x`: "Pantropical moist forests are converging towards a middle leaf
  longevity".

## Tree condition and mortality (issue #87)

- `10.1038_s41467-020-18996-3`: "Tree mode of death and mortality risk factors across Amazon
  forests".
- `10.1038_s41559-017-0248-x`: "A multi-species synthesis of physiological mechanisms in
  drought-induced tree mortality".
- `10.1038_s41467-022-29289-2`: "Global field observations of tree die-off reveal hotter-drought
  fingerprint for Earth's forests".
- `10.1038_s41467-020-20455-y`: "Cross-scale interaction of host tree size and climatic water
  deficit governs bark beetle-induced tree mortality".
- `10.1038_s41467-019-12380-6`: "Tree height explains mortality risk during an intense drought".

## Wildfire

- `10.1038_s41598-019-55036-7`: "Short-interval wildfire and drought overwhelm boreal forest
  resilience".
- `10.1038_s41598-017-06747-2`: "Small fire refugia in the grassy matrix and the persistence of
  Afrotemperate forest in the Drakensberg mountains".

## Wind animation (shipping games and technique)

- `GPU Gems- Programming Techniques, Tips and Tricks for -- Randima Fernando; ...` (home-still doc
  id begins with this): GPU Gems 1, chapter 7, Pelzer, "Rendering Countless Blades of Waving Grass"
  (the Codecreatures Benchmark: per-cluster, per-vertex and per-object wind in the vertex shader).
- Web, not indexed: GPU Gems 3, chapter 16, Sousa, "Vegetation Procedural Animation and Shading in
  Crysis",
  <https://developer.nvidia.com/gpugems/gpugems3/part-iii-rendering/chapter-16-vegetation-procedural-animation-and-shading-crysis>.
- Web, not indexed: GPU Gems 3, chapter 6, Zioma, "GPU-Generated Procedural Wind Animations for
  Trees" (Battlefield-era DICE),
  <https://developer.nvidia.com/gpugems/gpugems3/part-i-geometry/chapter-6-gpu-generated-procedural-wind-animations-trees>.

## Rendering, level of detail, impostors

- `book_real_time_rendering_4th_akenine`: Real-Time Rendering, 4th edition (vegetation instancing,
  chapter 18).
- `book_level_of_detail_3d_graphics_luebke`: Luebke et al., "Level of Detail for 3D Graphics".
- `s2008-advances-full-course-notes-5`: SIGGRAPH 2008 "Advances in Real-Time Rendering" course notes
  (CryEngine terrain and vegetation).
- Web, not indexed: SpeedTree Modeler manual (Unity), "Branch generator properties", Segments:
  Radial (Absolute, + Relative from the branch radius) and Dynamic LOD,
  <https://docs.unity3d.com/speedtree-modeler/manual/branch-generator-properties.html>.

## Spatial queries (`FloraIndex`)

- `book_real_time_collision_detection_ericson`: Ericson, "Real-Time Collision Detection", chapter 7
  (uniform grids: dense, sparse and hashed storage; a hashed grid holds only the occupied cells).
- `10.1145_1730804.1730814`: Laine and Karras 2010, "Efficient Sparse Voxel Octrees" (quotes the
  Amanatides and Woo regular-grid traversal: the next-plane `t` values per axis, step along the
  smallest).
- `eth-cgl-sim_anim-Sol14a`: "SPH Fluids in Computer Graphics" (the hashed grid's occupancy:
  "generally sparsely filled").
- `10.1111_j.1467-8659.2005.00829.x`: Teschner et al. 2005, "Collision Detection for Deformable
  Objects" (spatial subdivision and hashing for neighbourhood queries).
- Web, not indexed: Clark and Evans 1954, "Distance to Nearest Neighbor as a Measure of Spatial
  Relationships in Populations", doi `10.2307/1931034` (paywalled); the 1979 generalisation to k
  dimensions, doi `10.2307/1937660`.
