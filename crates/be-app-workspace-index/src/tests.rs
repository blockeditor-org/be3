use be_sdk_beui::be_block_content::BlockContent;
use be_type_example_counter::CounterContent;

use be_sdk_beui::beui::native::Document;
use be_sdk_beui::beui::{NodeId, Pos2};
use be_sdk_test::BeuiTest;
use be_sdk_test::prelude::*;
use be_type_workspace::FolderContent;
use uuid::Uuid;

use crate::root::WorkspaceIndexRoot;

mod an_empty_folder_says_so;
mod dropping_a_block_reports_whether_the_folder_takes_it;
mod every_entry_in_the_index_gets_a_cell;
mod an_entry_s_row_menu_renames_it_in_place;
mod on_a_phone_the_folder_toolbar_fits_above_the_entries;
mod the_folder_toolbar_is_a_bar_above_the_entries_that_goes_with_the_chrome;

struct Fixture {
    editor: BeuiTest<WorkspaceIndexRoot>,
    folder: Uuid,
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
    (Fixture { editor, folder }, children)
}

/// Clicks the item an open menu or list offers as `label`.
fn click_item(editor: &mut BeuiTest<WorkspaceIndexRoot>, label: &str) {
    let document = editor.document();
    let item = document
        .root()
        .and_then(|root| open_item(&document, root, label, false))
        .unwrap_or_else(|| panic!("an open menu offers {label}"));
    drop(document);
    editor.click(item);
    editor.run();
}

fn open_item(document: &Document, node: NodeId, label: &str, open: bool) -> Option<Pos2> {
    let open = match document.node_kind(node) {
        "overlay" => document.node_detail(node).as_deref() == Some("open"),
        _ => open,
    };
    let text = document.node_detail(node).unwrap_or_default();
    if open && document.node_kind(node) == "text" && text.trim_matches('"') == label {
        return document.node_rect(node).map(|rect| rect.center());
    }
    document
        .children(node)
        .into_iter()
        .find_map(|child| open_item(document, child, label, open))
}
