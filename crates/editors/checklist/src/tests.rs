use block_editor_plugin::be_block::{
    BlockContent, Checklist as ChecklistModel, ChecklistContent, Edit, LiveEdit, ObjectId,
};
use block_editor_plugin::beui::Document;
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::ChecklistApp;

mod adding_an_item_puts_it_on_the_list;
mod an_edit_from_elsewhere_leaves_the_other_rows_alone;
mod filtering_to_open_hides_the_items_that_are_done;

struct Harness {
    editor: BeuiTest<ChecklistApp>,
    host: EditorHost,
    content: ChecklistContent,
    applied: u64,
}

impl Harness {
    fn new(items: &[(&str, bool)]) -> Self {
        let block = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        let editor = Editor::new(host.clone(), block);
        let mut content = ChecklistContent::default();
        for (text, done) in items {
            let (id, add) = ChecklistModel::add(*text);
            content.apply(&add);
            if *done {
                content.apply(&ChecklistModel::set_done(id, true));
            }
        }
        let mut harness = Self {
            editor: BeuiTest::new(editor),
            host,
            content,
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
            let operation = ChecklistContent::decode_operation(&operation)
                .expect("the editor sent an operation the checklist cannot read");
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
            ChecklistContent::CONTENT_TYPE,
            self.content.encode(),
            self.applied,
        );
    }

    fn arrive(&mut self, operation: Edit) {
        self.content.apply(&operation);
        self.publish();
        self.run();
    }

    fn click(&mut self, test_id: &str) {
        self.editor.click(test_id);
        self.run();
    }

    fn type_text(&mut self, text: &str) {
        self.editor.text(text);
        self.run();
    }

    fn document(&self) -> &Document {
        self.editor.document()
    }

    fn snapshot(&mut self, name: &str) {
        self.editor.snapshot(name);
    }

    fn items(&self) -> Vec<(String, bool)> {
        self.content
            .root()
            .items
            .iter()
            .map(|item| (item.text.clone(), item.done))
            .collect()
    }

    fn id(&self, index: usize) -> ObjectId {
        self.content.root().items[index].id
    }
}
