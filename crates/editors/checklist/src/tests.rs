use block_editor_beui::be_block::{Edit, ObjectId};
use block_editor_beui::beui::Document;
use block_editor_beui::{EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use checklist_types::{Checklist as ChecklistModel, ChecklistContent};
use uuid::Uuid;

use crate::app::ChecklistApp;

mod a_checklist_drawn_at_a_finer_raster_is_sharper_where_it_was;
mod adding_an_item_puts_it_on_the_list;
mod an_edit_from_elsewhere_leaves_the_other_rows_alone;
mod filtering_to_open_hides_the_items_that_are_done;
mod the_checklist_waits_for_its_content;

struct Harness {
    editor: BeuiTest<ChecklistApp>,
}

impl Harness {
    fn new(items: &[(&str, bool)]) -> Self {
        let block = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        let editor = OwnedEditor::new(host, block);
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
        harness
            .editor
            .store()
            .register(checklist_types::registrations());
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
        self.editor.type_text(text);
        self.run();
    }

    fn document(&self) -> std::cell::Ref<'_, Document> {
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

    fn place(&mut self, size: block_editor_beui::beui::Vec2, raster: f32) {
        self.editor.set_size(size);
        self.editor.set_raster(raster);
    }

    fn id(&self, index: usize) -> ObjectId {
        self.content().root().items[index].id
    }
}
