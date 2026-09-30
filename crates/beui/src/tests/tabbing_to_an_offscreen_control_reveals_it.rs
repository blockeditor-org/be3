use super::*;
use crate::reactive::{ForEach, NodeRef, build, view};
use crate::unstyled::Scroll;

#[test]
fn tabbing_to_an_offscreen_control_reveals_it() {
    let scroll = NodeRef::new();
    let buttons: Vec<NodeRef> = (0..10).map(|_| NodeRef::new()).collect();
    let document = build({
        let (scroll, buttons) = (scroll.clone(), buttons.clone());
        move || {
            view! {
                <Scroll @node_ref=&scroll>
                    <ForEach keys={indices(buttons.len())}>
                        {move |index: usize| view! {
                            <LabelledButton
                                @node_ref={&buttons[index]}
                                label={format!("Button {index}")}
                            />
                        }}
                    </ForEach>
                </Scroll>
            }
        }
    });
    let scroll = scroll.get();
    let buttons: Vec<NodeId> = buttons.iter().map(NodeRef::get).collect();
    let mut harness = Harness::sized(document, Vec2::new(300.0, 100.0));
    for _ in 0..6 {
        harness.key(Key::Tab, Modifiers::NONE);
    }
    assert!(unstyled::button_focused(harness.document(), buttons[4]).get());
    assert!(harness.document.scroll_offset(scroll) > 0.0);
    let rect = harness.rect(buttons[4]);
    assert!(rect.top() >= 0.0 && rect.bottom() <= 100.0);
    for _ in 0..4 {
        harness.key(Key::Tab, Modifiers::SHIFT);
    }
    assert!(unstyled::button_focused(harness.document(), buttons[0]).get());
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
}
