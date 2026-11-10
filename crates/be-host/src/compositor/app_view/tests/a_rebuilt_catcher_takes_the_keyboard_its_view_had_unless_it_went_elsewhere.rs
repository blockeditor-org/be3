use super::*;
use be_sdk_api::{EditorInstanceId, ViewId};

fn view() -> super::super::held_keys::View {
    ("be3.text".to_owned(), EditorInstanceId(3), ViewId::MAIN)
}

#[test]
fn a_rebuilt_catcher_takes_the_keyboard_its_view_had_unless_it_went_elsewhere() {
    assert!(
        !refocus_rebuilt(&view(), true),
        "a view that did not have the keyboard when it was rebuilt does not take it"
    );
    keep_focus_for_rebuild(&view());
    assert!(
        refocus_rebuilt(&view(), true),
        "the catcher built in place of one that had the keyboard takes it back"
    );
    assert!(
        !refocus_rebuilt(&view(), true),
        "it takes it back once"
    );
    keep_focus_for_rebuild(&view());
    assert!(
        !refocus_rebuilt(&view(), false),
        "the person moved the keyboard elsewhere meanwhile, so it stays there"
    );
    assert!(
        !refocus_rebuilt(&view(), true),
        "and the view does not take it later"
    );
}
