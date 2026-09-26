use block_editor_plugin::be_block::database_schema::DatabaseField;
use block_editor_plugin::be_block::database_view::{DatabaseView, DatabaseViewKind};
use block_editor_plugin::be_block::{BlockContent, DatabaseSchemaContent};
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::icons::{
    ICON_DESELECT, ICON_GRID_ON, ICON_SCATTER_PLOT, ICON_SCHEMA, ICON_VIEW_KANBAN,
};
use block_editor_plugin::beui::reactive::{
    Align, Callback, Direction, ForEach, ItemSize, List, Memo, Show, Spacer, clone, component,
    create_memo, view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Heading, Select, Separator, ToggleButton, use_theme,
};
use block_editor_plugin::beui::unstyled::ChoiceOption;
use block_editor_plugin::block_ui::database::{DatabaseBlockPickRequest, DatabaseValueChange};
use block_editor_plugin::database::DatabaseValueEditor;
use uuid::Uuid;

use crate::database_view::app::data::Data;
use crate::database_view::app::kanban::enum_fields;
use crate::database_view::app::scatter::number_fields;

const SPACING: f32 = 10.0;
const KINDS: [(DatabaseViewKind, &str, &str); 3] = [
    (DatabaseViewKind::Spreadsheet, "Spreadsheet", ICON_GRID_ON),
    (DatabaseViewKind::Kanban, "Kanban", ICON_VIEW_KANBAN),
    (DatabaseViewKind::Scatter, "Scatter", ICON_SCATTER_PLOT),
];

#[component]
pub fn ViewSidebar(data: Data) -> NodeId {
    let failed = create_memo(clone!(data -> move || data.error.with(Option::is_some)));
    let message = create_memo(clone!(data -> move || data.error.get().unwrap_or_default()));
    let dismiss = clone!(data -> move || data.dismiss_error());
    let columns = clone!(data -> move || {
        if let Some(schema) = data.schema_id.get_untracked() {
            data.editor().host().open_block(schema, DatabaseSchemaContent::CONTENT_TYPE);
        }
    });
    let kanban = create_memo(clone!(data -> move || data.kind.get() == DatabaseViewKind::Kanban));
    let scatter = create_memo(clone!(data -> move || data.kind.get() == DatabaseViewKind::Scatter));
    let (switch, status, axes, item) = (data.clone(), data.clone(), data.clone(), data.clone());
    let theme = use_theme();
    view! {
        <List spacing=SPACING>
            <Show condition={failed}>
                <List spacing=SPACING>
                    <Body content={message} color={theme.danger.clone()} />
                    <List direction=Direction::Horizontal spacing=0.0>
                        <Button
                            label="Dismiss"
                            variant=ButtonVariant::Secondary
                            @test_id={"database-view.dismiss-error"}
                            on_click={dismiss}
                        />
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                    </List>
                    <Separator />
                </List>
            </Show>
            <Heading content="View" />
            <ViewSwitch data={switch} />
            <Show condition={kanban}>
                <StatusPicker data={status} />
            </Show>
            <Show condition={scatter}>
                <AxisPickers data={axes} />
            </Show>
            <Separator />
            <List direction=Direction::Horizontal spacing=0.0>
                <Button
                    glyph={ICON_SCHEMA.to_owned()}
                    label="Columns"
                    variant=ButtonVariant::Secondary
                    @test_id={"database-view.columns"}
                    on_click={columns}
                />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
            <Separator />
            <SelectedItem data={item} />
        </List>
    }
}

#[component]
fn ViewSwitch(data: Data) -> NodeId {
    let keys = create_memo(|| (0..KINDS.len()).collect::<Vec<usize>>());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=6.0 wrap=true>
            <ForEach keys={keys}>
                {move |index: usize| {
                    let (wanted, label, glyph) = KINDS[index];
                    let data = data.clone();
                    let pressed = create_memo(clone!(data -> move || data.kind.get() == wanted));
                    let choose = clone!(data -> move |_: bool| {
                        if data.kind.get_untracked() != wanted {
                            data.operate_view(DatabaseView::set_kind(wanted));
                        }
                    });
                    view! {
                        <ToggleButton
                            label={label}
                            glyph={glyph.to_owned()}
                            pressed={pressed}
                            @test_id={format!("database-view.kind.{label}")}
                            on_change={choose}
                        />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn StatusPicker(data: Data) -> NodeId {
    let options = enum_fields(&data);
    let empty = create_memo(clone!(options -> move || options.with(Vec::is_empty)));
    let chosen = data.kanban_field.clone();
    let picked = clone!(data options -> move |index: Option<usize>| {
        let field_id = index.and_then(|index| {
            options.with_untracked(|options| options.get(index).map(|field| field.id))
        });
        data.operate_view(DatabaseView::set_kanban_field(field_id));
    });
    view! {
        <List spacing=6.0>
            <Caption content="Status field" />
            <FieldPicker
                options={options}
                chosen={chosen}
                label="Status field"
                picker_id={"database-view.kanban-field".to_owned()}
                on_change={picked}
            />
            <Show condition={empty}>
                <Caption content="Add an Enum column to use the kanban view." wrap=true />
            </Show>
        </List>
    }
}

#[component]
fn AxisPickers(data: Data) -> NodeId {
    let options = number_fields(&data);
    let empty = create_memo(clone!(options -> move || options.with(Vec::is_empty)));
    let x_options = options.clone();
    let y_options = options.clone();
    let x_chosen = data.scatter_x.clone();
    let y_chosen = data.scatter_y.clone();
    let pick_x = clone!(data options -> move |index: Option<usize>| {
        let field_id = index.and_then(|index| {
            options.with_untracked(|options| options.get(index).map(|field| field.id))
        });
        data.operate_view(DatabaseView::set_scatter_x(field_id));
    });
    let pick_y = clone!(data options -> move |index: Option<usize>| {
        let field_id = index.and_then(|index| {
            options.with_untracked(|options| options.get(index).map(|field| field.id))
        });
        data.operate_view(DatabaseView::set_scatter_y(field_id));
    });
    view! {
        <List spacing=6.0>
            <Caption content="X axis" />
            <FieldPicker
                options={x_options}
                chosen={x_chosen}
                label="X axis"
                picker_id={"database-view.scatter-x".to_owned()}
                on_change={pick_x}
            />
            <Caption content="Y axis" />
            <FieldPicker
                options={y_options}
                chosen={y_chosen}
                label="Y axis"
                picker_id={"database-view.scatter-y".to_owned()}
                on_change={pick_y}
            />
            <Show condition={empty}>
                <Caption content="Add a Number column to use the scatter view." wrap=true />
            </Show>
        </List>
    }
}

#[component]
fn FieldPicker(
    options: Memo<Vec<DatabaseField>>,
    chosen: Memo<Option<Uuid>>,
    label: &'static str,
    picker_id: String,
    on_change: Callback<Option<usize>>,
) -> NodeId {
    let keys = create_memo(clone!(options -> move || {
        options.with(|options| options.iter().map(|field| field.id).collect::<Vec<Uuid>>())
    }));
    let selected = create_memo(clone!(options chosen -> move || {
        let chosen = chosen.get()?;
        options.with(|options| options.iter().position(|field| field.id == chosen))
    }));
    let names = options.clone();
    view! {
        <Select
            options={view! {
                <ForEach keys={keys}>
                    {move |id: Uuid| {
                        let name = create_memo(clone!(names -> move || {
                            names.with(|options| {
                                options
                                    .iter()
                                    .find(|field| field.id == id)
                                    .map_or_else(String::new, |field| field.name.clone())
                            })
                        }));
                        view! {
                            <ChoiceOption label={name} />
                        }
                    }}
                </ForEach>
            }}
            selected={selected}
            label={label}
            @test_id={picker_id}
            on_change={move |index: Option<usize>| on_change.call(index)}
        />
    }
}

#[component]
fn SelectedItem(data: Data) -> NodeId {
    let row =
        create_memo(clone!(data -> move || data.selected.get().map(|selection| selection.row)));
    let chosen = create_memo(clone!(row -> move || row.get().is_some()));
    let editing = chosen.clone();
    let none = create_memo(clone!(row -> move || row.get().is_none()));
    let title = create_memo(clone!(row -> move || match row.get() {
        Some(row) => format!("Row {}", row + 1),
        None => "Selected item".to_owned(),
    }));
    let values = create_memo(clone!(data row -> move || {
        let Some(row) = row.get() else {
            return Default::default();
        };
        data.rows.with(|rows| {
            rows.get(row).map(|row| row.values().clone()).unwrap_or_default()
        })
    }));
    let deselect = clone!(data -> move || data.deselect());
    let changed = clone!(data row -> move |change: DatabaseValueChange| {
        if let Some(index) = row.get_untracked() {
            data.set_cell(index, change.field_id, change.value);
        }
    });
    let picked = clone!(data row -> move |request: DatabaseBlockPickRequest| {
        if let Some(index) = row.get_untracked() {
            data.pick_value(index, request);
        }
    });
    let fields = data.fields.clone();
    let labels = data.labels.clone();
    let read_only = data.read_only.clone();
    view! {
        <List spacing=SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Heading content={title} />
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Show condition={chosen}>
                    <Button
                        glyph={ICON_DESELECT.to_owned()}
                        label="Deselect"
                        variant=ButtonVariant::Secondary
                        @test_id={"database-view.deselect"}
                        on_click={deselect}
                    />
                </Show>
            </List>
            <Show condition={none}>
                <Caption content="Select a row to edit it here." wrap=true />
            </Show>
            <Show condition={editing}>
                <DatabaseValueEditor
                    fields={fields}
                    values={values}
                    labels={labels}
                    disabled={read_only}
                    prefix="database-view.selected-item"
                    on_change={changed}
                    on_pick={picked}
                />
            </Show>
        </List>
    }
}
