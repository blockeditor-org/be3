use block_editor_beui::beui::{Rect, Vec2, pos2};
use block_editor_beui::{Editor, EditorHost, OwnedEditor, ViewChange};
use block_ui_test::BeuiTest;
use block_ui_test::prelude::*;
use uuid::Uuid;

use crate::app::PanZoomApp;

mod a_pinch_in_the_frame_a_resize_lands_in_keeps_the_view_where_the_person_put_it;
mod a_view_moved_before_the_editor_opened_stays_where_it_was;
mod clicking_a_card_selects_it;
mod focusing_a_card_asks_the_host_to_pan_to_it;
mod the_canvas_places_the_cards_through_the_view_the_host_gave;
mod the_editor_opens_with_its_cards_fitted_into_the_view;
mod the_editor_reports_the_canvas_as_the_content_the_host_pans;
mod the_sidebar_goes_away_when_the_host_takes_the_chrome;
mod the_stage_stays_transparent_so_the_host_canvas_shows_through;
mod zooming_asks_the_host_instead_of_moving_the_view;
mod zooming_stops_the_view_following_its_fit_until_fit_is_pressed;

fn editor() -> (BeuiTest<PanZoomApp>, Editor) {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let owner = OwnedEditor::new(host, block);
    let editor = *owner;
    (BeuiTest::new(owner), editor)
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
