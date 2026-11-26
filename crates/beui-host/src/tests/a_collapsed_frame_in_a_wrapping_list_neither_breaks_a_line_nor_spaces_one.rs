use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_collapsed_frame_in_a_wrapping_list_neither_breaks_a_line_nor_spaces_one() {
    let _root = RootScope::new();
    let hidden = create_signal(false);
    let document = build_node(move || {
        view! {
            <Frame width=100.0>
                <List direction=Direction::Horizontal wrap=true spacing=10.0>
                    <Frame @test_id="a" width=60.0 height=20.0 />
                    <Frame collapsed={hidden}>
                        <Frame width=30.0 height=20.0 />
                    </Frame>
                    <Frame @test_id="b" width=30.0 height=20.0 />
                </List>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    assert_eq!(
        harness.rect(harness.find("b")).top(),
        20.0 + 10.0,
        "shown, the middle child fills the first line and the last wraps"
    );

    hidden.set(true);
    harness.settle();
    let b = harness.rect(harness.find("b"));
    assert_eq!(
        (b.left(), b.top()),
        (70.0, 0.0),
        "collapsed, it takes no room or spacing, so the last fits after the first, one spacing on"
    );
}
