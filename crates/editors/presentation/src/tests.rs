use std::sync::Arc;

use block_client::block_ref::BlockRef;
use block_client::blocks::presentation::{Presentation, PresentationOperation, PresentationSlide};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::beui::Key;
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::PresentationApp;

mod detaching_a_slide_takes_it_off_the_deck;
mod dragging_a_slide_onto_another_reorders_the_deck;
mod presenting_hides_the_filmstrip_and_the_toolbar;
mod the_filmstrip_places_a_child_editor_for_every_slide;
mod the_stage_shows_the_slide_the_filmstrip_selected;

fn editor(count: usize) -> (BeuiTest<PresentationApp>, Editor, BlockHandle<Presentation>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Presentation::new());
    for index in 0..count {
        block.operate(PresentationOperation::Insert {
            slide: PresentationSlide {
                id: Uuid::new_v4(),
                block_id: BlockRef::Direct(Uuid::new_v4()),
            },
            index,
        });
    }
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let test = BeuiTest::new(editor.clone());
    (test, editor, block)
}

fn detail(test: &BeuiTest<PresentationApp>, test_id: &str) -> String {
    let node = test
        .document()
        .find_test_id(test_id)
        .unwrap_or_else(|| panic!("no node with test id {test_id:?}"));
    let detail = test
        .document()
        .node_detail(node)
        .expect("the node has no text");
    detail.trim_matches('"').to_owned()
}

fn slide_ids(block: &BlockHandle<Presentation>) -> Vec<Uuid> {
    block
        .read()
        .unwrap()
        .slides()
        .iter()
        .map(|slide| slide.id)
        .collect()
}
