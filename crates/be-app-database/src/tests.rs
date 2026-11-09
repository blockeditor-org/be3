use be_sdk_beui::{Creation, Editor, EditorHost, OwnedEditor};
use be_sdk_test::BeuiTest;
use be_type_database::database::{Database, DatabaseContent};
use uuid::Uuid;

use crate::root::DatabaseRoot;

mod a_database_without_views_says_so;
mod a_new_database_starts_with_a_name_field;
mod a_new_view_of_a_database_in_no_folder_goes_to_unfiled;
mod on_a_phone_the_schema_is_gathered_beside_the_views;
mod the_schema_sidebar_goes_away_with_the_chrome;

fn editor() -> (BeuiTest<DatabaseRoot>, Editor) {
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
