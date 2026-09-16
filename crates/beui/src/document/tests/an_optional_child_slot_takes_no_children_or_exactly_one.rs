use super::*;
use crate::reactive::{Column, Frame, Text, build, view};

#[test]
fn an_optional_child_slot_takes_no_children_or_exactly_one() {
    let document = build(|| {
        view! {
            <Column spacing=0.0>
                <Frame></Frame>
                <Frame>
                    <Text string="only" />
                </Frame>
            </Column>
        }
    });
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(harness.tree(), ["column", "  frame", "  frame", "    text"]);
}
