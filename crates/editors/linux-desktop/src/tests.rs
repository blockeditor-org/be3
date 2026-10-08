use block_editor_beui::be_block::{
    EditorView, LINUX_DESKTOP_EDITOR, Root, Settings, SettingsContent, WORKSPACE_EDITOR,
};
use block_editor_beui::beui::{Document, NodeId, Rect};
use block_editor_beui::{BlockInfo, BlockParent, ChildContent, Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::LinuxDesktopApp;

mod a_block_shown_on_the_desktop_opens_in_its_own_window;
mod a_session_chosen_from_the_menu_opens_in_a_window_and_closing_it_keeps_the_session;
mod clicking_the_clock_opens_the_desktops_calendar_and_clicking_away_closes_it;
mod the_desktop_starts_with_nothing_open_but_its_bar;
mod the_programs_button_asks_the_host_for_its_launcher;

const MAX_TAB: u64 = 64;
const SHOWN_TYPE: Uuid = Uuid::from_u128(0x7368_6f77_6e2d_7479_7065_2d74_6573_7431);

struct Fixture {
    test: BeuiTest<LinuxDesktopApp>,
    host: EditorHost,
    client: Uuid,
    settings: Uuid,
}

impl Fixture {
    fn new() -> Self {
        let desktop = Uuid::new_v4();
        let client = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        host.set_client_id(client);
        host.set_view_block(Some(desktop));
        let editor = Editor::new(host.clone(), desktop);
        let mut test = BeuiTest::new(editor);
        test.hold(None, EditorView::document(LINUX_DESKTOP_EDITOR, None));
        Self {
            test,
            host,
            client,
            settings: Uuid::new_v4(),
        }
    }

    fn with_sessions(mut self, sessions: &[(&str, Uuid)]) -> (Self, Vec<Uuid>) {
        let store = self.test.store();
        store.add_block(BlockInfo::new(
            self.settings,
            Settings::CONTENT_TYPE,
            BlockParent::Root,
        ));
        let mut settings = SettingsContent::default();
        let mut profiles = Vec::new();
        for (name, editor) in sessions {
            let profile = Uuid::new_v4();
            store.add_block(BlockInfo {
                name: Some((*name).to_owned()),
                ..BlockInfo::new(
                    profile,
                    EditorView::CONTENT_TYPE,
                    BlockParent::Block(self.settings),
                )
            });
            store.hold(Some(profile), EditorView::document(*editor, None));
            settings.apply(&Settings::add_profile(*editor, self.client, profile));
            profiles.push(profile);
        }
        self.test.hold(Some(self.settings), settings);
        (self, profiles)
    }

    fn settle(&mut self) {
        self.test.run();
    }

    fn says(&self, words: &str) -> bool {
        let document = self.test.document();
        document
            .root()
            .is_some_and(|root| text_within(document, root, words).is_some())
    }

    fn click_text(&mut self, words: &str) {
        let document = self.test.document();
        let node = document
            .root()
            .and_then(|root| text_within(document, root, words))
            .unwrap_or_else(|| panic!("{words:?} is on screen"));
        let center = document
            .node_rect(node)
            .expect("the text is laid out")
            .center();
        self.test.click_at(center);
        self.settle();
    }

    fn placed_blocks(&self) -> Vec<(Uuid, Option<Uuid>, Rect)> {
        self.test
            .children()
            .iter()
            .filter_map(|placement| match placement.content {
                ChildContent::Block {
                    block_id,
                    view_block,
                    ..
                } => Some((
                    Uuid::from_bytes(block_id),
                    view_block.map(Uuid::from_bytes),
                    Rect::from_min_size(
                        block_editor_beui::pos2(placement.rect.x, placement.rect.y),
                        block_editor_beui::vec2(placement.rect.width, placement.rect.height),
                    ),
                )),
                _ => None,
            })
            .collect()
    }

    fn calendar(&self) -> Option<Uuid> {
        match self.placed_blocks()[..] {
            [] => None,
            [(block, _, _)] => Some(block),
            ref placed => panic!("only the calendar is placed: {placed:?}"),
        }
    }

    fn close_a_tab(&mut self) {
        let document = self.test.document();
        let cross = (0..MAX_TAB)
            .filter_map(|tab| document.find_test_id(&format!("dock.tab.{tab}.close")))
            .find_map(|close| document.node_rect(close))
            .expect("an open tab can be closed");
        self.test.click_at(cross.center());
        self.settle();
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

fn text_under(document: &Document, id: NodeId) -> Option<String> {
    if let Some(node) = document.arena.kind_of(id) {
        return Some(document.text(node).to_owned());
    }
    document
        .children(id)
        .into_iter()
        .find_map(|child| text_under(document, child))
}
