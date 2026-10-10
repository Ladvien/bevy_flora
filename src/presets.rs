//! Built-in tree presets.
use crate::params::*;
use bevy::color::Color;
use bevy::reflect::Reflect;

use crate::crown::CrownShape;

/// A named, ready-made [`TreeParams`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TreePreset {
    /// Broad, round-crowned deciduous tree.
    Oak,
    /// Conical conifer with a straight leader and needles.
    Pine,
    /// Drooping branches with long trailing foliage.
    Willow,
    /// Slender tree with pale bark and a light crown.
    Birch,
    /// Bare trunk with a tuft of fronds at the top.
    Palm,
    /// Tall, narrow, columnar conifer.
    Cypress,
    /// Small, compact potted-style tree.
    Bonsai,
    /// Rounded deciduous tree with lobed foliage.
    Maple,
    /// Dense, narrow conical conifer.
    Spruce,
    /// Tall, narrow deciduous tree with upswept branches.
    Poplar,
    /// Very thick, short trunk with a sparse crown.
    Baobab,
    /// Thick trunk with an umbrella-like crown.
    DragonTree,
    /// Small tree with a delicate, spreading crown.
    JapaneseMaple,
    /// Leafless, broken-branched tree.
    DeadTree,
    /// Very tall, thick-trunked conifer.
    Redwood,
}

impl TreePreset {
    /// Every preset, in display order.
    pub const ALL: [TreePreset; 15] = [
        TreePreset::Oak,
        TreePreset::Pine,
        TreePreset::Willow,
        TreePreset::Birch,
        TreePreset::Palm,
        TreePreset::Cypress,
        TreePreset::Bonsai,
        TreePreset::Maple,
        TreePreset::Spruce,
        TreePreset::Poplar,
        TreePreset::Baobab,
        TreePreset::DragonTree,
        TreePreset::JapaneseMaple,
        TreePreset::DeadTree,
        TreePreset::Redwood,
    ];

    /// snake_case name of the preset.
    pub fn name(self) -> &'static str {
        match self {
            TreePreset::Oak => "oak",
            TreePreset::Pine => "pine",
            TreePreset::Willow => "willow",
            TreePreset::Birch => "birch",
            TreePreset::Palm => "palm",
            TreePreset::Cypress => "cypress",
            TreePreset::Bonsai => "bonsai",
            TreePreset::Maple => "maple",
            TreePreset::Spruce => "spruce",
            TreePreset::Poplar => "poplar",
            TreePreset::Baobab => "baobab",
            TreePreset::DragonTree => "dragon_tree",
            TreePreset::JapaneseMaple => "japanese_maple",
            TreePreset::DeadTree => "dead_tree",
            TreePreset::Redwood => "redwood",
        }
    }

    /// The parameters for this preset.
    pub fn params(self) -> TreeParams {
        match self {
            TreePreset::Oak => oak(),
            TreePreset::Pine => pine(),
            TreePreset::Willow => willow(),
            TreePreset::Birch => birch(),
            TreePreset::Palm => palm(),
            TreePreset::Cypress => cypress(),
            TreePreset::Bonsai => bonsai(),
            TreePreset::Maple => maple(),
            TreePreset::Spruce => spruce(),
            TreePreset::Poplar => poplar(),
            TreePreset::Baobab => baobab(),
            TreePreset::DragonTree => dragon_tree(),
            TreePreset::JapaneseMaple => japanese_maple(),
            TreePreset::DeadTree => dead_tree(),
            TreePreset::Redwood => redwood(),
        }
    }
}

fn oak() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 6.0,
            radius: 0.6,
            radial_segments: 8,
            height_segments: 8,
            taper: 0.3,
            taper_curve: 0.5,
            flare: 1.2,
            randomness: 0.05,
            root_flare_count: 5,
            root_flare_spread: 0.6,
            root_flare_height: 0.15,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 0.1,
            leader_taper: 0.15,
            leader_has_branches: true,
            twist: 15.0,
        },
        branches: BranchParams {
            start: 0.35,
            end: 0.85,
            density: 3.0,
            length: 0.65,
            angle: 55.0,
            radius_ratio: 0.35,
            taper: 0.7,
            phyllotaxis_angle: 137.5,
            randomness: 0.25,
            up_attraction: 0.15,
            recursion: 2,
            sub_branch_count: 3,
            sub_branch_scale: 0.55,
            length_variation: 0.2,
            sub_branch_position_bias: 0.2,
            apical_dominance: 0.3,
            flatness: 0.3,
            angle_curve: 0.0,
            crown_angle_variation: -0.2,
            twist: 10.0,
            gravity_strength: 0.3,
            stiffness: 0.6,
            break_chance: 0.05,
            crown_shape: CrownShape::Spherical,
            crown_influence: 0.9,
            floor_avoidance: true,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: true,
            probability: 0.3,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.1,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::RadialOutward,
            density: 6.0,
            cluster_size: 5,
            leaf_size: 0.33,
            leaf_size_variation: 0.2,
            radius_threshold: 0.2,
            height_falloff: 0.25,
            droop: 0.15,
            rotation_variation: 0.6,
            use_crown_density: true,
            color: Color::srgb(0.176, 0.314, 0.086),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.365, 0.227, 0.102),
    }
}

fn pine() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 8.0,
            radius: 0.4,
            radial_segments: 8,
            height_segments: 5,
            taper: 0.1,
            taper_curve: 0.7,
            flare: 1.1,
            randomness: 0.0,
            root_flare_count: 0,
            root_flare_spread: 0.0,
            root_flare_height: 0.15,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 0.2,
            leader_taper: 0.05,
            leader_has_branches: false,
            twist: 0.0,
        },
        branches: BranchParams {
            start: 0.25,
            end: 0.95,
            density: 3.6,
            length: 0.45,
            angle: 70.0,
            radius_ratio: 0.25,
            taper: 0.8,
            phyllotaxis_angle: 72.0,
            randomness: 0.15,
            up_attraction: -0.1,
            recursion: 1,
            sub_branch_count: 3,
            sub_branch_scale: 0.4,
            length_variation: 0.1,
            sub_branch_position_bias: 0.0,
            apical_dominance: 0.8,
            flatness: 0.0,
            angle_curve: 0.3,
            crown_angle_variation: -0.3,
            twist: 5.0,
            gravity_strength: 0.05,
            stiffness: 0.8,
            break_chance: 0.0,
            crown_shape: CrownShape::Conical,
            crown_influence: 1.0,
            floor_avoidance: true,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: false,
            probability: 0.0,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.08,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::NeedleCluster,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::FollowBranch,
            density: 5.0,
            cluster_size: 6,
            leaf_size: 0.25,
            leaf_size_variation: 0.1,
            radius_threshold: 0.25,
            height_falloff: 0.2,
            droop: 0.05,
            rotation_variation: 0.3,
            use_crown_density: true,
            color: Color::srgb(0.106, 0.302, 0.243),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.420, 0.267, 0.137),
    }
}

fn willow() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 5.0,
            radius: 0.5,
            radial_segments: 8,
            height_segments: 8,
            taper: 0.4,
            taper_curve: 0.4,
            flare: 1.0,
            randomness: 0.1,
            root_flare_count: 4,
            root_flare_spread: 0.4,
            root_flare_height: 0.15,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.15,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 10.0,
        },
        branches: BranchParams {
            start: 0.4,
            end: 0.9,
            density: 2.0,
            length: 0.7,
            angle: 50.0,
            radius_ratio: 0.2,
            taper: 0.85,
            phyllotaxis_angle: 137.5,
            randomness: 0.3,
            up_attraction: -0.75,
            recursion: 3,
            sub_branch_count: 3,
            sub_branch_scale: 0.7,
            length_variation: 0.25,
            sub_branch_position_bias: 0.3,
            apical_dominance: 0.2,
            flatness: 0.2,
            angle_curve: -0.4,
            crown_angle_variation: -0.3,
            twist: 15.0,
            gravity_strength: 0.5,
            stiffness: 0.2,
            break_chance: 0.0,
            crown_shape: CrownShape::Hemispherical,
            crown_influence: 0.8,
            floor_avoidance: true,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: false,
            probability: 0.0,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.05,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::SingleQuad,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::RandomUpward,
            density: 6.0,
            cluster_size: 4,
            leaf_size: 0.26,
            leaf_size_variation: 0.25,
            radius_threshold: 0.15,
            height_falloff: 0.4,
            droop: 0.6,
            rotation_variation: 0.8,
            use_crown_density: true,
            color: Color::srgb(0.565, 0.690, 0.376),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.478, 0.361, 0.239),
    }
}

fn birch() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 7.0,
            radius: 0.3,
            radial_segments: 6,
            height_segments: 5,
            taper: 0.2,
            taper_curve: 0.5,
            flare: 1.0,
            randomness: 0.15,
            root_flare_count: 0,
            root_flare_spread: 0.0,
            root_flare_height: 0.15,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 0.15,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 5.0,
        },
        branches: BranchParams {
            start: 0.4,
            end: 0.95,
            density: 3.0,
            length: 0.35,
            angle: 40.0,
            radius_ratio: 0.2,
            taper: 0.75,
            phyllotaxis_angle: 137.5,
            randomness: 0.2,
            up_attraction: 0.3,
            recursion: 1,
            sub_branch_count: 3,
            sub_branch_scale: 0.5,
            length_variation: 0.15,
            sub_branch_position_bias: 0.1,
            apical_dominance: 0.55,
            flatness: 0.1,
            angle_curve: 0.2,
            crown_angle_variation: 0.1,
            twist: 8.0,
            gravity_strength: 0.1,
            stiffness: 0.7,
            break_chance: 0.1,
            crown_shape: CrownShape::TaperedCylindrical,
            crown_influence: 0.7,
            floor_avoidance: true,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: true,
            probability: 0.2,
            angle: 25.0,
            position: 0.5,
            radius_threshold: 0.05,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::TerminalBranches,
            orientation: LeafOrientation::RandomUpward,
            density: 3.5,
            cluster_size: 4,
            leaf_size: 0.26,
            leaf_size_variation: 0.15,
            radius_threshold: 0.12,
            height_falloff: 0.35,
            droop: 0.1,
            rotation_variation: 0.5,
            use_crown_density: true,
            color: Color::srgb(0.486, 0.804, 0.486),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.961, 0.961, 0.863),
    }
}

fn palm() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 6.0,
            radius: 0.35,
            radial_segments: 8,
            height_segments: 6,
            taper: 0.35,
            taper_curve: 0.3,
            flare: 1.3,
            randomness: 0.0,
            root_flare_count: 0,
            root_flare_spread: 0.0,
            root_flare_height: 0.15,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.15,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 0.0,
        },
        branches: BranchParams {
            start: 0.85,
            end: 0.98,
            density: 8.0,
            length: 0.6,
            angle: 60.0,
            radius_ratio: 0.28,
            taper: 0.9,
            phyllotaxis_angle: 45.0,
            randomness: 0.1,
            up_attraction: -0.2,
            recursion: 0,
            sub_branch_count: 0,
            sub_branch_scale: 0.5,
            length_variation: 0.1,
            sub_branch_position_bias: 0.0,
            apical_dominance: 0.9,
            flatness: 0.0,
            angle_curve: 0.0,
            crown_angle_variation: 0.0,
            twist: 0.0,
            gravity_strength: 0.15,
            stiffness: 0.5,
            break_chance: 0.0,
            crown_shape: CrownShape::Cylindrical,
            crown_influence: 0.5,
            floor_avoidance: true,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: false,
            probability: 0.0,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.1,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::SingleQuad,
            placement: FoliagePlacement::TerminalBranches,
            orientation: LeafOrientation::FollowBranch,
            density: 1.5,
            cluster_size: 2,
            leaf_size: 1.05,
            leaf_size_variation: 0.2,
            radius_threshold: 0.3,
            height_falloff: 0.1,
            droop: 0.4,
            rotation_variation: 0.2,
            use_crown_density: false,
            color: Color::srgb(0.133, 0.545, 0.133),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.545, 0.451, 0.333),
    }
}

fn cypress() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 10.0,
            radius: 0.4,
            radial_segments: 8,
            height_segments: 6,
            taper: 0.15,
            taper_curve: 0.6,
            flare: 1.0,
            randomness: 0.0,
            root_flare_count: 0,
            root_flare_spread: 0.0,
            root_flare_height: 0.15,
            termination: TrunkTermination::PointedTip,
            leader_length: 0.15,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 5.0,
        },
        branches: BranchParams {
            start: 0.1,
            end: 0.95,
            density: 3.0,
            length: 0.25,
            angle: 18.0,
            radius_ratio: 0.28,
            taper: 0.7,
            phyllotaxis_angle: 137.5,
            randomness: 0.1,
            up_attraction: 0.85,
            recursion: 1,
            sub_branch_count: 2,
            sub_branch_scale: 0.4,
            length_variation: 0.05,
            sub_branch_position_bias: -0.2,
            apical_dominance: 0.9,
            flatness: 0.0,
            angle_curve: 0.5,
            crown_angle_variation: 0.4,
            twist: 3.0,
            gravity_strength: 0.0,
            stiffness: 0.9,
            break_chance: 0.0,
            crown_shape: CrownShape::Flame,
            crown_influence: 1.0,
            floor_avoidance: true,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: false,
            probability: 0.0,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.1,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::ClusterSphere,
            placement: FoliagePlacement::TipClusters,
            orientation: LeafOrientation::RadialOutward,
            density: 4.0,
            cluster_size: 8,
            leaf_size: 0.15,
            leaf_size_variation: 0.1,
            radius_threshold: 0.18,
            height_falloff: 0.15,
            droop: 0.05,
            rotation_variation: 0.4,
            use_crown_density: true,
            color: Color::srgb(0.208, 0.369, 0.231),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.290, 0.235, 0.165),
    }
}

fn bonsai() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 2.0,
            radius: 0.25,
            radial_segments: 8,
            height_segments: 8,
            taper: 0.25,
            taper_curve: 0.4,
            flare: 1.4,
            randomness: 0.25,
            root_flare_count: 3,
            root_flare_spread: 0.8,
            root_flare_height: 0.15,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.15,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 30.0,
        },
        branches: BranchParams {
            start: 0.3,
            end: 0.8,
            density: 2.5,
            length: 0.5,
            angle: 60.0,
            radius_ratio: 0.3,
            taper: 0.6,
            phyllotaxis_angle: 90.0,
            randomness: 0.5,
            up_attraction: 0.0,
            recursion: 2,
            sub_branch_count: 2,
            sub_branch_scale: 0.6,
            length_variation: 0.3,
            sub_branch_position_bias: 0.4,
            apical_dominance: 0.4,
            flatness: 0.4,
            angle_curve: -0.2,
            crown_angle_variation: -0.1,
            twist: 20.0,
            gravity_strength: 0.15,
            stiffness: 0.4,
            break_chance: 0.1,
            crown_shape: CrownShape::Spherical,
            crown_influence: 0.6,
            floor_avoidance: true,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: true,
            probability: 0.4,
            angle: 35.0,
            position: 0.5,
            radius_threshold: 0.06,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::StarBurst,
            placement: FoliagePlacement::TipClusters,
            orientation: LeafOrientation::HorizontalSpread,
            density: 2.5,
            cluster_size: 6,
            leaf_size: 0.33,
            leaf_size_variation: 0.2,
            radius_threshold: 0.2,
            height_falloff: 0.2,
            droop: 0.15,
            rotation_variation: 0.7,
            use_crown_density: true,
            color: Color::srgb(0.208, 0.369, 0.231),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.361, 0.251, 0.200),
    }
}

fn maple() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 8.0,
            radius: 0.5,
            radial_segments: 8,
            height_segments: 8,
            taper: 0.35,
            taper_curve: 0.5,
            flare: 1.15,
            randomness: 0.05,
            root_flare_count: 4,
            root_flare_spread: 0.5,
            root_flare_height: 0.15,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 0.1,
            leader_taper: 0.12,
            leader_has_branches: true,
            twist: 10.0,
        },
        branches: BranchParams {
            start: 0.35,
            end: 0.9,
            density: 3.0,
            length: 0.55,
            angle: 52.0,
            radius_ratio: 0.32,
            taper: 0.7,
            phyllotaxis_angle: 137.5,
            randomness: 0.2,
            up_attraction: 0.1,
            recursion: 2,
            sub_branch_count: 3,
            sub_branch_scale: 0.55,
            length_variation: 0.2,
            sub_branch_position_bias: 0.15,
            apical_dominance: 0.45,
            flatness: 0.25,
            angle_curve: 0.0,
            crown_angle_variation: -0.15,
            twist: 8.0,
            gravity_strength: 0.25,
            stiffness: 0.55,
            break_chance: 0.05,
            crown_shape: CrownShape::Spherical,
            crown_influence: 0.85,
            floor_avoidance: true,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: true,
            probability: 0.25,
            angle: 28.0,
            position: 0.5,
            radius_threshold: 0.1,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::RadialOutward,
            density: 4.5,
            cluster_size: 5,
            leaf_size: 0.39,
            leaf_size_variation: 0.2,
            radius_threshold: 0.18,
            height_falloff: 0.25,
            droop: 0.12,
            rotation_variation: 0.6,
            use_crown_density: true,
            color: Color::srgb(0.133, 0.322, 0.133),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.32, 0.21, 0.12),
    }
}

fn spruce() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 10.0,
            radius: 0.35,
            radial_segments: 8,
            height_segments: 6,
            taper: 0.12,
            taper_curve: 0.8,
            flare: 1.05,
            randomness: 0.0,
            root_flare_count: 0,
            root_flare_spread: 0.0,
            root_flare_height: 0.15,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 0.25,
            leader_taper: 0.03,
            leader_has_branches: false,
            twist: 0.0,
        },
        branches: BranchParams {
            start: 0.12,
            end: 0.95,
            density: 4.4,
            length: 0.5,
            angle: 78.0,
            radius_ratio: 0.22,
            taper: 0.82,
            phyllotaxis_angle: 72.0,
            randomness: 0.1,
            up_attraction: -0.15,
            recursion: 1,
            sub_branch_count: 3,
            sub_branch_scale: 0.35,
            length_variation: 0.08,
            sub_branch_position_bias: 0.0,
            apical_dominance: 0.88,
            flatness: 0.0,
            angle_curve: 0.35,
            crown_angle_variation: -0.3,
            twist: 3.0,
            gravity_strength: 0.08,
            stiffness: 0.75,
            break_chance: 0.0,
            crown_shape: CrownShape::Conical,
            crown_influence: 1.0,
            floor_avoidance: true,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: false,
            probability: 0.0,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.08,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::NeedleCluster,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::FollowBranch,
            density: 5.5,
            cluster_size: 7,
            leaf_size: 0.22,
            leaf_size_variation: 0.08,
            radius_threshold: 0.22,
            height_falloff: 0.18,
            droop: 0.08,
            rotation_variation: 0.25,
            use_crown_density: true,
            color: Color::srgb(0.086, 0.278, 0.212),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.380, 0.243, 0.141),
    }
}

fn poplar() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 12.0,
            radius: 0.35,
            radial_segments: 8,
            height_segments: 7,
            taper: 0.12,
            taper_curve: 0.7,
            flare: 1.0,
            randomness: 0.0,
            root_flare_count: 0,
            root_flare_spread: 0.0,
            root_flare_height: 0.15,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 0.2,
            leader_taper: 0.05,
            leader_has_branches: false,
            twist: 0.0,
        },
        branches: BranchParams {
            start: 0.2,
            end: 0.95,
            density: 1.6,
            length: 0.3,
            angle: 15.0,
            radius_ratio: 0.2,
            taper: 0.75,
            phyllotaxis_angle: 137.5,
            randomness: 0.05,
            up_attraction: 0.92,
            recursion: 1,
            sub_branch_count: 2,
            sub_branch_scale: 0.4,
            length_variation: 0.1,
            sub_branch_position_bias: -0.1,
            apical_dominance: 0.92,
            flatness: 0.0,
            angle_curve: 0.4,
            crown_angle_variation: 0.3,
            twist: 2.0,
            gravity_strength: 0.0,
            stiffness: 0.9,
            break_chance: 0.0,
            crown_shape: CrownShape::Flame,
            crown_influence: 1.0,
            floor_avoidance: true,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: false,
            probability: 0.0,
            angle: 20.0,
            position: 0.5,
            radius_threshold: 0.1,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::TerminalBranches,
            orientation: LeafOrientation::RandomUpward,
            density: 3.5,
            cluster_size: 4,
            leaf_size: 0.29,
            leaf_size_variation: 0.12,
            radius_threshold: 0.15,
            height_falloff: 0.3,
            droop: 0.05,
            rotation_variation: 0.45,
            use_crown_density: true,
            color: Color::srgb(0.404, 0.545, 0.306),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.553, 0.514, 0.435),
    }
}

fn baobab() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 6.0,
            radius: 1.5,
            radial_segments: 12,
            height_segments: 6,
            taper: 0.5,
            taper_curve: 0.3,
            flare: 1.6,
            randomness: 0.08,
            root_flare_count: 6,
            root_flare_spread: 0.7,
            root_flare_height: 0.2,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.05,
            leader_taper: 0.2,
            leader_has_branches: false,
            twist: 5.0,
        },
        branches: BranchParams {
            start: 0.75,
            end: 0.95,
            density: 3.0,
            length: 0.45,
            angle: 55.0,
            radius_ratio: 0.35,
            taper: 0.6,
            phyllotaxis_angle: 137.5,
            randomness: 0.35,
            up_attraction: 0.2,
            recursion: 2,
            sub_branch_count: 3,
            sub_branch_scale: 0.5,
            length_variation: 0.25,
            sub_branch_position_bias: 0.3,
            apical_dominance: 0.25,
            flatness: 0.3,
            angle_curve: -0.2,
            crown_angle_variation: -0.2,
            twist: 12.0,
            gravity_strength: 0.15,
            stiffness: 0.5,
            break_chance: 0.1,
            crown_shape: CrownShape::Hemispherical,
            crown_influence: 0.7,
            floor_avoidance: true,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: true,
            probability: 0.3,
            angle: 35.0,
            position: 0.6,
            radius_threshold: 0.15,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.8,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::ClusterSphere,
            placement: FoliagePlacement::TipClusters,
            orientation: LeafOrientation::RadialOutward,
            density: 2.0,
            cluster_size: 5,
            leaf_size: 0.4,
            leaf_size_variation: 0.25,
            radius_threshold: 0.25,
            height_falloff: 0.15,
            droop: 0.2,
            rotation_variation: 0.5,
            use_crown_density: false,
            color: Color::srgb(0.306, 0.463, 0.235),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.506, 0.424, 0.369),
    }
}

fn dragon_tree() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 5.0,
            radius: 0.4,
            radial_segments: 8,
            height_segments: 8,
            taper: 0.3,
            taper_curve: 0.5,
            flare: 1.1,
            randomness: 0.05,
            root_flare_count: 3,
            root_flare_spread: 0.4,
            root_flare_height: 0.15,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.05,
            leader_taper: 0.15,
            leader_has_branches: false,
            twist: 8.0,
        },
        branches: BranchParams {
            start: 0.5,
            end: 0.9,
            density: 1.0,
            length: 0.4,
            angle: 45.0,
            radius_ratio: 0.3,
            taper: 0.5,
            phyllotaxis_angle: 180.0,
            randomness: 0.15,
            up_attraction: 0.25,
            recursion: 3,
            sub_branch_count: 2,
            sub_branch_scale: 0.7,
            length_variation: 0.15,
            sub_branch_position_bias: 0.5,
            apical_dominance: 0.35,
            flatness: 0.15,
            angle_curve: 0.1,
            crown_angle_variation: 0.0,
            twist: 5.0,
            gravity_strength: 0.1,
            stiffness: 0.65,
            break_chance: 0.0,
            crown_shape: CrownShape::Hemispherical,
            crown_influence: 0.6,
            floor_avoidance: true,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: true,
            probability: 1.0,
            angle: 40.0,
            position: 0.5,
            radius_threshold: 0.08,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.6,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::StarBurst,
            placement: FoliagePlacement::TipClusters,
            orientation: LeafOrientation::RadialOutward,
            density: 3.0,
            cluster_size: 8,
            leaf_size: 0.46,
            leaf_size_variation: 0.15,
            radius_threshold: 0.2,
            height_falloff: 0.1,
            droop: 0.1,
            rotation_variation: 0.4,
            use_crown_density: false,
            color: Color::srgb(0.286, 0.443, 0.255),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.463, 0.396, 0.322),
    }
}

fn japanese_maple() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 3.5,
            radius: 0.15,
            radial_segments: 6,
            height_segments: 12,
            taper: 0.25,
            taper_curve: 0.4,
            flare: 1.1,
            randomness: 0.15,
            root_flare_count: 3,
            root_flare_spread: 0.5,
            root_flare_height: 0.1,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.08,
            leader_taper: 0.1,
            leader_has_branches: false,
            twist: 15.0,
        },
        branches: BranchParams {
            start: 0.25,
            end: 0.85,
            density: 2.5,
            length: 0.45,
            angle: 65.0,
            radius_ratio: 0.25,
            taper: 0.7,
            phyllotaxis_angle: 180.0,
            randomness: 0.25,
            up_attraction: -0.15,
            recursion: 2,
            sub_branch_count: 3,
            sub_branch_scale: 0.55,
            length_variation: 0.2,
            sub_branch_position_bias: 0.2,
            apical_dominance: 0.35,
            flatness: 0.65,
            angle_curve: -0.3,
            crown_angle_variation: -0.25,
            twist: 12.0,
            gravity_strength: 0.2,
            stiffness: 0.45,
            break_chance: 0.05,
            crown_shape: CrownShape::Spreading,
            crown_influence: 0.8,
            floor_avoidance: true,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: true,
            probability: 0.2,
            angle: 30.0,
            position: 0.4,
            radius_threshold: 0.05,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.4,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::CrossedPlanes,
            placement: FoliagePlacement::AllBranches,
            orientation: LeafOrientation::HorizontalSpread,
            density: 4.0,
            cluster_size: 4,
            leaf_size: 0.23,
            leaf_size_variation: 0.15,
            radius_threshold: 0.12,
            height_falloff: 0.35,
            droop: 0.18,
            rotation_variation: 0.65,
            use_crown_density: true,
            color: Color::srgb(0.698, 0.133, 0.133),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.376, 0.282, 0.212),
    }
}

fn dead_tree() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 5.0,
            radius: 0.4,
            radial_segments: 8,
            height_segments: 8,
            taper: 0.35,
            taper_curve: 0.5,
            flare: 1.2,
            randomness: 0.2,
            root_flare_count: 4,
            root_flare_spread: 0.6,
            root_flare_height: 0.15,
            termination: TrunkTermination::FlatCap,
            leader_length: 0.05,
            leader_taper: 0.2,
            leader_has_branches: false,
            twist: 40.0,
        },
        branches: BranchParams {
            start: 0.2,
            end: 0.7,
            density: 0.8,
            length: 0.4,
            angle: 50.0,
            radius_ratio: 0.35,
            taper: 0.6,
            phyllotaxis_angle: 137.5,
            randomness: 0.45,
            up_attraction: 0.0,
            recursion: 2,
            sub_branch_count: 2,
            sub_branch_scale: 0.5,
            length_variation: 0.35,
            sub_branch_position_bias: 0.3,
            apical_dominance: 0.3,
            flatness: 0.2,
            angle_curve: 0.0,
            crown_angle_variation: 0.0,
            twist: 25.0,
            gravity_strength: 0.4,
            stiffness: 0.35,
            break_chance: 0.35,
            crown_shape: CrownShape::Spherical,
            crown_influence: 0.4,
            floor_avoidance: false,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: true,
            probability: 0.25,
            angle: 40.0,
            position: 0.4,
            radius_threshold: 0.1,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.6,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: false,
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.388, 0.361, 0.325),
    }
}

fn redwood() -> TreeParams {
    TreeParams {
        seed: 42,
        trunk: TrunkParams {
            height: 20.0,
            radius: 1.0,
            radial_segments: 12,
            height_segments: 10,
            taper: 0.05,
            taper_curve: 0.8,
            flare: 1.3,
            randomness: 0.02,
            root_flare_count: 6,
            root_flare_spread: 0.5,
            root_flare_height: 0.2,
            termination: TrunkTermination::LeaderBranch,
            leader_length: 0.15,
            leader_taper: 0.05,
            leader_has_branches: false,
            twist: 0.0,
        },
        branches: BranchParams {
            start: 0.5,
            end: 0.95,
            density: 2.4,
            length: 0.35,
            angle: 75.0,
            radius_ratio: 0.25,
            taper: 0.8,
            phyllotaxis_angle: 137.5,
            randomness: 0.1,
            up_attraction: -0.1,
            recursion: 1,
            sub_branch_count: 2,
            sub_branch_scale: 0.4,
            length_variation: 0.1,
            sub_branch_position_bias: 0.0,
            apical_dominance: 0.85,
            flatness: 0.0,
            angle_curve: 0.25,
            crown_angle_variation: 0.2,
            twist: 3.0,
            gravity_strength: 0.15,
            stiffness: 0.85,
            break_chance: 0.02,
            crown_shape: CrownShape::Conical,
            crown_influence: 0.9,
            floor_avoidance: true,
            floor_level: 0.0,
        },
        split: SplitParams {
            enabled: false,
            probability: 0.0,
            angle: 30.0,
            position: 0.5,
            radius_threshold: 0.1,
        },
        mesh: MeshParams {
            collar_enabled: true,
            collar_length: 1.5,
            ..Default::default()
        },
        foliage: FoliageParams {
            enabled: true,
            style: LeafStyle::NeedleCluster,
            placement: FoliagePlacement::TerminalBranches,
            orientation: LeafOrientation::FollowBranch,
            density: 3.0,
            cluster_size: 5,
            leaf_size: 0.18,
            leaf_size_variation: 0.1,
            radius_threshold: 0.15,
            height_falloff: 0.25,
            droop: 0.1,
            rotation_variation: 0.3,
            use_crown_density: true,
            color: Color::srgb(0.12, 0.32, 0.21),
            ..Default::default()
        },
        growth: GrowthParams::default(),
        trunk_color: Color::srgb(0.463, 0.263, 0.161),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oak_values() {
        let oak = TreePreset::Oak.params();
        assert_eq!(oak.branches.crown_shape, CrownShape::Spherical);
        assert!(oak.branches.angle > 45.0);
    }

    #[test]
    fn pine_values() {
        let pine = TreePreset::Pine.params();
        assert_eq!(pine.branches.crown_shape, CrownShape::Conical);
        assert!(pine.branches.up_attraction < 0.0);
    }

    #[test]
    fn willow_values() {
        let willow = TreePreset::Willow.params();
        assert_eq!(willow.branches.crown_shape, CrownShape::Hemispherical);
        assert!(willow.branches.up_attraction < -0.3);
    }

    #[test]
    fn palm_values() {
        let palm = TreePreset::Palm.params();
        assert!(palm.branches.start > 0.8);
        assert_eq!(palm.branches.recursion, 0);
    }

    #[test]
    fn cypress_values() {
        let cypress = TreePreset::Cypress.params();
        assert_eq!(cypress.branches.crown_shape, CrownShape::Flame);
        assert!(cypress.branches.up_attraction > 0.5);
        assert!(cypress.branches.angle < 30.0);
    }

    #[test]
    fn spruce_whorled_branching() {
        let spruce = TreePreset::Spruce.params();
        assert_eq!(spruce.branches.phyllotaxis_angle, 72.0);
        assert!(spruce.branches.angle > 70.0);
        assert!(spruce.branches.apical_dominance > 0.85);
    }

    #[test]
    fn poplar_columnar() {
        let poplar = TreePreset::Poplar.params();
        assert!(poplar.branches.up_attraction > 0.9);
        assert!(poplar.branches.angle < 20.0);
        assert!(poplar.branches.apical_dominance > 0.9);
    }

    #[test]
    fn baobab_bottle_trunk() {
        let baobab = TreePreset::Baobab.params();
        assert!(baobab.trunk.radius > 1.0);
        assert!(baobab.branches.start > 0.7);
    }

    #[test]
    fn dragon_tree_forking() {
        let dragon = TreePreset::DragonTree.params();
        assert_eq!(dragon.split.probability, 1.0);
        assert_eq!(dragon.branches.sub_branch_count, 2);
    }

    #[test]
    fn japanese_maple_ornamental() {
        let jmaple = TreePreset::JapaneseMaple.params();
        assert!(jmaple.branches.flatness > 0.5);
        assert_eq!(jmaple.branches.phyllotaxis_angle, 180.0);
        assert_eq!(jmaple.branches.crown_shape, CrownShape::Spreading);
    }

    #[test]
    fn dead_tree_no_foliage() {
        let dead = TreePreset::DeadTree.params();
        assert!(!dead.foliage.enabled);
        assert!(dead.branches.break_chance > 0.3);
        assert!(dead.trunk.twist > 30.0);
    }

    #[test]
    fn redwood_massive() {
        let redwood = TreePreset::Redwood.params();
        assert!(redwood.trunk.height >= 20.0);
        assert!(redwood.trunk.radius >= 1.0);
        assert!(redwood.trunk.taper < 0.15);
    }

    #[test]
    fn foliage_preset_oak() {
        let oak = TreePreset::Oak.params().foliage;
        assert_eq!(oak.style, LeafStyle::CrossedPlanes);
        assert_eq!(oak.placement, FoliagePlacement::AllBranches);
        assert!(oak.density > 3.0);
    }

    #[test]
    fn foliage_preset_pine() {
        let pine = TreePreset::Pine.params().foliage;
        assert_eq!(pine.style, LeafStyle::NeedleCluster);
        assert_eq!(pine.orientation, LeafOrientation::FollowBranch);
    }

    #[test]
    fn foliage_preset_palm() {
        let palm = TreePreset::Palm.params().foliage;
        assert_eq!(palm.placement, FoliagePlacement::TerminalBranches);
        assert!(palm.leaf_size > 0.5);
        assert!(palm.density < 2.0);
    }

    #[test]
    fn all_presets_have_distinct_names() {
        assert_eq!(TreePreset::ALL.len(), 15);
        let mut names: Vec<_> = TreePreset::ALL.iter().map(|p| p.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 15);
    }
}
