use std::sync::Arc;

use block_client::BlockClient;
use block_client::blocks::presentation::Presentation as PresentationBlock;
use block_editor_plugin::be_block::presentation::PresentationContent;
use block_editor_plugin::be_block::{BlockRef, ObjectId};
use block_editor_plugin::beui::Key;
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::PresentationApp;

mod detaching_a_slide_takes_it_off_the_deck;
mod dragging_a_slide_onto_another_reorders_the_deck;
mod editing_gives_the_slide_the_whole_stage;
mod presenting_hides_the_filmstrip_and_the_toolbar;
mod the_filmstrip_keeps_its_focus_outline_off_the_tiles;
mod the_filmstrip_places_a_child_editor_for_every_slide;
mod the_filmstrip_stays_while_a_slide_holds_the_frame;
mod the_stage_shows_the_slide_the_filmstrip_selected;

type Harness = ContentHarness<PresentationApp>;

fn editor(count: usize) -> (Harness, Editor) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(PresentationBlock::new());
    let mut content = PresentationContent::default();
    for index in 0..count {
        let edit = content
            .root()
            .insert(ObjectId::new(), index, BlockRef::Direct(Uuid::new_v4()));
        content.apply(&edit);
    }
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), client, block.id());
    let mut test = ContentHarness::new(BeuiTest::new(editor.clone()), host);
    test.hold(None, content);
    (test, editor)
}

fn detail(test: &Harness, test_id: &str) -> String {
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

fn slide_ids(test: &Harness) -> Vec<Uuid> {
    test.content::<PresentationContent>(None)
        .root()
        .slides
        .iter()
        .map(|slide| slide.id.as_uuid())
        .collect()
}
