use block_editor_plugin::beui::{Rect, Vec2, pos2};
use block_editor_plugin::{Editor, EditorHost, ViewChange};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::PanZoomApp;

mod clicking_a_card_selects_it;
mod focusing_a_card_asks_the_host_to_pan_to_it;
mod the_canvas_places_the_cards_through_the_view_the_host_gave;
mod the_editor_reports_the_canvas_as_the_content_the_host_pans;
mod the_sidebar_goes_away_when_the_host_takes_the_chrome;
mod the_stage_stays_transparent_so_the_host_canvas_shows_through;
mod zooming_asks_the_host_instead_of_moving_the_view;

fn editor() -> (BeuiTest<PanZoomApp>, Editor) {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, block);
    (BeuiTest::new(editor.clone()), editor)
}

fn shown(test: &mut BeuiTest<PanZoomApp>, test_id: &str) -> String {
    let node = test
        .document()
        .find_test_id(test_id)
        .unwrap_or_else(|| panic!("no node with test id {test_id:?}"));
    test.document()
        .node_detail(node)
        .expect("the node has no text")
}
