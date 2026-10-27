use super::*;
use crate::native::keyboard::styled::SoftwareKeyboard;
use crate::native::keyboard::styled::key_center;
use crate::native::keyboard::unstyled::{Layer, Value};
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn a_wheel_over_the_software_keyboard_scrolls_what_holds_it() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let keyboard = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <List spacing=0.0>
                        <Frame height=300.0 @node_ref=&keyboard>
                            <SoftwareKeyboard />
                        </Frame>
                        <Frame height=2000.0 />
                    </List>
                </Scroll>
            </List>
        }
    });
    let (scroll, keyboard) = (scroll.get(), keyboard.get());
    let mut harness = Harness::sized(document, vec2(480.0, 600.0));
    harness.frame(Vec::new());
    let key = key_center(harness.rect(keyboard), Layer::Text, Value::Char('g')).expect("a g key");

    harness.scroll_at(key, vec2(0.0, -120.0), Modifiers::NONE);
    harness.advance(Duration::from_secs(2));
    harness.frame(Vec::new());
    assert!(
        harness.document().scroll_offset(scroll) > 0.0,
        "the wheel passes over the keys to the scroll around them"
    );
}
