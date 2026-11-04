use super::*;

use beui_types::{FillRule, Path};

#[test]
fn an_even_odd_path_leaves_a_hole_inside_its_inner_ring() {
    let ring = |rule| {
        let rings = Path::new()
            .move_to(pos2(8.0, 8.0))
            .line_to(pos2(56.0, 8.0))
            .line_to(pos2(56.0, 56.0))
            .line_to(pos2(8.0, 56.0))
            .close()
            .move_to(pos2(20.0, 20.0))
            .line_to(pos2(44.0, 20.0))
            .line_to(pos2(44.0, 44.0))
            .line_to(pos2(20.0, 44.0))
            .close();
        capture(Color32::BLACK, |painter| {
            painter.fill_path(&rings, rule, Color32::WHITE);
        })
    };
    let even_odd = ring(FillRule::EvenOdd);
    let non_zero = ring(FillRule::NonZero);

    assert_eq!(even_odd.pixel(12, 32), [255, 255, 255, 255]);
    assert_eq!(even_odd.pixel(32, 32), [0, 0, 0, 255]);
    assert_eq!(non_zero.pixel(32, 32), [255, 255, 255, 255]);
}
