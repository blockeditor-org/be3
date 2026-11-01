use super::*;

use beui_types::{LineCap, Path, Stroke};

#[test]
fn a_stroked_path_joins_caps_and_dashes_its_line() {
    let line = Path::polyline([pos2(8.0, 32.0), pos2(56.0, 32.0)]);
    let solid = capture(Color32::BLACK, |painter| {
        painter.stroke_path(&line, &Stroke::new(4.0), Color32::WHITE);
    });
    let dashed = capture(Color32::BLACK, |painter| {
        painter.stroke_path(
            &line,
            &Stroke::new(4.0)
                .cap(LineCap::Square)
                .dashed([8.0, 8.0], 0.0),
            Color32::WHITE,
        );
    });

    assert_eq!(solid.pixel(20, 31), [255, 255, 255, 255]);
    assert_eq!(solid.pixel(20, 34), [0, 0, 0, 255]);
    assert_eq!(solid.pixel(6, 32), [0, 0, 0, 255]);
    assert_eq!(dashed.pixel(12, 32), [255, 255, 255, 255]);
    assert_eq!(dashed.pixel(20, 32), [0, 0, 0, 255]);
    assert_eq!(dashed.pixel(7, 32), [255, 255, 255, 255]);
}
