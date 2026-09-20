use super::*;

#[test]
fn a_surface_is_drawn_along_the_line_it_spans() {
    let surface = RayEntity::Surface {
        id: 1,
        start: Point::new(8.0, 8.0),
        end: Point::new(24.0, 24.0),
        color_index: 2,
        roughness: 0.0,
        metalness: 0.0,
        transmission: 0.0,
        refractive_index: 1.5,
    };
    let rect = Rect::from_min_size(pos2(100.0, 200.0), Vec2::splat(256.0));

    let shapes = shapes_of(draw(vec![surface], None, Preview::None), rect);

    let drawn: Vec<_> = shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Line { from, to, .. } => Some((*from, *to)),
            _ => None,
        })
        .collect();
    assert_eq!(
        drawn,
        vec![(pos2(116.0, 216.0), pos2(148.0, 248.0))],
        "the surface spans its two ends in the rectangle it was given"
    );
}
