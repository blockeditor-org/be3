use block_editor_plugin::be_block::{BlockContent, BrowserTabContent, HistoryItem, LiveEdit};
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
    host: EditorHost,
    content: BrowserTabContent,
    applied: u64,
}

impl Harness {
    fn new() -> Self {
        let block = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        host.set_client_id(ACCOUNT);
        let editor = Editor::new(host.clone(), block);
        let mut harness = Self {
            editor: BeuiTest::new(editor),
            host,
            content: {
                let mut content = BrowserTabContent::default();
                let first = content
                    .root()
                    .push(&HistoryItem::new("https://example.com/", ""));
                content.apply(&first);
                content
            },
            applied: 0,
        };
        harness.publish();
        harness.run();
        harness
    }

    fn run(&mut self) {
        self.editor.run();
        let mut changed = false;
        for operation in self.host.take_content_operations() {
            let operation = BrowserTabContent::decode_operation(&operation)
                .expect("the editor sent an operation the browser tab cannot read");
            self.content.apply(&operation);
            self.applied += 1;
            changed = true;
        }
        if changed {
            self.publish();
        }
        self.editor.run();
    }

    fn publish(&mut self) {
        self.host.set_block_content(
            BrowserTabContent::CONTENT_TYPE,
            self.content.encode(),
            self.applied,
        );
    }

    fn urls(&self) -> Vec<String> {
        self.content
            .root()
            .history
            .iter()
            .map(|item| item.url.clone())
            .collect()
    }
}
