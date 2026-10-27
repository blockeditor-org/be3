use block_editor_beui::{Creation, Editor, EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use block_ui_test::prelude::*;
use database_types::database::{Database, DatabaseContent};
use uuid::Uuid;

use crate::app::DatabaseApp;

mod a_database_without_views_says_so;
mod a_new_database_starts_with_a_name_field;
mod the_schema_sidebar_goes_away_with_the_chrome;

fn editor() -> (BeuiTest<DatabaseApp>, Editor) {
    let schema = Uuid::new_v4();
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let owner = OwnedEditor::new(host.clone(), block);
    let editor = *owner;
    let mut harness = BeuiTest::new(owner);
    harness.hold(None, DatabaseContent::new(&Database::with_schema(schema)));
    harness.run();
    (harness, editor)
}
