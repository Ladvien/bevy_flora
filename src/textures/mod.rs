//! The bundled texture data. The images live in this directory (credits and licences in
//! `CREDITS.md`) and are compiled in by `shading::textures`; the tables generated from them by
//! `tools/prepare_textures.py` are here. Without the `textures` feature there is none of it.

#[cfg(feature = "textures")]
pub(crate) mod leaves;

#[cfg(all(test, feature = "textures"))]
mod tests {
    use bevy::math::Vec2;

    use super::leaves::LEAF_SETS;
    use crate::params::LeafScan;

    fn points(outline: &[[f32; 2]]) -> Vec<Vec2> {
        outline.iter().map(|p| Vec2::from_array(*p)).collect()
    }

    fn area(polygon: &[Vec2]) -> f32 {
        let n = polygon.len();
        (0..n).map(|i| polygon[i].perp_dot(polygon[(i + 1) % n])).sum::<f32>() / 2.0
    }

    fn segments_cross(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
        let side = |p: Vec2, q: Vec2, r: Vec2| (q - p).perp_dot(r - p);
        side(a, b, c) * side(a, b, d) < 0.0 && side(c, d, a) * side(c, d, b) < 0.0
    }

    #[test]
    fn there_is_a_set_for_every_leaf_scan() {
        assert_eq!(LEAF_SETS.len(), LeafScan::ALL.len());
    }

    #[test]
    fn every_scanned_leaf_outline_is_a_simple_polygon_its_triangles_exactly_cover() {
        for (set, scan) in LEAF_SETS.iter().zip(LeafScan::ALL) {
            for (k, leaf) in set.leaves.iter().enumerate() {
                let what = format!("{scan:?} leaf {k}");
                let outline = points(leaf.outline);
                let n = outline.len();
                assert!((3..=24).contains(&n), "{what}: {n} outline points");
                assert_eq!(leaf.uv.len(), n, "{what}: one atlas coordinate per outline point");
                assert!(leaf.aspect > 0.0, "{what}: aspect {}", leaf.aspect);
                for p in &outline {
                    assert!(p.x.abs() <= 1.0 + 1e-4 && p.y.abs() <= 1.0 + 1e-4, "{what}: {p:?} outside the card");
                }
                for uv in leaf.uv {
                    assert!(uv.iter().all(|c| (0.0..=1.0).contains(c)), "{what}: atlas coordinate {uv:?}");
                }
                // Simple: no two edges that do not share a vertex cross.
                for i in 0..n {
                    for j in i + 2..n {
                        if i == 0 && j == n - 1 {
                            continue;
                        }
                        assert!(
                            !segments_cross(outline[i], outline[(i + 1) % n], outline[j], outline[(j + 1) % n]),
                            "{what}: edges {i} and {j} cross"
                        );
                    }
                }
                // Counter-clockwise, as seen from the card's front.
                let polygon_area = area(&outline);
                assert!(polygon_area > 0.05, "{what}: area {polygon_area}");
                // The triangulation covers the polygon exactly, every triangle the right way round.
                assert_eq!(leaf.triangles.len() % 3, 0, "{what}");
                assert_eq!(leaf.triangles.len() / 3, n - 2, "{what}: a triangulation has n - 2 triangles");
                let mut covered = 0.0;
                for t in leaf.triangles.as_chunks::<3>().0 {
                    assert!(t.iter().all(|&i| (i as usize) < n), "{what}: index out of range");
                    let [a, b, c] = t.map(|i| outline[i as usize]);
                    let triangle = (b - a).perp_dot(c - a) / 2.0;
                    assert!(triangle > 0.0, "{what}: clockwise or flat triangle {t:?}");
                    covered += triangle;
                }
                assert!(
                    (covered - polygon_area).abs() <= 0.01 * polygon_area,
                    "{what}: triangles cover {covered}, the polygon is {polygon_area}"
                );
            }
        }
    }
}
