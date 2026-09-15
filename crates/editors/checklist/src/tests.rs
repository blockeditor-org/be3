use std::sync::Arc;

use block_client::blocks::checklist::{Checklist, ChecklistOperation};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::ChecklistApp;

mod adding_an_item_puts_it_on_the_list;
mod editing_one_item_leaves_the_other_rows_alone;
mod filtering_to_open_hides_the_items_that_are_done;

fn editor(items: &[(&str, bool)]) -> (BeuiTest<ChecklistApp>, BlockHandle<Checklist>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Checklist::default());
    for (text, done) in items {
        let add = ChecklistOperation::add(*text);
        let ChecklistOperation::Add { id, .. } = add else {
            unreachable!("add builds an add operation")
        };
        block.operate(add);
        if *done {
            block.operate(ChecklistOperation::SetDone { id, done: true });
        }
    }

    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    (BeuiTest::new(editor), block)
}

fn items(block: &BlockHandle<Checklist>) -> Vec<(String, bool)> {
    block
        .read()
        .unwrap()
        .items()
        .iter()
        .map(|item| (item.text.clone(), item.done))
        .collect()
}

fn id(block: &BlockHandle<Checklist>, index: usize) -> Uuid {
    block.read().unwrap().items()[index].id
}
