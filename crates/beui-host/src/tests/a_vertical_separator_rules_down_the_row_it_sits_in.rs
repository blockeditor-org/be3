use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_vertical_separator_rules_down_the_row_it_sits_in() {
    let document = build_node(|| {
        view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <Separator direction=Direction::Vertical />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        }
    });
    let mut harness = Harness::sized(document, VIEWPORT);

    let output = harness.frame(Vec::new());

    assert_eq!(
        rule(&output),
        Some(Rect::from_min_size(Pos2::ZERO, vec2(1.0, VIEWPORT.y))),
        "a vertical separator must take a line's width from its row and the rest of its height"
    );
}

fn rule(output: &crate::native::FrameOutput) -> Option<Rect> {
    output.main().shapes().iter().find_map(|shape| match shape {
        crate::painter::Shape::Rect { rect, color, .. } if *color == Theme::DARK.border => {
            Some(*rect)
        }
        _ => None,
    })
}
