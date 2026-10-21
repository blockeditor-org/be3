use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::Scroll;

#[test]
fn unused_navigation_keys_scroll_the_nearest_ancestor() {
    let _root = crate::reactive::RootScope::new();
    let (scroll, tabs, slider) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build(move || {
        view! {
            <Scroll @node_ref=&scroll>
                <Tabs
                    @node_ref=&tabs
                    options={view! {
                        <unstyled::ChoiceOption label="One" />
                        <unstyled::ChoiceOption label="Two" />
                    }}
                    selected=0
                />
                <Slider @node_ref=&slider value=0.5 />
                <ForEach keys={indices(20)}>|_: usize|
                    <Text color=Color32::WHITE>"Content"</Text>
                </ForEach>
            </Scroll>
        }
    });
    let (scroll, tabs, slider) = (scroll.get(), tabs.get(), slider.get());
    let mut harness = Harness::sized(document, Vec2::new(300.0, 100.0));
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(styled::tabs_selected(harness.document(), tabs), 0);
    assert_eq!(harness.document.scroll_offset(scroll), 40.0);
    harness.key(Key::PageUp, Modifiers::NONE);
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert!(styled::slider_value(harness.document(), slider) < 0.5);
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
}
