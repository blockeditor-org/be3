use super::*;
use crate::reactive::view;
use crate::styled::Select;

#[test]
fn tapping_inside_a_selection_in_a_select_search_box_opens_its_menu() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption label="Apple" />
            <unstyled::ChoiceOption label="Banana" />
        };
        [view! {
            <Select options selected=None />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let trigger = unstyled::select_trigger(harness.document(), select);
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    harness.type_text("apple pie");
    harness.frame(Vec::new());

    let search = unstyled::select_search(harness.document(), select);
    let text = harness.rect(unstyled::text_input_text(harness.document(), search));
    let word = pos2(text.left() + 10.0, text.center().y);
    for _ in 0..2 {
        harness.touch(TouchPhase::Start, word);
        harness.touch(TouchPhase::End, word);
    }
    let inside = pos2(text.left() + 24.0, text.center().y);
    harness.touch(TouchPhase::Start, inside);
    harness.touch(TouchPhase::End, inside);
    harness.frame(Vec::new());

    let copy = unstyled::text_input_menu_row(harness.document(), search, 0)
        .expect("tapping the selection opens the search box's menu");
    assert!(harness.document().node_rect(copy).is_some());
    assert!(unstyled::select_open(harness.document(), select));
    assert!(unstyled::text_input_focused(harness.document(), search).get_untracked());
}
