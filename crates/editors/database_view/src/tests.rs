use std::{cmp::Ordering, collections::HashMap};

use block_editor_plugin::be_block::Edit;
use block_editor_plugin::be_block::database::{
    Database, DatabaseColor, DatabaseContent, DatabaseValue,
};
use block_editor_plugin::be_block::database_schema::{
    DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaContent,
};
use block_editor_plugin::be_block::database_view::{
    DatabaseView, DatabaseViewContent, DatabaseViewKind,
};
use block_editor_plugin::beui::Key;
use block_editor_plugin::{
    BeuiApp, Creation, Editor, EditorHost,
    block_ui::{BlockLabel, database::DatabaseBlockPickRequest},
};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::{DatabaseViewApp, value_block_filter};
use crate::sort::compare_database_values;

mod a_new_view_starts_as_a_spreadsheet;
mod arrow_keys_walk_the_spreadsheet_and_enter_steps_down;
mod block_values_sort_by_resolved_label_then_reference;
mod cards_sit_in_the_kanban_column_their_status_names;
mod dragging_a_kanban_card_moves_it_to_the_column_it_lands_in;
mod new_type_cells_render_and_a_boolean_cell_toggles;
mod new_values_sort_by_their_typed_order;
mod selected_row_string_edit_updates_the_database;
mod switching_to_kanban_stores_the_kind;
mod the_scatter_plot_places_and_selects_a_point_for_each_row;
mod value_picker_filter_is_exact_and_never_includes_templates;

struct Fixture {
    harness: BeuiTest<DatabaseViewApp>,
    schema: Uuid,
    database: Uuid,
    fields: Vec<Uuid>,
}

impl Fixture {
    fn run(&mut self) {
        self.harness.run();
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.run();
        }
    }

    fn database(&self) -> Database {
        self.harness
            .content::<DatabaseContent>(Some(self.database))
            .root()
    }

    fn view_state(&self) -> DatabaseView {
        self.harness.content::<DatabaseViewContent>(None).root()
    }

    fn set(&mut self, row: usize, field: Uuid, value: DatabaseValue) {
        let edit = self.database().set_cell(row, field, Some(value));
        self.harness
            .edit::<DatabaseContent>(Some(self.database), &edit);
    }

    fn option(&mut self, field: Uuid, name: &str) -> Uuid {
        let (id, edit) = DatabaseSchema::add_enum_option(field, name);
        self.harness
            .edit::<DatabaseSchemaContent>(Some(self.schema), &edit);
        id
    }

    fn edit_view(&mut self, edit: Edit) {
        self.harness.edit::<DatabaseViewContent>(None, &edit);
    }
}

fn editor(fields: &[(&str, DatabaseFieldType)]) -> Fixture {
    let schema = Uuid::new_v4();
    let mut schema_content = DatabaseSchemaContent::default();
    let ids: Vec<Uuid> = fields
        .iter()
        .map(|(name, field_type)| {
            let (id, edit) = DatabaseSchema::add_field(*name, *field_type);
            schema_content.apply(&edit);
            id
        })
        .collect();
    let database = Uuid::new_v4();
    let view = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), view);
    let mut harness = BeuiTest::new(editor);
    harness.hold(None, DatabaseViewContent::new(&DatabaseView::of(database)));
    harness.hold(
        Some(database),
        DatabaseContent::new(&Database::with_schema(schema)),
    );
    harness.hold(Some(schema), schema_content);
    for _ in 0..6 {
        harness.run();
    }
    Fixture {
        harness,
        schema,
        database,
        fields: ids,
    }
}

fn text_editor() -> Fixture {
    editor(&[("Name", DatabaseFieldType::String)])
}

fn field(field_type: DatabaseFieldType) -> DatabaseField {
    DatabaseField {
        id: Uuid::new_v4(),
        name: "Field".into(),
        field_type,
        enum_options: Vec::new(),
        number_options: Default::default(),
        block_options: Default::default(),
    }
}
