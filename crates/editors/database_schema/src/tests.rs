use std::sync::Arc;

use block_client::BlockClient;
use block_client::blocks::database_schema::DatabaseSchema as SchemaBlock;
use block_editor_plugin::be_block::database_schema::{DatabaseSchema, DatabaseSchemaContent};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::DatabaseSchemaApp;

mod adding_a_field_appends_a_string_field;
mod configured_fields_paint_on_separate_lines;
mod renaming_a_field_writes_the_new_name_to_the_block;

fn editor() -> ContentHarness<DatabaseSchemaApp> {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(SchemaBlock::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), client, block.id());
    let mut harness = ContentHarness::new(BeuiTest::new(editor), host);
    harness.hold(None, DatabaseSchemaContent::default());
    harness.run();
    harness
}

fn schema(harness: &ContentHarness<DatabaseSchemaApp>) -> DatabaseSchema {
    harness.content::<DatabaseSchemaContent>(None).root()
}
