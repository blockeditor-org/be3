use block_editor_beui::be_block::{Checklist as ChecklistModel, ChecklistContent, Edit, ObjectId};
use block_editor_beui::beui::Document;
use block_editor_beui::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::ChecklistApp;

mod adding_an_item_puts_it_on_the_list;
mod an_edit_from_elsewhere_leaves_the_other_rows_alone;
mod filtering_to_open_hides_the_items_that_are_done;

struct Harness {
    editor: BeuiTest<ChecklistApp>,
}

impl Harness {
    fn new(items: &[(&str, bool)]) -> Self {
        let block = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        let editor = Editor::new(host, block);
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
        };
        harness.editor.hold(None, content);
        harness.run();
        harness
    }

    fn run(&mut self) {
        self.editor.run();
    }

    fn arrive(&mut self, operation: Edit) {
        self.editor.edit::<ChecklistContent>(None, &operation);
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

    fn content(&self) -> ChecklistContent {
        self.editor.content(None)
    }

    fn items(&self) -> Vec<(String, bool)> {
        self.content()
            .root()
            .items
            .iter()
            .map(|item| (item.text.clone(), item.done))
            .collect()
    }

    fn id(&self, index: usize) -> ObjectId {
        self.content().root().items[index].id
    }
}
