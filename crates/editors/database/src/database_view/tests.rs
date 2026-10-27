use std::cell::Cell;
use std::rc::Rc;
use std::{cmp::Ordering, collections::HashMap};

use block_editor_beui::be_block::{Edit, ObjectId};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::beui::{Key, PointerButton, Pos2, Vec2};
use block_editor_beui::{
    BeuiApp, Creation, EditorHost, OwnedEditor, block_ui::BlockLabel,
    database::DatabaseBlockPickRequest,
};
use block_ui_test::BeuiTest;
use database_types::database::{Database, DatabaseColor, DatabaseContent, DatabaseValue};
use database_types::database_schema::{
    DatabaseField, DatabaseFieldType, DatabaseSchema, DatabaseSchemaContent,
};
use database_types::database_view::{DatabaseView, DatabaseViewContent, DatabaseViewKind};
use uuid::Uuid;

use crate::database_view::app::data::{Data, ViewData};
use crate::database_view::app::ui::EditorBody;
use crate::database_view::app::{DatabaseViewApp, value_block_filter};
use crate::database_view::groups::GroupKey;
use crate::database_view::sort::{BlockLabels, compare_database_values};

mod a_blocked_row_lists_its_blockers_and_they_list_it_as_blocking;
mod a_card_carried_into_another_column_lands_where_its_gap_opened;
mod a_card_dragged_down_its_column_parts_the_cards_it_passes;
mod a_filter_hides_the_rows_it_rejects;
mod a_list_groups_rows_by_status_and_folds_the_closed_ones;
mod a_list_shows_each_row_by_its_short_id_and_opens_it_beside_the_list;
mod a_list_waits_for_the_rows_its_relations_name;
mod a_new_row_takes_each_choices_default;
mod a_new_view_starts_as_a_spreadsheet;
mod a_phone_opens_a_row_over_the_list_and_back_returns_to_it;
mod a_prefix_is_listed_in_the_workspaces_prefixes_and_a_shared_one_says_so;
mod a_relation_cell_shows_the_title_of_the_row_it_links_to;
mod a_row_scrolled_into_a_reused_line_opens_the_row_it_shows;
mod a_sorted_kanban_moves_cards_between_columns_but_not_within_one;
mod a_sorted_view_refuses_manual_ordering;
mod adding_a_page_links_a_new_text_block_to_the_row;
mod arrow_keys_walk_the_spreadsheet_and_enter_steps_down;
mod block_values_sort_by_resolved_label_then_reference;
mod cards_sit_in_the_kanban_column_their_status_names;
mod dragging_a_kanban_card_moves_it_to_the_column_it_lands_in;
mod dragging_a_list_row_moves_it_within_and_between_groups;
mod moving_a_card_within_and_across_kanban_columns;
mod moving_a_row_reorders_the_list_view;
mod new_starts_a_row_and_typing_names_it;
mod new_type_cells_render_and_a_boolean_cell_toggles;
mod new_values_sort_by_their_typed_order;
mod searching_keeps_the_rows_whose_title_or_id_matches;
mod selected_row_string_edit_updates_the_database;
mod switching_to_kanban_stores_the_kind;
mod the_filter_menu_keeps_only_the_rows_of_the_values_ticked;
mod the_scatter_plot_places_and_selects_a_point_for_each_row;
mod typing_a_prefix_puts_it_in_front_of_every_row_id;
mod value_picker_filter_is_exact_and_never_includes_templates;

struct Fixture {
    harness: BeuiTest<DatabaseViewApp>,
    schema: Uuid,
    database: Uuid,
    fields: Vec<Uuid>,
    data: Rc<Cell<Option<Data>>>,
}

impl Fixture {
    fn data(&self) -> Data {
        self.data
            .get()
            .expect("the fixture was opened with `viewed`")
    }

    fn row(&self, index: usize) -> ObjectId {
        self.database().rows.get_index(index).unwrap().id
    }

    fn card(&self, index: usize) -> String {
        format!("database-view.card.{}", self.row(index).as_uuid())
    }

    fn order(&self) -> Vec<ObjectId> {
        self.database().rows.iter().map(|row| row.id).collect()
    }

    fn run(&mut self) {
        self.harness.run();
    }

    fn settle(&mut self) {
        self.run();
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

    fn lift(&mut self, from: Pos2, to: Pos2) {
        self.harness.down(from, PointerButton::Primary);
        self.harness.next_frame();
        self.harness.hover(from + Vec2::new(0.0, 8.0));
        self.harness.next_frame();
        self.harness.hover(to);
    }

    fn put(&mut self, at: Pos2) {
        self.harness.next_frame();
        self.harness.up(at, PointerButton::Primary);
        self.settle();
    }
}

struct Board {
    fixture: Fixture,
    status: Uuid,
    done: Uuid,
}

fn board(cards: &[(&str, bool)]) -> Board {
    let mut fixture = viewed(&[
        ("Status", DatabaseFieldType::Enum),
        ("Name", DatabaseFieldType::String),
    ]);
    let (status, name) = (fixture.fields[0], fixture.fields[1]);
    let todo = fixture.option(status, "To do");
    let done = fixture.option(status, "Done");
    for (index, (title, finished)) in cards.iter().enumerate() {
        let state = if *finished { done } else { todo };
        fixture.set(index, status, DatabaseValue::Enum(state));
        fixture.set(index, name, DatabaseValue::String((*title).into()));
    }
    fixture.edit_view(DatabaseView::set_group_by(Some(status)));
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::Kanban));
    fixture.settle();
    Board {
        fixture,
        status,
        done,
    }
}

fn editor(fields: &[(&str, DatabaseFieldType)]) -> Fixture {
    open(fields, false)
}

fn viewed(fields: &[(&str, DatabaseFieldType)]) -> Fixture {
    open(fields, true)
}

fn open(fields: &[(&str, DatabaseFieldType)], keep_data: bool) -> Fixture {
    let schema = Uuid::from_u128(0xdb_1001);
    let mut schema_content = DatabaseSchemaContent::default();
    let ids: Vec<Uuid> = fields
        .iter()
        .map(|(name, field_type)| {
            let (id, edit) = DatabaseSchema::add_field(*name, *field_type);
            schema_content.apply(&edit);
            id
        })
        .collect();
    let database = Uuid::from_u128(0xdb_1002);
    let view = Uuid::from_u128(0xdb_1003);
    let host = EditorHost::default();
    host.set_editable(true);
    let owner = OwnedEditor::new(host.clone(), view);
    let data = Rc::new(Cell::new(None::<Data>));
    let mut harness = match keep_data {
        false => BeuiTest::new(owner),
        true => {
            let editor = *owner;
            let keep = Rc::clone(&data);
            BeuiTest::with_view(owner, move || {
                let data = ViewData::new(editor);
                keep.set(Some(data));
                view! {
                    <Root>
                        <EditorBody data={data} />
                    </Root>
                }
            })
        }
    };
    harness.hold(None, DatabaseViewContent::new(&DatabaseView::of(database)));
    harness.hold(
        Some(database),
        DatabaseContent::new(&Database::with_schema(schema)),
    );
    harness.hold(Some(schema), schema_content);
    harness.run();
    Fixture {
        harness,
        schema,
        database,
        fields: ids,
        data,
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
        relation_options: Default::default(),
        default_option: None,
    }
}

struct Tracker {
    fixture: Fixture,
    title: Uuid,
    status: Uuid,
    project: Uuid,
    backlog: Uuid,
    review: Uuid,
    done: Uuid,
}

impl Tracker {
    fn row(&self, index: usize) -> block_editor_beui::be_block::ObjectId {
        self.fixture.database().rows.get_index(index).unwrap().id
    }

    fn line(&self, index: usize) -> String {
        format!("database-view.list.row.{}", self.row(index).as_uuid())
    }
}

fn tracker() -> Tracker {
    use database_types::database_schema::{ChoiceColor, ChoiceIcon};

    let mut fixture = editor(&[
        ("Title", DatabaseFieldType::String),
        ("Status", DatabaseFieldType::Enum),
        ("Priority", DatabaseFieldType::Enum),
        ("Project", DatabaseFieldType::Enum),
    ]);
    let (title, status, priority, project) = (
        fixture.fields[0],
        fixture.fields[1],
        fixture.fields[2],
        fixture.fields[3],
    );
    let schema = fixture.schema;
    let mut choice =
        |field: Uuid, name: &str, icon: ChoiceIcon, color: ChoiceColor, closed: bool| {
            let id = fixture.option(field, name);
            for edit in [
                DatabaseSchema::set_option_icon(id, icon),
                DatabaseSchema::set_option_color(id, color),
                DatabaseSchema::set_option_closed(id, closed),
            ] {
                fixture
                    .harness
                    .edit::<DatabaseSchemaContent>(Some(schema), &edit);
            }
            id
        };
    let backlog = choice(
        status,
        "Backlog",
        ChoiceIcon::Backlog,
        ChoiceColor::Gray,
        false,
    );
    let started = choice(
        status,
        "In Progress",
        ChoiceIcon::Started,
        ChoiceColor::Yellow,
        false,
    );
    let review = choice(
        status,
        "In Review",
        ChoiceIcon::Review,
        ChoiceColor::Green,
        false,
    );
    let done = choice(status, "Done", ChoiceIcon::Done, ChoiceColor::Blue, true);
    let urgent = choice(
        priority,
        "Urgent",
        ChoiceIcon::Urgent,
        ChoiceColor::Orange,
        false,
    );
    let high = choice(priority, "High", ChoiceIcon::High, ChoiceColor::Gray, false);
    let wayland = choice(
        project,
        "Wayland",
        ChoiceIcon::None,
        ChoiceColor::Blue,
        false,
    );
    let beui = choice(
        project,
        "Beui",
        ChoiceIcon::None,
        ChoiceColor::Purple,
        false,
    );
    fixture.harness.edit::<DatabaseSchemaContent>(
        Some(schema),
        &DatabaseSchema::set_default_option(status, Some(backlog)),
    );
    let rows = [
        (
            "Ctrl+Shift+I opens both the host's inspector and the plugin's",
            started,
            None,
            None,
        ),
        ("small protocols", review, None, Some(wayland)),
        (
            "beui: Text always lays out from Span children, reshaping only spans that change",
            review,
            None,
            Some(beui),
        ),
        ("freeze prep", backlog, Some(urgent), None),
        (
            "one wl_output per monitor",
            backlog,
            Some(high),
            Some(wayland),
        ),
        ("fractional scaling", backlog, None, Some(wayland)),
        (
            "block-app --session: system tray",
            done,
            None,
            Some(wayland),
        ),
    ];
    for (index, (name, state, urgency, area)) in rows.into_iter().enumerate() {
        fixture.set(index, title, DatabaseValue::String(name.into()));
        fixture.set(index, status, DatabaseValue::Enum(state));
        if let Some(urgency) = urgency {
            fixture.set(index, priority, DatabaseValue::Enum(urgency));
        }
        if let Some(area) = area {
            fixture.set(index, project, DatabaseValue::Enum(area));
        }
    }
    let database = fixture.database;
    fixture
        .harness
        .edit::<DatabaseContent>(Some(database), &Database::set_prefix("BE"));
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::List));
    fixture.edit_view(DatabaseView::set_group_by(Some(status)));
    fixture.settle();
    Tracker {
        fixture,
        title,
        status,
        project,
        backlog,
        review,
        done,
    }
}
