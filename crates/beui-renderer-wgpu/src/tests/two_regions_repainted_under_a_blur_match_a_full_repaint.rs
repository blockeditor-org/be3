use super::*;

#[test]
fn two_regions_repainted_under_a_blur_match_a_full_repaint() {
    let blur = || Filter {
        region: everything(),
        blur: 2.0,
        ..Filter::default()
    };
    let square = |left: f32, top: f32| Rect::from_min_max(pos2(left, top), pos2(left + 6.0, top + 6.0));
    let painted = |shift: f32| {
        move |painter: &Painter| {
            painter.rect_filled(square(4.0 + shift, 4.0), 0.0, Color32::WHITE);
            painter.rect_filled(square(48.0 - shift, 50.0), 0.0, Color32::WHITE);
            painter.ctx().apply_filter(blur());
        }
    };
    let mut target = Target::new();
    target.draw(Color32::BLACK, Repaint::Everything, painted(0.0));
    let mut region = Region::from(square(4.0, 4.0).union(square(6.0, 4.0)));
    region.add(square(46.0, 50.0).union(square(48.0, 50.0)));
    assert_eq!(region.rects().len(), 2, "the two corners stay apart");
    target.draw(
        Color32::BLACK,
        Repaint::Region {
            region,
            background: Color32::BLACK,
        },
        painted(2.0),
    );
    let incremental = target.read();

    target.draw(Color32::BLACK, Repaint::Everything, painted(2.0));
    let whole = target.read();

    for y in 0..SIZE {
        for x in 0..SIZE {
            let (one, other) = (incremental.pixel(x, y), whole.pixel(x, y));
            assert!(
                one[0].abs_diff(other[0]) <= 2,
                "the pixel at {x}, {y} read {one:?} incrementally and {other:?} in full"
            );
        }
    }
}
