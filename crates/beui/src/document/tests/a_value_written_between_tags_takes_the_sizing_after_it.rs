use super::*;
use crate::reactive::{Direction, Frame, ItemSize, List, NodeRef, build, view};

#[test]
fn a_value_written_between_tags_takes_the_sizing_after_it() {
    let (fixed, filling) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (fixed, filling) = (fixed.clone(), filling.clone());
        move || {
            let handed_over = view! {
                <Frame @node_ref=&filling />
            };
            view! {
                <List direction=Direction::Horizontal spacing=0.0>
                    <Frame @node_ref=&fixed @sizing=ItemSize::Fixed(30.0) />
                    {handed_over} @sizing=ItemSize::Percent(100.0)
                </List>
            }
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(fixed.get()).width(), 30.0);
    assert_eq!(
        harness.rect(filling.get()).width(),
        WIDE_VIEWPORT.x - 30.0,
        "a node handed to a list as a value must take the sizing written after it"
    );
}
