use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

/// A 200-point scroll over 2000 points with stops at 0, 200 and 400: up to the last stop the
/// wheel and the keys step between stops both ways, and past it they move freely, coming back
/// to the last stop before stepping on.
#[test]
fn a_pager_wheels_and_keys_back_between_its_stops() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem length=200.0>
                    <Scroll
                        @node_ref=&scroll
                        style={plain_scroll()}
                        snaps={vec![0.0, 200.0, 400.0]}
                    >
                        <Frame height=2000.0 />
                    </Scroll>
                </ListItem>
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(200.0, 200.0));
    harness.settle();
    let offset = |harness: &Harness| harness.document().scroll_offset(scroll.get());
    let wheel = |harness: &mut Harness, by: f32| {
        harness.scroll_at(pos2(100.0, 100.0), vec2(0.0, by), Modifiers::NONE);
        harness.settle();
    };
    let key = |harness: &mut Harness, key: Key| {
        harness.key(key, Modifiers::NONE);
        harness.settle();
    };

    wheel(&mut harness, -40.0);
    assert_eq!(offset(&harness), 200.0, "a wheel forwards steps to the next stop");
    wheel(&mut harness, 40.0);
    assert_eq!(offset(&harness), 0.0, "and back, to the one before");
    wheel(&mut harness, -40.0);
    wheel(&mut harness, -40.0);
    assert_eq!(offset(&harness), 400.0);
    wheel(&mut harness, -100.0);
    assert_eq!(offset(&harness), 500.0, "past the last stop it scrolls freely");
    wheel(&mut harness, 300.0);
    assert_eq!(offset(&harness), 400.0, "coming back no further than the last stop");
    wheel(&mut harness, 40.0);
    assert_eq!(offset(&harness), 200.0, "and from there steps back");

    harness.click(pos2(100.0, 100.0));
    key(&mut harness, Key::ArrowUp);
    assert_eq!(offset(&harness), 0.0, "a key steps back too");
    key(&mut harness, Key::ArrowDown);
    key(&mut harness, Key::ArrowDown);
    assert_eq!(offset(&harness), 400.0);
    key(&mut harness, Key::End);
    assert_eq!(offset(&harness), 1800.0, "past the last stop keys move freely");
    key(&mut harness, Key::Home);
    assert_eq!(offset(&harness), 400.0, "coming back to the last stop");
    key(&mut harness, Key::PageUp);
    assert_eq!(offset(&harness), 200.0, "and stepping back from it");
}
