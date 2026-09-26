use std::collections::HashSet;

use accesskit::Role;

use super::*;
use crate::geometry::vec2;
use crate::reactive::{ForEach, List, build, view};
use crate::unstyled::Scroll;

const ROWS: u32 = 30;

#[test]
fn scrolling_resends_the_rows_that_moved_but_not_what_moved_with_them() {
    let document = build(|| {
        view! {
            <List spacing=0.0>
                <Frame height=200.0>
                    <Scroll @test_id="scroll">
                        <ForEach keys={(0..ROWS).collect::<Vec<_>>()}>
                            {|row: u32| view! {
                                <styled::Button
                                    variant=styled::ButtonVariant::Primary
                                    label={format!("Row {row}")}
                                    on_click={|| {}}
                                />
                            }}
                        </ForEach>
                    </Scroll>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    let known: HashSet<_> = harness
        .frame(Vec::new())
        .accessibility_tree("Test", VIEWPORT)
        .nodes
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let scroll = harness.find("scroll");
    harness.frame(vec![Event::PointerMoved(harness.rect(scroll).center())]);

    let output = harness.frame(vec![Event::Scroll(vec2(0.0, -25.0))]);
    assert_eq!(harness.document().scroll_offset(scroll), 25.0);
    let sent = output.accessibility_tree("Test", VIEWPORT).nodes;

    let buttons = sent
        .iter()
        .filter(|(_, node)| node.role() == Role::Button)
        .count();
    let resent_labels = sent
        .iter()
        .filter(|(id, node)| node.role() == Role::Label && known.contains(id))
        .count();
    assert!(buttons > 0, "the rows moved, so they are sent again");
    assert_eq!(
        resent_labels, 0,
        "a label keeps its place inside its row, so it is not sent again"
    );
}
