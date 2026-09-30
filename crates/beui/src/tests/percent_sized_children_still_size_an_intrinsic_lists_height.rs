use super::*;
use crate::reactive::{Direction, Frame, ItemSize, List, NodeRef, build};

#[test]
fn percent_sized_children_still_size_an_intrinsic_lists_height() {
    let row = NodeRef::new();
    let document = build({
        let row = row.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <List @node_ref=&row direction=Direction::Horizontal spacing=0.0>
                        <Frame
                            @sizing=ItemSize::Percent(50.0)
                            padding_horizontal=0.0
                            padding_vertical=20.0
                        >
                            <Spacer />
                        </Frame>
                        <Frame
                            @sizing=ItemSize::Percent(50.0)
                            padding_horizontal=0.0
                            padding_vertical=20.0
                        >
                            <Spacer />
                        </Frame>
                    </List>
                </List>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(row.get()).height(), 40.0);
}
