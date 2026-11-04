use be_sdk_beui::be_block::BlockContent;
use be_type_example_counter::CounterContent;

use be_sdk_test::BeuiTest;
use be_type_workspace::FolderContent;
use uuid::Uuid;

use crate::root::WorkspaceIndexRoot;

mod an_empty_folder_says_so;
mod dropping_a_block_reports_whether_the_folder_takes_it;
mod every_entry_in_the_index_gets_a_cell;
mod on_a_phone_the_folder_toolbar_fits_above_the_entries;
mod the_folder_toolbar_is_a_bar_above_the_entries_that_goes_with_the_chrome;

struct Fixture {
    editor: BeuiTest<WorkspaceIndexRoot>,
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
    let mut editor = BeuiTest::editing(folder);
    editor.hold(None, content);
    editor.run();
    (Fixture { editor }, children)
}
