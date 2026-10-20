use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn the_more_button_of_a_selection_toolbar_opens_the_full_menu() {
    let _root = crate::reactive::RootScope::new();
    let (text, region) = (NodeRef::new(), NodeRef::new());
    let document = build(move || {
        view! {
            <SelectableText @node_ref=&region>
                <List spacing=4.0>
                    <Text @node_ref=&text string="hello world" />
                </List>
            </SelectableText>
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

    assert!(
        unstyled::context_menu_toolbar(harness.document(), region).is_some(),
        "a double tap shows the toolbar"
    );
    let more = harness.center(unstyled::context_menu_more(harness.document(), region));
    harness.advance(Duration::from_secs(1));
    harness.touch(TouchPhase::Start, more);
    harness.touch(TouchPhase::End, more);

    assert!(
        unstyled::context_menu_open(harness.document(), region),
        "More opens the full menu"
    );
    assert!(
        unstyled::context_menu_toolbar(harness.document(), region).is_none(),
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
