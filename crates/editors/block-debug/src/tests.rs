use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::native::Document;
use block_ui_test::BeuiTest;
use counter_types::{Counter, CounterContent};
use uuid::Uuid;

use crate::app::BlockDebugApp;

mod an_edit_to_the_block_shows_in_its_debug_view;
mod the_debug_view_shows_what_the_block_holds;

fn editor() -> BeuiTest<BlockDebugApp> {
    let mut test = BeuiTest::editing(Uuid::from_u128(1));
    test.hold(None, CounterContent::default());
    test.edit::<CounterContent>(None, &Counter::add(3));
    test.run();
    test
}

fn says(test: &BeuiTest<BlockDebugApp>, words: &str) -> bool {
    let document = test.document();
    document
        .root()
        .is_some_and(|root| says_within(&document, root, words))
}

fn says_within(document: &Document, node: NodeId, words: &str) -> bool {
    document
        .arena
        .kind_of(node)
        .is_some_and(|text| document.text(text).contains(words))
        || document
            .children(node)
            .into_iter()
            .any(|child| says_within(&document, child, words))
}
