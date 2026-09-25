use block_editor_plugin::be_block::{BrowserTabContent, HistoryItem};
use block_editor_plugin::beui::{Key, Modifiers};
use block_editor_plugin::{Editor, EditorHost, WebViewCommand, WebViewEvent};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::BrowserTabApp;

mod a_pushed_url_becomes_the_tab_s_history;
mod typing_an_address_navigates_the_web_view;

const ACCOUNT: Uuid = Uuid::from_u128(0x7765_622d_7465_7374_2d61_6363_6f75_6e74);

struct Harness {
    editor: BeuiTest<BrowserTabApp>,
}

impl Harness {
    fn new() -> Self {
        let block = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        host.set_client_id(ACCOUNT);
        let editor = Editor::new(host, block);
        let mut harness = Self {
            editor: BeuiTest::new(editor),
        };
        let mut content = BrowserTabContent::default();
        let first = content
            .root()
            .push(&HistoryItem::new("https://example.com/", ""));
        content.apply(&first);
        harness.editor.hold(None, content);
        harness.run();
        harness
    }

    fn run(&mut self) {
        self.editor.run();
    }

    fn urls(&self) -> Vec<String> {
        self.editor
            .content::<BrowserTabContent>(None)
            .root()
            .history
            .iter()
            .map(|item| item.url.clone())
            .collect()
    }
}
