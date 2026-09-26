use block_editor_plugin::be_block::{BlockContent, FileTreeContent};

use block_editor_plugin::beui::{Document, NodeId, Rect};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::WorkspaceUiApp;

mod a_block_opened_while_another_is_shown_gets_its_own_tab;
mod a_block_tab_asks_its_editor_for_the_top_bar;
mod a_narrow_workspace_folds_files_into_the_pane_beside_it;
mod a_shown_block_is_reported_as_focused;
mod an_open_menu_is_withheld_from_the_block_under_it;
mod closing_the_only_tab_leaves_the_blank_workspace;

const SETTLE_FRAMES: usize = 8;
const MAX_TAB: u64 = 64;

struct Fixture {
    test: BeuiTest<WorkspaceUiApp>,
    host: EditorHost,
}

impl Fixture {
    fn settle(&mut self) {
        for _ in 0..SETTLE_FRAMES {
            self.test.run();
        }
    }

    fn shown(&self) -> Vec<Uuid> {
        self.test
            .children()
            .iter()
            .map(|placement| Uuid::from_bytes(placement.block_id))
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
    if document.node_kind(id) == "text" && document.text(id).contains(words) {
        return Some(id);
    }
    document
        .children(id)
        .into_iter()
        .find_map(|child| text_within(document, child, words))
}

fn editor() -> (Fixture, Uuid) {
    let workspace = Uuid::new_v4();
    let opened = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(Uuid::new_v4());
    let editor = Editor::new(host.clone(), workspace);
    let mut fixture = Fixture {
        test: BeuiTest::new(editor),
        host,
    };
    fixture.settle();
    (fixture, opened)
}

fn show(fixture: &mut Fixture, id: Uuid, via: Option<Uuid>) {
    fixture
        .host
        .show_block(id, FileTreeContent::CONTENT_TYPE, via);
    fixture.settle();
}
