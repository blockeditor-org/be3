use super::*;
use crate::base::Direction;
use crate::reactive::{ItemSize, List, NodeRef, Scroll, build, view};

#[test]
fn a_scroll_inside_a_scroll_lays_out_the_rows_it_holds() {
    let row = NodeRef::new();
    let held = row.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <Scroll direction=Direction::Horizontal>
                        <List spacing=0.0>
                            <Frame @node_ref=&held width=1200.0 height=40.0 />
                            <Frame width=1200.0 height=40.0 />
                        </List>
                    </Scroll>
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let laid_out = harness.rect(row.get());
    assert_eq!(laid_out.height(), 40.0);
    assert_eq!(laid_out.min, pos2(0.0, 0.0));
}
