use super::*;
use crate::base::overlay::OverlayNode;
use crate::reactive::{List, NodeRef, Text, build, view};
use crate::styled::SelectableText;

#[test]
fn a_long_press_on_selectable_text_selects_the_word_and_its_toolbar_copies_it() {
    let (text, region) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (text, region) = (text.clone(), region.clone());
        move || {
            view! {
                <SelectableText @node_ref=&region>
                    <List spacing=4.0>
                        <Text @node_ref=&text string="hello world" />
                    </List>
                </SelectableText>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (text, region) = (text.get(), region.get());
    let rect = harness.rect(text);
    let on_world = pos2(rect.right() - 4.0, rect.center().y);

    harness.touch(TouchPhase::Start, on_world);
    harness.advance(Duration::from_secs(1));
    harness.frame(Vec::new());
    harness.touch(TouchPhase::End, on_world);
    assert_eq!(
        unstyled::selectable_text(harness.document(), region),
        "world",
        "a long press selects the word under the finger"
    );
    let overlay = unstyled::context_menu_overlay(harness.document(), region);
    assert!(
        !harness.document().is_overlay_open(overlay),
        "the long press does not open the full menu"
    );
    let toolbar = unstyled::context_menu_toolbar(harness.document(), region);
    assert!(
        harness.document().is_overlay_open(toolbar),
        "it shows the toolbar instead"
    );
    let bar = harness.rect(
        harness
            .document()
            .overlay_content(kind_of::<OverlayNode>(harness.document(), toolbar))
            .expect("the toolbar has content"),
    );
    assert!(
        bar.bottom() <= rect.top() || bar.top() >= rect.bottom(),
        "the toolbar floats clear of the selected text: {bar:?} and {rect:?}"
    );

    let copy = harness
        .center(text_within(harness.document(), toolbar, "Copy").expect("the toolbar offers Copy"));
    harness.touch(TouchPhase::Start, copy);
    let copied = harness.frame(vec![touch_event(1, TouchPhase::End, copy)]);
    assert_eq!(copied.copied_text.as_deref(), Some("world"));
    assert!(
        !harness.document().is_overlay_open(toolbar),
        "choosing an action puts the toolbar away"
    );
    assert_eq!(
        unstyled::selectable_text(harness.document(), region),
        "world",
        "and keeps the selection"
    );
}
