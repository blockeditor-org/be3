use super::*;
use crate::base::list::Align;
use crate::reactive::view;
use crate::styled::{Separator, Theme};

const LENGTH: f32 = 20.0;

#[test]
fn a_separator_keeps_the_length_it_is_given_where_its_row_centres_it() {
    let document = build(|| {
        view! {
            <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                <Separator direction=Direction::Vertical length=LENGTH />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        }
    });
    let mut harness = Harness::sized(document, VIEWPORT);

    let output = harness.frame(Vec::new());

    assert_eq!(
        rule(&output).map(|rule| rule.size()),
        Some(vec2(1.0, LENGTH)),
        "a separator that names its length keeps it where its row will not stretch it"
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
