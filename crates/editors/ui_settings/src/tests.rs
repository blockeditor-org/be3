use block_editor_plugin::be_block::{BlockContent, LiveEdit, UiSettingsContent};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::UiSettingsApp;

mod pressing_an_arrow_key_stores_a_new_zoom;

struct Harness {
    editor: BeuiTest<UiSettingsApp>,
    host: EditorHost,
    content: UiSettingsContent,
    applied: u64,
}

impl Harness {
    fn new() -> Self {
        let block = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        let editor = Editor::new(host.clone(), block);
        let mut harness = Self {
            editor: BeuiTest::new(editor),
            host,
            content: UiSettingsContent::default(),
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
            let operation = UiSettingsContent::decode_operation(&operation)
                .expect("the editor sent an operation the settings cannot read");
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
            UiSettingsContent::CONTENT_TYPE,
            self.content.encode(),
            self.applied,
        );
    }
}
