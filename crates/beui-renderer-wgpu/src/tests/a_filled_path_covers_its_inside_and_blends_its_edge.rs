use super::*;

use beui_types::{FillRule, Path};

#[test]
fn a_filled_path_covers_its_inside_and_blends_its_edge() {
    let center = pos2(32.0, 32.0);
    let disc = Path::ellipse(center, vec2(20.0, 20.0));
    let painted = capture(Color32::BLACK, |painter| {
        painter.fill_path(&disc, FillRule::NonZero, Color32::WHITE);
    });

    let mut blended = 0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let reach = (pos2(x as f32 + 0.5, y as f32 + 0.5) - center).length();
            let [red, ..] = painted.pixel(x, y);
            if reach < 19.0 {
                assert_eq!(red, 255, "({x}, {y}) is inside");
            } else if reach > 21.0 {
                assert_eq!(red, 0, "({x}, {y}) is outside");
            } else if red > 0 && red < 255 {
                blended += 1;
            }
        }
    }
    assert!(blended > 40, "{blended} pixels on the edge are blended");
}
