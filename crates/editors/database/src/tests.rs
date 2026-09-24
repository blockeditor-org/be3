use std::sync::Arc;

use block_client::BlockClient;
use block_client::blocks::database::Database as DatabaseBlock;
use block_client::blocks::database_schema::DatabaseSchema as SchemaBlock;
use block_editor_plugin::be_block::database::{Database, DatabaseContent};
use block_editor_plugin::{Creation, Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::DatabaseApp;

mod a_database_without_views_says_so;
mod a_new_database_starts_with_a_name_field;
mod the_schema_sidebar_goes_away_with_the_chrome;

fn editor() -> (ContentHarness<DatabaseApp>, Editor) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let schema = client.create_block(SchemaBlock::new());
    let block = client.create_block(DatabaseBlock::with_references(vec![schema.id()]));
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), Arc::clone(&client), block.id());
    let mut harness = ContentHarness::new(BeuiTest::new(editor.clone()), host);
    harness.hold(
        None,
        DatabaseContent::new(&Database::with_schema(schema.id())),
    );
    harness.run();
    (harness, editor)
}
