use super::*;
use crate::reactive::view;
use crate::styled::{Separator, Theme};

#[test]
fn a_bare_separator_rules_across_the_column_it_sits_in() {
    let document = build(|| {
        view! {
            <List spacing=0.0>
                <Separator />
            </List>
        }
    });
    let mut harness = Harness::sized(document, VIEWPORT);

    let output = harness.frame(Vec::new());

    assert_eq!(
        rule(&output),
        Some(Rect::from_min_size(Pos2::ZERO, vec2(VIEWPORT.x, 1.0))),
        "a separator given no sizing of its own must still rule across its column"
    );
}

fn rule(output: &crate::FrameOutput) -> Option<Rect> {
    output.shapes().iter().find_map(|shape| match shape {
        crate::painter::Shape::Rect { rect, color, .. } if *color == Theme::DARK.border => {
            Some(*rect)
        }
        _ => None,
    })
}
