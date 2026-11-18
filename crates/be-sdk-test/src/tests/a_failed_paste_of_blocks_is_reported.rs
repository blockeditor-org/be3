use std::cell::Cell;

use beui::styled::Root;

use super::*;
use crate::HostCall;

thread_local! {
    static ANSWERED: Cell<Option<usize>> = const { Cell::new(None) };
}

struct FailingPaste;

#[component]
impl BeuiRoot for FailingPaste {
    #[component]
    fn View(editor: Editor) -> Root {
        let asked = editor
            .host()
            .paste_blocks(&[Uuid::from_u128(1)], BlockParent::Block(editor.block_id()));
        editor.then(asked, |pasted| ANSWERED.set(Some(pasted.len())));
        view! {
            <Root>
                <Text>"Pasting"</Text>
            </Root>
        }
    }
}

#[test]
fn a_failed_paste_of_blocks_is_reported() {
    let mut test = BeuiTest::<FailingPaste>::editing(Uuid::new_v4());
    test.settle();
    let reply = test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::PasteBlocks { reply, .. } => Some(reply),
            _ => None,
        })
        .expect("the editor asks the host to paste");
    test.answer(reply, Err(be_sdk_api::Failure::Denied));
    test.settle();

    assert_eq!(
        test.take_reported_failures(),
        ["The blocks could not be pasted. The host refused this request."],
        "the SDK reports the failed paste for the editor"
    );
    assert_eq!(
        ANSWERED.get(),
        Some(0),
        "the editor hears that nothing was pasted"
    );
}

test_editor!(FailingPaste);
