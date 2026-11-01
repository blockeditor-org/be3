use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui::Direction;

#[test]
fn a_scroll_inside_a_scroll_lays_out_the_rows_it_holds() {
    let _root = crate::reactive::RootScope::new();
    let row = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Percent(100.0)>
                    <Offset direction=Direction::Horizontal>
                        <List spacing=0.0>
                            <Frame @node_ref=&row width=1200.0 height=40.0 />
                            <Frame width=1200.0 height=40.0 />
                        </List>
                    </Offset>
                </Offset>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let laid_out = harness.rect(row.get());
    assert_eq!(laid_out.height(), 40.0);
    assert_eq!(laid_out.min, pos2(0.0, 0.0));
}
