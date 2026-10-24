use block_editor_beui::be_block::{Counter, CounterContent};
use block_editor_beui::beui::{Document, Key, Modifiers, NodeId};
use block_editor_beui::{EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::BlockDebugApp;

mod a_value_changed_in_place_reaches_the_block;
mod an_edit_to_the_block_shows_in_its_debug_view;
mod the_debug_view_shows_what_the_block_holds;
mod values_can_only_be_changed_after_the_warning_is_dismissed;

fn editor() -> BeuiTest<BlockDebugApp> {
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::new(OwnedEditor::new(host, Uuid::from_u128(1)));
    test.hold(None, CounterContent::default());
    test.edit::<CounterContent>(None, &Counter::add(3));
    test.run();
    test
}

const COUNT: &str = "block-debug.content.2.1";

fn rename_field() -> String {
    format!("{COUNT}.rename")
}

fn says(test: &BeuiTest<BlockDebugApp>, words: &str) -> bool {
    let document = test.document();
    document
        .root()
        .is_some_and(|root| says_within(document, root, words))
}

fn says_within(document: &Document, node: NodeId, words: &str) -> bool {
    document
        .arena
        .kind_of(node)
        .is_some_and(|text| document.text(text).contains(words))
        || document
            .children(node)
            .into_iter()
            .any(|child| says_within(document, child, words))
}
