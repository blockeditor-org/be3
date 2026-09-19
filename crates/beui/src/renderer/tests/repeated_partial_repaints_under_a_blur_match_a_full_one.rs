use super::*;

#[test]
fn repeated_partial_repaints_under_a_blur_match_a_full_one() {
    let blur = || Filter {
        region: everything(),
        blur: 6.0,
        ..Filter::default()
    };
    let bar = |left: f32| Rect::from_min_max(pos2(left, 8.0), pos2(left + 10.0, 56.0));
    let mut target = Target::new();

    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.rect_filled(bar(10.0), 0.0, Color32::WHITE);
        painter.ctx().apply_filter(blur());
    });
    for step in 1u8..4 {
        let moved = 10.0 + f32::from(step) * 8.0;
        target.draw(
            Color32::BLACK,
            Repaint::Region {
                region: bar(moved - 8.0).union(bar(moved)),
                background: Color32::BLACK,
            },
            move |painter| {
                painter.rect_filled(bar(moved), 0.0, Color32::WHITE);
                painter.ctx().apply_filter(blur());
            },
        );
    }
    let incremental = target.read();

    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.rect_filled(bar(34.0), 0.0, Color32::WHITE);
        painter.ctx().apply_filter(blur());
    });
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
