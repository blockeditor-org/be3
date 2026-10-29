use block_editor_beui::be_block::ObjectId;
use block_editor_beui::beui::Key;
use block_editor_beui::{Editor, EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use presentation_types::PresentationContent;
use uuid::Uuid;

use crate::app::PresentationApp;

mod a_card_shows_only_the_stage;
mod a_slide_template_makes_a_canvas_with_its_placeholders;
mod a_slide_with_no_picture_of_its_own_shows_its_name_in_the_filmstrip;
mod detaching_a_slide_takes_it_off_the_deck;
mod dragging_a_slide_onto_another_reorders_the_deck;
mod editing_gives_the_slide_the_whole_stage;
mod on_a_phone_the_filmstrip_is_a_page_of_its_own;
mod presenting_hides_the_filmstrip_and_the_toolbar;
mod the_filmstrip_keeps_its_focus_outline_off_the_tiles;
mod the_filmstrip_places_a_child_editor_for_every_slide;
mod the_filmstrip_pops_out_into_a_window_once_its_area_is_unlocked;
mod the_stage_shows_the_slide_the_filmstrip_selected;

type Harness = BeuiTest<PresentationApp>;

fn editor(count: usize) -> (Harness, Editor) {
    let block = Uuid::new_v4();
    let mut content = PresentationContent::default();
    for index in 0..count {
        let edit = content
            .root()
            .insert(ObjectId::new(), index, Uuid::new_v4());
        content.apply(&edit);
    }
    let host = EditorHost::default();
    host.set_editable(true);
    let owner = OwnedEditor::new(host.clone(), block);
    let editor = *owner;
    let mut test = BeuiTest::new(owner);
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
