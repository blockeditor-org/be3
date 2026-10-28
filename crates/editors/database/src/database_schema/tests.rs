use block_editor_beui::{EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use block_ui_test::prelude::*;
use database_types::database_schema::{DatabaseSchema, DatabaseSchemaContent};
use uuid::Uuid;

use crate::database_schema::app::DatabaseSchemaApp;

mod a_choice_option_takes_an_icon_and_can_be_closed_and_the_default;
mod a_relation_field_names_the_database_it_links_to;
mod adding_a_field_appends_a_string_field;
mod configured_fields_paint_on_separate_lines;
mod renaming_a_field_writes_the_new_name_to_the_block;

fn editor() -> BeuiTest<DatabaseSchemaApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = OwnedEditor::new(host.clone(), block);
    let mut harness = BeuiTest::new(editor);
    harness.hold(None, DatabaseSchemaContent::default());
    harness.run();
    harness
}

fn schema(harness: &BeuiTest<DatabaseSchemaApp>) -> DatabaseSchema {
    harness.content::<DatabaseSchemaContent>(None).root()
}
