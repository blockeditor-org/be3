use std::{cmp::Ordering, collections::HashMap, sync::Arc};

use block_client::block_ref::BlockRef;
use block_client::blocks::database::{Database, DatabaseColor, DatabaseOperation, DatabaseValue};
use block_client::blocks::database_schema::{
    DatabaseEnumOption, DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaOperation,
};
use block_client::blocks::database_view::{DatabaseView, DatabaseViewKind, DatabaseViewOperation};
use block_client::{BlockClient, BlockHandle};
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
    test: BeuiTest<DatabaseViewApp>,
    view: BlockHandle<DatabaseView>,
    schema: BlockHandle<DatabaseSchema>,
    database: BlockHandle<Database>,
    fields: Vec<Uuid>,
}

impl Fixture {
    fn settle(&mut self) {
        for _ in 0..3 {
            self.test.run();
        }
    }

    fn set(&self, row_index: usize, field_id: Uuid, value: DatabaseValue) {
        self.database.operate(DatabaseOperation::SetCell {
            row_index,
            field_id,
            value: Some(value),
        });
    }
}

fn editor(fields: &[(&str, DatabaseFieldType)]) -> Fixture {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let schema = client.create_block(DatabaseSchema::new());
    let ids: Vec<Uuid> = fields
        .iter()
        .map(|(name, field_type)| {
            let id = Uuid::new_v4();
            schema.operate(DatabaseSchemaOperation::AddField {
                field: DatabaseField {
                    id,
                    name: (*name).into(),
                    field_type: *field_type,
                    enum_options: Vec::new(),
                    number_options: Default::default(),
                    block_options: Default::default(),
                },
            });
            id
        })
        .collect();
    let database = client.create_block(Database::new(BlockRef::Direct(schema.id())));
    let view = client.create_block(DatabaseView::new(BlockRef::Direct(database.id())));
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, Arc::clone(&client), view.id());
    let mut test = BeuiTest::new(editor);
    for _ in 0..6 {
        test.run();
    }
    Fixture {
        test,
        view,
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
