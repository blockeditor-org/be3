use super::*;
use crate::reactive::view;
use crate::styled::Link;

#[test]
fn hovering_a_link_underlines_it_without_moving_anything() {
    let (document, [link]) = toolbar_of(|| {
        [view! {
            <Link label="Open the grid" />
        }]
    });
    let mut harness = Harness::new(document);
    let idle = harness.frame(Vec::new());
    let before = harness.document().node_rect(link).expect("the link");

    let pointer = before.center();
    let hovered = harness.frame(vec![Event::PointerMoved(pointer)]);
    let after = harness.document().node_rect(link).expect("the link");

    assert_eq!(before, after);
    assert_eq!(underlines(&idle), 0);
    assert_eq!(underlines(&hovered), 1);
}

fn underlines(output: &crate::FrameOutput) -> usize {
    output
        .shapes()
        .iter()
        .filter(|shape| {
            matches!(shape, crate::Shape::Rect { stroke_width, color, .. }
                if *color == styled::Theme::DARK.accent_hover && *stroke_width == 0.0)
        })
        .count()
}
