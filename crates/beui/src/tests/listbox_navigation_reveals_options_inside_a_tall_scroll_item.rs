use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::Scroll;

#[test]
fn listbox_navigation_reveals_options_inside_a_tall_scroll_item() {
    let _root = crate::reactive::RootScope::new();
    let (scroll, listbox) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <Scroll @node_ref=&scroll>
                <Listbox
                    @node_ref=&listbox
                    options={view! {
                        <unstyled::ChoiceOption value="One" label="One" />
                        <unstyled::ChoiceOption value="Two" label="Two" />
                        <unstyled::ChoiceOption value="Three" label="Three" />
                        <unstyled::ChoiceOption value="Four" label="Four" />
                        <unstyled::ChoiceOption value="Five" label="Five" />
                        <unstyled::ChoiceOption value="Six" label="Six" />
                    }}
                    selected=Some("One")
                />
            </Scroll>
        }
    });
    let (scroll, listbox) = (scroll.get(), listbox.get());
    let mut harness = Harness::sized(document, Vec2::new(300.0, 80.0));
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::End, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi<&str>>(listbox)
            .selected(),
        Some("Six")
    );
    let focused = harness.document().focused_node().unwrap();
    let rect = harness.rect(focused);
    assert!(rect.top() >= 0.0 && rect.bottom() <= 80.0);
    harness.key(Key::Home, Modifiers::NONE);
    assert_eq!(harness.document().scroll_offset(scroll), 0.0);
}
