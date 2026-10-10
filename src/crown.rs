//! Crown envelope: modulates primary branch length by height within the crown.

use bevy::reflect::Reflect;
use std::f32::consts::PI;

/// Crown shape envelope that modulates branch length based on height position.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CrownShape {
    /// Constant branch length at every height.
    #[default]
    Cylindrical,
    /// Longest branches at the bottom, shortening linearly to the top (a cone, e.g. pine).
    Conical,
    /// Longest branches at mid-height, rounded at both ends.
    Spherical,
    /// Longest branches at the top of the dome profile, with a sine falloff toward the bottom.
    Hemispherical,
    /// Nearly constant length that shortens gently toward the top.
    TaperedCylindrical,
    /// Short at the base, peaking at about 70% height, then tapering sharply to the top.
    Flame,
    /// Shortest branches at the base, lengthening linearly to the top (a vase).
    InverseConical,
    /// Like `Flame` but peaking at about 60% height with a quadratic falloff to the top.
    TendFlame,
    /// Widest at ~40% height, gradual taper above and below.
    Spreading,
}

impl CrownShape {
    /// Branch length multiplier at normalized crown height (0 = bottom, 1 = top).
    pub fn length_multiplier(self, height_ratio: f32) -> f32 {
        let t = height_ratio.clamp(0.0, 1.0);

        match self {
            CrownShape::Cylindrical => 1.0,
            // Silhouettes: Conical is widest at the bottom (a cone, e.g. pine),
            // InverseConical widest at the top (a vase).
            CrownShape::Conical => 1.0 - 0.8 * t,
            CrownShape::Spherical => 0.2 + 0.8 * (PI * t).sin(),
            CrownShape::Hemispherical => 0.2 + 0.8 * (PI * 0.5 * t).sin(),
            CrownShape::TaperedCylindrical => 1.0 - 0.5 * t,
            CrownShape::Flame => {
                let peak = 0.7;
                if t < peak {
                    0.3 + 0.7 * (t / peak)
                } else {
                    1.0 - 0.8 * ((t - peak) / (1.0 - peak))
                }
            }
            CrownShape::InverseConical => 0.2 + 0.8 * t,
            CrownShape::TendFlame => {
                let peak = 0.6;
                if t < peak {
                    0.4 + 0.6 * (t / peak)
                } else {
                    let falloff = (t - peak) / (1.0 - peak);
                    1.0 - 0.6 * falloff * falloff
                }
            }
            CrownShape::Spreading => {
                let peak = 0.4;
                if t < peak {
                    0.5 + 0.5 * (t / peak)
                } else {
                    let falloff = (t - peak) / (1.0 - peak);
                    1.0 - 0.4 * falloff
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cylindrical_uniform() {
        let shape = CrownShape::Cylindrical;
        assert_eq!(shape.length_multiplier(0.0), 1.0);
        assert_eq!(shape.length_multiplier(0.5), 1.0);
        assert_eq!(shape.length_multiplier(1.0), 1.0);
    }

    #[test]
    fn conical_decreases_with_height() {
        let shape = CrownShape::Conical;
        let bottom = shape.length_multiplier(0.0);
        let middle = shape.length_multiplier(0.5);
        let top = shape.length_multiplier(1.0);
        assert!(bottom > middle);
        assert!(middle > top);
        assert!((bottom - 1.0).abs() < 0.001);
        assert!((top - 0.2).abs() < 0.001);
    }

    #[test]
    fn spherical_peaks_in_middle() {
        let shape = CrownShape::Spherical;
        let bottom = shape.length_multiplier(0.0);
        let middle = shape.length_multiplier(0.5);
        let top = shape.length_multiplier(1.0);
        assert!(middle > bottom);
        assert!(middle > top);
    }

    #[test]
    fn flame_peaks_at_70_percent() {
        let shape = CrownShape::Flame;
        assert!(shape.length_multiplier(0.7) > shape.length_multiplier(0.5));
        assert!(shape.length_multiplier(0.7) > shape.length_multiplier(0.9));
    }

    #[test]
    fn inverse_conical_increases_with_height() {
        let shape = CrownShape::InverseConical;
        let bottom = shape.length_multiplier(0.0);
        let top = shape.length_multiplier(1.0);
        assert!(bottom < top);
        assert!((bottom - 0.2).abs() < 0.001);
        assert!((top - 1.0).abs() < 0.001);
    }

    #[test]
    fn clamping() {
        let shape = CrownShape::Conical;
        assert_eq!(shape.length_multiplier(-0.5), shape.length_multiplier(0.0));
        assert_eq!(shape.length_multiplier(1.5), shape.length_multiplier(1.0));
    }

    #[test]
    fn spreading_peaks_at_40_percent() {
        let shape = CrownShape::Spreading;
        let at_peak = shape.length_multiplier(0.4);
        let before_peak = shape.length_multiplier(0.2);
        let after_peak = shape.length_multiplier(0.7);
        let at_top = shape.length_multiplier(1.0);
        assert!(at_peak > before_peak);
        assert!(at_peak > after_peak);
        assert!(after_peak > at_top);
        assert!((at_peak - 1.0).abs() < 0.001);
    }
}
