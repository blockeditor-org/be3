use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn a_scroll_with_a_button_atop_its_text_is_scrolled_from_the_button() {
    let _root = crate::reactive::RootScope::new();
    let (scroll, button) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <Scroll @node_ref=&scroll>
                <LabelledButton @node_ref=&button label="Top" on_click={move || {}} />
                <ForEach keys={indices(20)}>|_: usize|
                    <Text color=Color32::WHITE>"Content"</Text>
                </ForEach>
            </Scroll>
        }
    });
    let (scroll, button) = (scroll.get(), button.get());
    let mut harness = Harness::sized(document, Vec2::new(300.0, 100.0));
    harness.frame(Vec::new());
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(button)
            .focused()
            .get()
    );
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(harness.document().scroll_offset(scroll), 40.0);
    harness.key(Key::PageDown, Modifiers::NONE);
    assert_eq!(harness.document().scroll_offset(scroll), 140.0);
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(button)
            .focused()
            .get()
    );
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(button)
            .focused()
            .get()
    );
}
