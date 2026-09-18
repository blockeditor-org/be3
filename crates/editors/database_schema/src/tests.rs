use std::sync::Arc;

use block_client::blocks::database_schema::DatabaseSchema;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::DatabaseSchemaApp;

mod adding_a_field_appends_a_string_field;
mod configured_fields_paint_on_separate_lines;
mod renaming_a_field_writes_the_new_name_to_the_block;

fn editor() -> (BeuiTest<DatabaseSchemaApp>, BlockHandle<DatabaseSchema>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(DatabaseSchema::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let mut test = BeuiTest::new(editor);
    test.run();
    (test, block)
}
