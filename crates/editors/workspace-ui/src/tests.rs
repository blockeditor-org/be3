use block_editor_beui::be_block::{EditorView, EditorViewContent, ViewState, WORKSPACE_EDITOR};

use block_editor_beui::beui::{Document, NodeId, Rect, Vec2};
use block_editor_beui::{BlockParent, ChildContent, Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::WorkspaceUiApp;

mod a_block_opened_while_another_is_shown_gets_its_own_tab;
mod a_block_tab_asks_its_editor_for_the_top_bar;
mod a_closed_tab_gives_up_its_view_block;
mod a_host_panel_the_host_asks_for_is_placed_in_its_own_window;
mod a_phone_shows_one_file_at_a_time_and_the_dock_bar_goes_back_or_switches;
mod a_profile_keeps_its_tabs_as_view_blocks_and_reopens_them;
mod a_reopened_phone_goes_back_through_the_files_in_the_order_they_were_shown;
mod a_shown_block_is_remembered_in_the_recents;
mod a_shown_block_is_reported_as_focused;
mod an_open_menu_is_withheld_from_the_block_under_it;
mod closing_the_only_tab_leaves_the_blank_workspace;
mod crossing_the_phone_width_keeps_the_block_on_show;
mod switching_from_a_block_to_a_host_panel_keeps_the_panel_on_show;
mod the_back_gesture_on_a_phone_leaves_a_file_for_the_files;
mod widening_the_phone_with_its_switcher_open_keeps_the_workspace;

const MAX_TAB: u64 = 64;
const SHOWN_TYPE: Uuid = Uuid::from_u128(0x7368_6f77_6e2d_7479_7065_2d74_6573_7431);

struct Fixture {
    test: BeuiTest<WorkspaceUiApp>,
    host: EditorHost,
}

impl Fixture {
    fn settle(&mut self) {
        self.test.run();
    }

    fn shown(&self) -> Vec<Uuid> {
        self.test
            .children()
            .iter()
            .filter_map(|placement| placement.content.block_id())
            .map(Uuid::from_bytes)
            .collect()
    }

    fn focused(&self) -> Option<Uuid> {
        self.host.focused_block().block_id
    }

    fn says(&self, words: &str) -> bool {
        let document = self.test.document();
        document
            .root()
            .is_some_and(|root| text_within(document, root, words).is_some())
    }

    fn open_tabs(&self) -> usize {
        self.tab_closes().len()
    }

    fn close_active_tab(&mut self) {
        let cross = self
            .tab_closes()
            .first()
            .copied()
            .expect("an open tab can be closed");
        self.test.click_at(cross.center());
        self.settle();
    }

    fn tab_closes(&self) -> Vec<Rect> {
        let document = self.test.document();
        (0..MAX_TAB)
            .filter_map(|tab| document.find_test_id(&format!("dock.tab.{tab}.close")))
            .filter_map(|close| document.node_rect(close))
            .collect()
    }
}

fn text_within(document: &Document, id: NodeId, words: &str) -> Option<NodeId> {
    if document
        .arena
        .kind_of(id)
        .is_some_and(|node| document.text(node).contains(words))
    {
        return Some(id);
    }
    document
        .children(id)
        .into_iter()
        .find_map(|child| text_within(document, child, words))
}

fn editor() -> (Fixture, Uuid) {
    editor_sized(None)
}

fn editor_sized(size: Option<Vec2>) -> (Fixture, Uuid) {
    open_editor(size, None)
}

fn profiled(layout: Option<ViewState>) -> (Fixture, Uuid) {
    profiled_sized(None, layout)
}

fn profiled_sized(size: Option<Vec2>, layout: Option<ViewState>) -> (Fixture, Uuid) {
    let mut profile = EditorView::document(WORKSPACE_EDITOR, None);
    if let Some(layout) = layout {
        let edit = profile
            .root()
            .set_state("layout", Some(&layout), 1, Uuid::nil());
        profile.apply(&edit);
    }
    open_editor(size, Some(profile))
}

fn open_editor(size: Option<Vec2>, profile: Option<EditorViewContent>) -> (Fixture, Uuid) {
    let workspace = Uuid::new_v4();
    let opened = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(Uuid::new_v4());
    if profile.is_some() {
        host.set_view_block(Some(workspace));
    }
    let editor = Editor::new(host.clone(), workspace);
    let mut fixture = Fixture {
        test: match size {
            Some(size) => BeuiTest::new(editor).with_size(size),
            None => BeuiTest::new(editor),
        },
        host,
    };
    if let Some(profile) = profile {
        fixture.test.hold(None, profile);
    }
    fixture.settle();
    (fixture, opened)
}

fn profile(fixture: &Fixture) -> EditorView {
    fixture.test.content::<EditorViewContent>(None).root()
}

fn show(fixture: &mut Fixture, id: Uuid, via: Option<Uuid>) {
    fixture.host.show_block(id, SHOWN_TYPE, via);
    fixture.settle();
}
