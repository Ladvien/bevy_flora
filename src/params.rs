//! All tunable parameters of a tree, grouped by subsystem.
//!
//! Every struct implements [`Default`]; `TreeParams::default()` is the
//! generic "oak-ish" tree. Presets in [`crate::presets`] are plain
//! `TreeParams` values.

use bevy::color::Color;
use bevy::reflect::Reflect;

pub use crate::crown::CrownShape;
pub use crate::growth::GrowthParams;

/// What happens at the top of the trunk.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TrunkTermination {
    /// Flat disc closes the trunk top.
    #[default]
    FlatCap,
    /// Trunk tapers to a point; no cap.
    PointedTip,
    /// A thin leader branch continues upward from the trunk top.
    LeaderBranch,
}

/// Leaf geometry style.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LeafStyle {
    /// Two leaf-shaped cards at 90° (classic pixel-art leaf).
    #[default]
    CrossedPlanes,
    /// One leaf-shaped card.
    SingleQuad,
    /// Octahedron approximation of a leaf cluster.
    ClusterSphere,
    /// Three leaf-shaped cards at 60°.
    StarBurst,
    /// Six thin radiating quads (pine needles).
    NeedleCluster,
}

/// Where leaves are placed on the skeleton.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FoliagePlacement {
    /// Only at the tips of branches without children in the full-grown tree.
    /// A young branch that will grow twigs stays leafless, so leaves never
    /// appear and then vanish as the tree ages.
    #[default]
    TerminalBranches,
    /// Distributed along every branch.
    AllBranches,
    /// Spherical clusters at the tips of branches without children in the
    /// full-grown tree.
    TipClusters,
}

/// Leaf orientation mode.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LeafOrientation {
    /// Horizontally away from the trunk axis.
    #[default]
    RadialOutward,
    /// Along the branch direction.
    FollowBranch,
    /// Random with an upward bias.
    RandomUpward,
    /// Flat with a random yaw.
    HorizontalSpread,
}

/// Trunk shape.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrunkParams {
    /// Trunk height in metres.
    pub height: f32,
    /// Base trunk radius in metres.
    pub radius: f32,
    /// Number of vertices around each trunk ring.
    pub radial_segments: u32,
    /// Number of ring subdivisions along the trunk height.
    pub height_segments: u32,
    /// Top radius as a fraction of `radius` (0 = point, 1 = no taper).
    pub taper: f32,
    /// Taper curve exponent (0.5 = linear-ish, <0.5 early taper, >0.5 gradual).
    pub taper_curve: f32,
    /// Base radius multiplier (1.0 = none, 1.5 = 50% wider base).
    pub flare: f32,
    /// Lean/wobble amount (0 = straight).
    pub randomness: f32,
    /// Number of root ridges around the base (0 = none).
    pub root_flare_count: u32,
    /// Radial extent of root ridges relative to the base radius.
    pub root_flare_spread: f32,
    /// Height of the root ridges as a fraction of trunk height.
    pub root_flare_height: f32,
    /// How the trunk top is finished.
    pub termination: TrunkTermination,
    /// Leader length as a fraction of trunk height.
    pub leader_length: f32,
    /// Leader tip radius as a fraction of its base.
    pub leader_taper: f32,
    /// Grow sub-branches on the leader.
    pub leader_has_branches: bool,
    /// Twist around the trunk axis from base to top, in degrees.
    pub twist: f32,
}

impl Default for TrunkParams {
    fn default() -> Self {
        Self {
            height: 5.0,
            radius: 0.5,
            radial_segments: 8,
            height_segments: 4,
            taper: 0.3,
            taper_curve: 0.5,
            flare: 1.0,
            randomness: 0.0,
            root_flare_count: 0,
            root_flare_spread: 0.5,
            root_flare_height: 0.15,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.15,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 0.0,
        }
    }
}

/// Branch placement, shape and recursion.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BranchParams {
    /// Start of the branch zone as a fraction of trunk height.
    pub start: f32,
    /// End of the branch zone as a fraction of trunk height.
    pub end: f32,
    /// Primary branches per unit of trunk length.
    pub density: f32,
    /// Primary branch length relative to trunk height.
    pub length: f32,
    /// Angle from the trunk in degrees (0 = up, 90 = horizontal).
    pub angle: f32,
    /// Branch base radius relative to the parent radius at the attachment point.
    pub radius_ratio: f32,
    /// Taper from base to tip (0 = none, 1 = point).
    pub taper: f32,
    /// Spiral angle between consecutive primaries (137.5° = golden angle).
    pub phyllotaxis_angle: f32,
    /// Direction randomness.
    pub randomness: f32,
    /// Upward growth tendency.
    pub up_attraction: f32,
    /// Sub-branch recursion depth (0 = none).
    pub recursion: u32,
    /// Sub-branches per branch.
    pub sub_branch_count: u32,
    /// Length multiplier per recursion level.
    pub sub_branch_scale: f32,
    /// Random length variation (0 = uniform).
    pub length_variation: f32,
    /// Sub-branch position bias (-1 = base, 0 = uniform, 1 = tip).
    pub sub_branch_position_bias: f32,
    /// Suppression of upper branches (0 = equal, 1 = strong leader).
    pub apical_dominance: f32,
    /// Horizontal spread (0 = natural, 1 = flat canopy).
    pub flatness: f32,
    /// Angle change with height (-1 = droop at top, +1 = reach up at top).
    pub angle_curve: f32,
    /// Crown-based angle variation (-1 = lower horizontal, +1 = lower vertical).
    pub crown_angle_variation: f32,
    /// Twist along each branch, in degrees.
    pub twist: f32,
    /// Downward bend toward the tip.
    pub gravity_strength: f32,
    /// Resistance to gravity (0 = flexible, 1 = stiff).
    pub stiffness: f32,
    /// Chance that a branch is missing ("broke off").
    pub break_chance: f32,
    /// Shape of the crown envelope that scales branch length by height.
    pub crown_shape: CrownShape,
    /// How strongly `crown_shape` affects branch length (0 = uniform).
    pub crown_influence: f32,
    /// Prevent branches from ending below `floor_level`.
    pub floor_avoidance: bool,
    /// World-space y of the floor, in metres, used when `floor_avoidance` is on.
    pub floor_level: f32,
}

impl Default for BranchParams {
    fn default() -> Self {
        Self {
            start: 0.3,
            end: 0.9,
            density: 1.0,
            length: 0.4,
            angle: 45.0,
            radius_ratio: 0.3,
            taper: 0.7,
            phyllotaxis_angle: 137.5,
            randomness: 0.2,
            up_attraction: 0.1,
            recursion: 1,
            sub_branch_count: 2,
            sub_branch_scale: 0.5,
            length_variation: 0.15,
            sub_branch_position_bias: 0.0,
            apical_dominance: 0.5,
            flatness: 0.0,
            angle_curve: 0.0,
            crown_angle_variation: 0.0,
            twist: 0.0,
            gravity_strength: 0.0,
            stiffness: 0.5,
            break_chance: 0.0,
            crown_shape: CrownShape::Cylindrical,
            crown_influence: 1.0,
            floor_avoidance: true,
            floor_level: 0.0,
        }
    }
}

/// Y-junction splitting of primary branches.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SplitParams {
    /// Whether primary branches may split into Y-junctions.
    pub enabled: bool,
    /// Chance that a primary splits.
    pub probability: f32,
    /// Angle between the two split branches, in degrees.
    pub angle: f32,
    /// Where along the primary the split occurs (0–1).
    pub position: f32,
    /// Minimum base radius for a branch to split.
    pub radius_threshold: f32,
}

impl Default for SplitParams {
    fn default() -> Self {
        Self {
            enabled: false,
            probability: 0.3,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.05,
        }
    }
}

/// Mesh construction options.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeshParams {
    /// Smoothing passes on the wood mesh (0 = off). Volume-preserving (Taubin)
    /// smoothing that softens creases but keeps the ground ring, branch ends,
    /// twig seats, the trunk under collars and hairline twigs in place.
    pub smooth_iterations: u32,
    /// Smoothing strength per pass (0–1).
    pub smooth_factor: f32,
    /// Pick radial segments per branch from its radius.
    pub adaptive_resolution: bool,
    /// Segments per unit radius when `adaptive_resolution` is on.
    pub resolution_scale: f32,
    /// Lower bound on radial segments when `adaptive_resolution` is on.
    pub min_radial_segments: u32,
    /// Upper bound on radial segments when `adaptive_resolution` is on.
    pub max_radial_segments: u32,
    /// Recompute parent radii from children (Da Vinci's pipe model).
    pub pipe_radius_enabled: bool,
    /// Pipe model exponent (2.0 = Da Vinci, ~2.5 = Murray).
    pub pipe_radius_exponent: f32,
    /// Minimum branch radius under the pipe model.
    pub pipe_radius_min: f32,
    /// Emit a flared collar where primaries meet the trunk.
    pub collar_enabled: bool,
    /// Collar length as a multiple of the branch base radius.
    pub collar_length: f32,
}

impl Default for MeshParams {
    fn default() -> Self {
        Self {
            smooth_iterations: 0,
            smooth_factor: 0.5,
            adaptive_resolution: false,
            resolution_scale: 16.0,
            min_radial_segments: 4,
            max_radial_segments: 16,
            pipe_radius_enabled: false,
            pipe_radius_exponent: 2.0,
            pipe_radius_min: 0.02,
            collar_enabled: true,
            collar_length: 1.5,
        }
    }
}

/// Leaf placement and geometry.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FoliageParams {
    /// Whether leaves are generated at all.
    pub enabled: bool,
    /// Leaf mesh style.
    pub style: LeafStyle,
    /// Where leaves are placed on the skeleton.
    pub placement: FoliagePlacement,
    /// How leaves are oriented relative to their branch.
    pub orientation: LeafOrientation,
    /// Leaves per branch (terminal) or per unit length (all branches).
    pub density: f32,
    /// Leaves per cluster for [`FoliagePlacement::TipClusters`].
    pub cluster_size: u32,
    /// Leaf length (base to tip) in metres; leaf cards are leaf-shaped, about half the area of a square of that size.
    pub leaf_size: f32,
    /// Random size variation (0–0.5).
    pub leaf_size_variation: f32,
    /// Branches thicker than this fraction of the trunk radius get no leaves.
    pub radius_threshold: f32,
    /// Density falloff from the bottom to the top of the crown.
    pub height_falloff: f32,
    /// Downward tilt of leaves.
    pub droop: f32,
    /// Random rotation around the leaf direction (0–1).
    pub rotation_variation: f32,
    /// Modulate density by crown height.
    pub use_crown_density: bool,
    /// How far leaf normals bend from each card's own facing toward pointing out of the crown
    /// (0 = flat cards, 1 = a soft round crown); baked into the mesh.
    pub normal_bend: f32,
    /// Per-leaf brightness variation (0–0.5): each leaf's colour is scaled by a factor in 1 ± this.
    pub color_variation: f32,
    /// Leaf base color.
    pub color: Color,
}

impl Default for FoliageParams {
    fn default() -> Self {
        Self {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::TerminalBranches,
            orientation: LeafOrientation::RadialOutward,
            density: 3.0,
            cluster_size: 4,
            leaf_size: 0.3,
            leaf_size_variation: 0.15,
            radius_threshold: 0.15,
            height_falloff: 0.3,
            droop: 0.2,
            rotation_variation: 0.5,
            use_crown_density: true,
            normal_bend: 0.8,
            color_variation: 0.3,
            color: Color::srgb(0.133, 0.545, 0.133),
        }
    }
}

/// Complete description of a tree.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TreeParams {
    /// Random seed; the same params and seed always generate the same tree.
    pub seed: i32,
    /// Trunk shape.
    pub trunk: TrunkParams,
    /// Branch generation.
    pub branches: BranchParams,
    /// Y-junction splitting.
    pub split: SplitParams,
    /// Mesh generation.
    pub mesh: MeshParams,
    /// Leaf placement and geometry.
    pub foliage: FoliageParams,
    /// Growth timing.
    pub growth: GrowthParams,
    /// Trunk and branch color.
    pub trunk_color: Color,
}

impl Default for TreeParams {
    fn default() -> Self {
        Self {
            seed: 42,
            trunk: TrunkParams::default(),
            branches: BranchParams::default(),
            split: SplitParams::default(),
            mesh: MeshParams::default(),
            foliage: FoliageParams::default(),
            growth: GrowthParams::default(),
            trunk_color: Color::srgb(0.545, 0.271, 0.075),
        }
    }
}
