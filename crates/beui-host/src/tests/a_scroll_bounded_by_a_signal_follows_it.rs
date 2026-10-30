use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::{Scroll, Size};

#[test]
fn a_scroll_bounded_by_a_signal_follows_it() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let max_length = create_signal(Some(Size::Custom(100.0)));
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&scroll max_length>
                    <Frame height=400.0 />
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(300.0, 600.0));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(scroll.get()).height(), 100.0);

    max_length.set(Some(Size::Custom(200.0)));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(scroll.get()).height(), 200.0);

    max_length.set(None);
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(scroll.get()).height(),
        400.0,
        "unbounded, it is as long as its content"
    );
}
