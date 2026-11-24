use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

/// A 200-point scroll of a 40-point sticky header over twelve 40-point rows.
#[test]
fn a_scroll_never_anchors_on_a_sticky_header() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let header = NodeRef::new();
    let last = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem length=200.0>
                    <Scroll @node_ref=&scroll style={plain_scroll()}>
                        <List spacing=0.0>
                            <ListItem sticky=true>
                                <Frame @node_ref=&header height=40.0 />
                            </ListItem>
                            <ForEach value={indices(11)}>|_: usize|
                                <Frame height=40.0 />
                            </ForEach>
                            <Frame @node_ref=&last height=40.0 />
                        </List>
                    </Scroll>
                </ListItem>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::sized(document, vec2(200.0, 200.0));
    harness.frame(Vec::new());
    {
        let document = harness.document_mut();
        let node = document
            .first_offset_within(scroll)
            .expect("a scroll lays its content out in an offset");
        document.drive_offset(node, 100.0);
    }
    harness.frame(Vec::new());
    assert_eq!(harness.rect(header.get()).top(), 0.0, "the header sticks");

    // Anchored on the header, which moves with the offset, the reveal's shift would move the
    // anchor by as much again on every layout, and the layout would never settle.
    harness.document_mut().reveal_node(last.get());
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(last.get()).bottom(),
        200.0,
        "the reveal scrolls the last row just into view, once"
    );
    assert_eq!(harness.rect(header.get()).top(), 0.0, "under the header still stuck");
}
