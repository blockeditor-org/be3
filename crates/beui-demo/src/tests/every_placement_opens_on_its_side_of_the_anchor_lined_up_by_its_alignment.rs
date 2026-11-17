use super::*;

#[test]
fn every_placement_opens_on_its_side_of_the_anchor_lined_up_by_its_alignment() {
    let mut test = alone(WIDE, Page::Layering);
    reveal(&mut test, "demo.placements.trigger");
    let middle = pos2(test.size().x * 0.75, test.size().y / 2.0);
    for _ in 0..REVEAL_TICKS {
        if test.rect_of("demo.placements.trigger").center().y <= test.size().y / 2.0 {
            break;
        }
        test.scroll(middle, Vec2::new(0.0, REVEAL_STEP / 4.0));
    }
    test.click("demo.placements.trigger");
    test.frame(Vec::new());
    test.settle();
    let anchor = test.rect_of("demo.placements.anchor");
    let tag = |test: &DocumentTest, index: usize| test.rect_of(&format!("demo.placement.{index}"));
    let close = |a: f32, b: f32| (a - b).abs() <= 1.0;
    for index in 0..12 {
        let shown = tag(&test, index);
        let (side, align) = (index / 3, index % 3);
        let on_side = match side {
            0 => close(shown.bottom(), anchor.top() - 4.0),
            1 => close(shown.top(), anchor.bottom() + 4.0),
            2 => close(shown.right(), anchor.left() - 4.0),
            _ => close(shown.left(), anchor.right() + 4.0),
        };
        assert!(on_side, "placement {index} opens 4 points off its side: {shown:?} by {anchor:?}");
        let lined_up = match (side < 2, align) {
            (true, 0) => close(shown.left(), anchor.left()),
            (true, 1) => close(shown.center().x, anchor.center().x),
            (true, _) => close(shown.right(), anchor.right()),
            (false, 0) => close(shown.top(), anchor.top()),
            (false, 1) => close(shown.center().y, anchor.center().y),
            (false, _) => close(shown.bottom(), anchor.bottom()),
        };
        assert!(lined_up, "placement {index} lines up by its alignment: {shown:?} by {anchor:?}");
    }
    test.snapshot("every_placement_opens_on_its_side_of_the_anchor_lined_up_by_its_alignment");
}
