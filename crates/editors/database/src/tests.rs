use std::sync::Arc;

use block_client::block_ref::BlockRef;
use block_client::blocks::database::Database;
use block_client::blocks::database_schema::DatabaseSchema;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{Creation, Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::DatabaseApp;

mod a_database_without_views_says_so;
mod a_new_database_starts_with_a_name_field;
mod the_schema_sidebar_goes_away_with_the_chrome;

fn editor() -> (
    BeuiTest<DatabaseApp>,
    Arc<BlockClient>,
    BlockHandle<Database>,
    Editor,
) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let schema = client.create_block(DatabaseSchema::new());
    let block = client.create_block(Database::new(BlockRef::Direct(schema.id())));
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, Arc::clone(&client), block.id());
    let mut test = BeuiTest::new(editor.clone());
    test.run();
    (test, client, block, editor)
}
