use super::*;
use crate::reactive::{List, NodeRef, Text, build, view};
use crate::styled::SelectableText;

#[test]
fn the_more_button_of_a_selection_toolbar_opens_the_full_menu() {
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
    let on_hello = pos2(rect.left() + 4.0, rect.center().y);
    harness.touch(TouchPhase::Start, on_hello);
    harness.touch(TouchPhase::End, on_hello);
    harness.touch(TouchPhase::Start, on_hello);
    harness.touch(TouchPhase::End, on_hello);
    assert_eq!(
        unstyled::selectable_text(harness.document(), region),
        "hello"
    );

    let toolbar = unstyled::context_menu_toolbar(harness.document(), region);
    assert!(
        harness.document().is_overlay_open(toolbar),
        "a double tap shows the toolbar"
    );
    let more = harness.center(unstyled::context_menu_more(harness.document(), region));
    harness.advance(Duration::from_secs(1));
    harness.touch(TouchPhase::Start, more);
    harness.touch(TouchPhase::End, more);

    let overlay = unstyled::context_menu_overlay(harness.document(), region);
    assert!(
        harness.document().is_overlay_open(overlay),
        "More opens the full menu"
    );
    assert!(
        !harness.document().is_overlay_open(toolbar),
        "in place of the toolbar"
    );
    let menu = unstyled::context_menu_menu(harness.document(), region);
    let select_all = harness.center(unstyled::menu_list_row_button(harness.document(), menu, 1));
    harness.touch(TouchPhase::Start, select_all);
    harness.touch(TouchPhase::End, select_all);
    assert_eq!(
        unstyled::selectable_text(harness.document(), region),
        "hello world"
    );
}
