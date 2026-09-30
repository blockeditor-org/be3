use super::*;

use beui_core::geometry::{Rect, pos2};

#[test]
fn a_scaled_layer_is_clipped_on_the_screen_and_scaled_inside() {
    let clip = Rect::from_min_max(pos2(40.0, 30.0), pos2(140.0, 90.0));
    let (wrap, scaled) = style::layer(clip, 2.0, 1.0);
    assert_eq!(
        wrap,
        "left:40px;width:100px;overflow-x:clip;top:30px;height:60px;overflow-y:clip;"
    );
    assert_eq!(scaled, "left:-40px;top:-30px;transform:scale(2);");

    let (_, unscaled) = style::layer(clip, 1.0, 1.0);
    assert_eq!(unscaled, "left:-40px;top:-30px;");
}
