use std::cell::Cell;

use beui::styled::Root;

use super::*;

thread_local! {
    static PICKED: Cell<bool> = const { Cell::new(false) };
}

struct FailingPick;

#[component]
impl BeuiRoot for FailingPick {
    #[component]
    fn View(editor: Editor) -> Root {
        editor.pick_block(
            be_sdk_beui::BlockFilter {
                name: "Block".to_owned(),
                block_types: Vec::new(),
                excluded: Vec::new(),
                templates: false,
                place: None,
            },
            None::<beui::Rect>,
            |_| PICKED.set(true),
        );
        view! {
            <Root>
                <Text>"Picking"</Text>
            </Root>
        }
    }
}

#[test]
fn a_failed_pick_is_reported_and_never_answers_the_editor() {
    let mut test = BeuiTest::<FailingPick>::editing(Uuid::new_v4());
    test.settle();
    test.answer_picker(Err(be_sdk_api::Failure::Failed(
        "The server can't be reached.".to_owned(),
    )));
    test.settle();

    assert_eq!(
        test.take_reported_failures(),
        ["The block could not be picked. The server can't be reached."],
        "the SDK reports the failed pick for the editor"
    );
    assert!(!PICKED.get(), "the editor's callback hears nothing of it");
}

test_editor!(FailingPick);
