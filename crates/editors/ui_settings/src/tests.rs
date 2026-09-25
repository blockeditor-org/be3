use block_editor_plugin::be_block::UiSettingsContent;
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::UiSettingsApp;

mod pressing_an_arrow_key_stores_a_new_zoom;

struct Harness {
    editor: BeuiTest<UiSettingsApp>,
}

impl Harness {
    fn new() -> Self {
        let block = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        let editor = Editor::new(host, block);
        let mut harness = Self {
            editor: BeuiTest::new(editor),
        };
        harness.editor.hold(None, UiSettingsContent::default());
        harness.run();
        harness
    }

    fn run(&mut self) {
        self.editor.run();
    }

    fn content(&self) -> UiSettingsContent {
        self.editor.content(None)
    }
}
