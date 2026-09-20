use super::*;

#[test]
fn rotated_text_lands_where_the_upright_run_was_turned_to() {
    let origin = pos2(20.0, 26.0);
    let pivot = pos2(32.0, 32.0);
    let upright = capture(Color32::BLACK, |painter| {
        painter.text(origin, "IIIII", FontId::proportional(12.0), Color32::WHITE);
    });
    let turned = capture(Color32::BLACK, |painter| {
        painter.rotated(pivot, std::f32::consts::FRAC_PI_2).text(
            origin,
            "IIIII",
            FontId::proportional(12.0),
            Color32::WHITE,
        );
    });

    let upright = ink(&upright);
    let turned = ink(&turned);
    assert!(upright.width() > upright.height());
    assert!(turned.height() > turned.width());
    assert!((upright.width() - turned.height()).abs() <= 2.0);
}

fn ink(capture: &Capture) -> Rect {
    let mut bounds = Rect::NOTHING;
    for y in 0..SIZE {
        for x in 0..SIZE {
            if capture.pixel(x, y)[0] > 64 {
                let point = pos2(x as f32, y as f32);
                bounds = bounds.union(Rect::from_min_max(point, point));
            }
        }
    }
    bounds
}
