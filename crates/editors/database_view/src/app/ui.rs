use block_client::blocks::database_schema::DatabaseField;
use block_client::blocks::database_view::DatabaseViewKind;
use block_editor_plugin::beui::reactive::{
    Align, Direction, Dynamic, Frame, ItemSize, List, Memo, NodeRef, Show, clone, component,
    create_effect, create_memo, view,
};
use block_editor_plugin::beui::styled::{Body, use_theme};
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::block_ui::database::{DatabaseBlockPickRequest, DatabaseValueChange};
use block_editor_plugin::database::DatabaseValueEditor;
use block_editor_plugin::{Editor, Sidebar, Toolbar};
use uuid::Uuid;

use crate::app::data::{Data, ViewData};
use crate::app::kanban::Kanban;
use crate::app::scatter::Scatter;
use crate::app::sidebar::ViewSidebar;
use crate::app::spreadsheet::Spreadsheet;
use crate::sort::{ROW_HEADER_WIDTH, ROW_HEIGHT, column_name, column_width};

const CELL_LABEL_WIDTH: f32 = 56.0;
const CELL_EDITOR_HEIGHT: f32 = 40.0;

#[component]
pub fn DatabaseViewEditor(editor: Editor) -> NodeId {
    let data = ViewData::new(&editor);
    let sized = editor.clone();
    let measured = data.clone();
    create_effect(move || {
        let size = match measured.kind.get() {
            DatabaseViewKind::Spreadsheet => Some(spreadsheet_size(&measured)),
            DatabaseViewKind::Kanban | DatabaseViewKind::Scatter => None,
        };
        sized.set_intrinsic_size(size);
    });

    let spreadsheet = create_memo(clone!(data -> move || {
        data.kind.get() == DatabaseViewKind::Spreadsheet
    }));
    let chrome = editor.chrome_shown();
    let toolbar_chrome = chrome.clone();
    let content = NodeRef::new();
    editor.content(&content);
    let (bar, body, panel) = (data.clone(), data.clone(), data);
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=0.0>
                <Show condition={spreadsheet}>
                    <Toolbar shown={toolbar_chrome}>
                        <CellEditor @sizing=ItemSize::Percent(100.0) data={bar} />
                    </Toolbar>
                </Show>
                <List @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal spacing=0.0>
                    <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                        <Content data={body} />
                    </Frame>
                    <Sidebar shown={chrome}>
                        <ViewSidebar data={panel} />
                    </Sidebar>
                </List>
            </List>
        </Frame>
    }
}

#[component]
pub fn DatabaseViewPreview(editor: Editor) -> NodeId {
    let data = ViewData::new(&editor);
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <Content data={data} />
        </Frame>
    }
}

#[component]
fn Content(data: Data) -> NodeId {
    let kind = data.kind.clone();
    view! {
        <List spacing=0.0>
            <Dynamic value={kind}>
                {move |kind: DatabaseViewKind| {
                    let data = data.clone();
                    match kind {
                        DatabaseViewKind::Spreadsheet => view! {
                            <Spreadsheet @sizing=ItemSize::Percent(100.0) data={data} />
                        },
                        DatabaseViewKind::Kanban => view! {
                            <Kanban @sizing=ItemSize::Percent(100.0) data={data} />
                        },
                        DatabaseViewKind::Scatter => view! {
                            <Scatter @sizing=ItemSize::Percent(100.0) data={data} />
                        },
                    }
                }}
            </Dynamic>
        </List>
    }
}

#[component]
fn CellEditor(data: Data) -> NodeId {
    let field = selected_field(&data);
    let chosen = create_memo(clone!(field -> move || field.get().is_some()));
    let label = create_memo(clone!(data field -> move || {
        let (Some(selection), Some(field)) = (data.selected.get(), field.get()) else {
            return "Select a cell".to_owned();
        };
        let column = data
            .fields
            .with(|fields| fields.iter().position(|shown| shown.id == field.id));
        match column {
            Some(column) => format!("{}{}", column_name(column), selection.row + 1),
            None => String::new(),
        }
    }));
    let fields = create_memo(clone!(field -> move || field.get().into_iter().collect::<Vec<_>>()));
    let values = create_memo(clone!(data -> move || {
        let Some(selection) = data.selected.get() else {
            return Default::default();
        };
        data.rows.with(|rows| {
            rows.get(selection.row)
                .map(|row| row.values().clone())
                .unwrap_or_default()
        })
    }));
    let changed = clone!(data -> move |change: DatabaseValueChange| {
        if let Some(selection) = data.selected.get_untracked() {
            data.set_cell(selection.row, change.field_id, change.value);
        }
    });
    let picked = clone!(data -> move |request: DatabaseBlockPickRequest| {
        if let Some(selection) = data.selected.get_untracked() {
            data.pick_value(selection.row, request);
        }
    });
    let submitted = clone!(data -> move |_: Uuid| data.step_selection(1));
    let labels = data.labels.clone();
    let read_only = data.read_only.clone();
    view! {
        <Frame height=CELL_EDITOR_HEIGHT>
            <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                <Frame @sizing=ItemSize::Fixed(CELL_LABEL_WIDTH)>
                    <Body @test_id={"database-view.cell-editor.cell"} content={label} />
                </Frame>
                <Show condition={chosen}>
                    <DatabaseValueEditor
                        @sizing=ItemSize::Percent(100.0)
                        fields={fields}
                        values={values}
                        labels={labels}
                        disabled={read_only}
                        prefix="database-view.cell-editor"
                        headings=false
                        on_change={changed}
                        on_pick={picked}
                        on_submit={submitted}
                    />
                </Show>
            </List>
        </Frame>
    }
}

fn selected_field(data: &Data) -> Memo<Option<DatabaseField>> {
    let data = data.clone();
    create_memo(move || {
        let field = data.selected.get()?.field?;
        data.fields
            .with(|fields| fields.iter().find(|shown| shown.id == field).cloned())
    })
}

fn spreadsheet_size(data: &Data) -> Vec2 {
    let width = ROW_HEADER_WIDTH
        + data.fields.with(|fields| {
            fields
                .iter()
                .map(|field| column_width(field.field_type))
                .sum::<f32>()
        });
    let rows = data.rows.with(Vec::len) + 2;
    Vec2::new(width, ROW_HEIGHT * rows as f32)
}
