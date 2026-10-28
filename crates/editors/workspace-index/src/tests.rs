use block_editor_beui::be_block::BlockContent;
use block_ui_test::prelude::*;
use counter_types::CounterContent;

use block_editor_beui::{EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use uuid::Uuid;
use workspace_ui_types::FolderContent;

use crate::app::WorkspaceIndexApp;

mod an_empty_folder_says_so;
mod dropping_a_block_reports_whether_the_folder_takes_it;
mod every_entry_in_the_index_gets_a_cell;

struct Fixture {
    editor: BeuiTest<WorkspaceIndexApp>,
}

fn editor(entries: usize) -> (Fixture, Vec<Uuid>) {
    let folder = Uuid::new_v4();
    let mut children = Vec::new();
    let mut content = FolderContent::default();
    for _ in 0..entries {
        let child = Uuid::new_v4();
        let edit = content.root().add(child);
        content.apply(&edit);
        children.push(child);
    }
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = OwnedEditor::new(host, folder);
    let mut editor = BeuiTest::new(editor);
    editor.hold(None, content);
    editor.run();
    (Fixture { editor }, children)
}
