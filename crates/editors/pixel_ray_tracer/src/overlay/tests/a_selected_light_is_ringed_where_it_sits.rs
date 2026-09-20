use super::*;

#[test]
fn a_selected_light_is_ringed_where_it_sits() {
    let light = RayEntity::Light {
        id: 7,
        position: Point::new(64.0, 32.0),
        color_index: 3,
        intensity: 2.0,
    };
    let rect = Rect::from_min_size(pos2(0.0, 0.0), Vec2::splat(128.0));

    let shapes = shapes_of(draw(vec![light], Some(7), Preview::None), rect);

    let rings: Vec<_> = shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Rect {
                rect,
                corner_radius,
                stroke_width,
                ..
            } if *stroke_width > 0.0 => Some((rect.center(), *corner_radius)),
            _ => None,
        })
        .collect();
    assert!(
        rings
            .iter()
            .all(|(center, radius)| *center == pos2(64.0, 32.0) && *radius > 0.0),
        "every ring is centred on the light and round, got {rings:?}"
    );
    assert_eq!(rings.len(), 2, "the light has its own edge and a selection");
}
