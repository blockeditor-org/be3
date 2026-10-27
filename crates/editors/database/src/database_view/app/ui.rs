use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::block_ui::database::DatabaseValueChange;
use block_editor_beui::database::DatabaseBlockPickRequest;
use block_editor_beui::database::DatabaseValueEditor;
use block_editor_beui::{Editor, Loaded, Toolbar, narrow_chrome};
use database_types::database_schema::DatabaseField;
use database_types::database_view::DatabaseViewKind;
use uuid::Uuid;

use crate::database_view::app::data::{Data, ViewData};
use crate::database_view::app::header::ViewHeader;
use crate::database_view::app::kanban::Kanban;
use crate::database_view::app::list::{ListView, RowBeside};
use crate::database_view::app::scatter::Scatter;
use crate::database_view::app::spreadsheet::Spreadsheet;
use crate::database_view::sort::{ROW_HEADER_WIDTH, ROW_HEIGHT, column_name, column_width};

const CELL_LABEL_WIDTH: f32 = 56.0;
const CELL_EDITOR_HEIGHT: Size = Size::Xl;

#[component]
pub fn DatabaseViewEditor(editor: Editor) -> NodeId {
    view! {
        <Loaded label="Loading the database">
            <ViewEditor editor={editor} />
        </Loaded>
    }
}

#[component]
fn ViewEditor(editor: Editor) -> NodeId {
    let data = ViewData::new(editor);
    view! {
        <EditorBody data={data} />
    }
}

#[component]
pub(crate) fn EditorBody(data: Data) -> NodeId {
    let editor = data.editor();
    create_effect(move || {
        let size = match data.kind.get() {
            DatabaseViewKind::Spreadsheet => Some(spreadsheet_size(data)),
            DatabaseViewKind::Kanban | DatabaseViewKind::Scatter | DatabaseViewKind::List => None,
        };
        editor.set_intrinsic_size(size);
    });

    let spreadsheet = create_memo(move || data.kind.get() == DatabaseViewKind::Spreadsheet);
    let chrome = editor.chrome_shown();
    let narrow = narrow_chrome();
    let header_shown = create_memo(move || {
        let covered = narrow.get()
            && data.kind.get() != DatabaseViewKind::Spreadsheet
            && data.selected_row().is_some();
        chrome.get() && !covered
    });
    let content = NodeRef::new();
    editor.content(&content);
    view! {
        <List spacing=Space::None>
            <Show condition={spreadsheet}>
                <Toolbar shown={chrome}>
                    <CellEditor @sizing=ItemSize::Percent(100.0) data={data} />
                </Toolbar>
            </Show>
            <Show condition={header_shown}>
                <ViewHeader data={data} />
            </Show>
            <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                <Content data={data} panels=true />
            </Frame>
        </List>
    }
}

#[component]
pub fn DatabaseViewPreview(editor: Editor) -> NodeId {
    view! {
        <Loaded label="Loading the database">
            <PreviewContent editor={editor} />
        </Loaded>
    }
}

#[component]
fn PreviewContent(editor: Editor) -> NodeId {
    let data = ViewData::new(editor);
    view! {
        <Content data={data} panels=false />
    }
}

#[component]
fn Content(data: Data, panels: bool) -> NodeId {
    let kind = data.kind;
    view! {
        <RowBeside data={data} enabled={panels}>
            <List spacing=Space::None>
                <Dynamic value={kind}>|kind: DatabaseViewKind| {
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
                        DatabaseViewKind::List => view! {
                            <ListView @sizing=ItemSize::Percent(100.0) data={data} />
                        },
                    }
                }</Dynamic>
            </List>
        </RowBeside>
    }
}

#[component]
fn CellEditor(data: Data) -> NodeId {
    let field = selected_field(data);
    let chosen = create_memo(move || field.get().is_some());
    let label = create_memo(move || {
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
    });
    let fields = create_memo(move || field.get().into_iter().collect::<Vec<_>>());
    let values = create_memo(move || {
        let Some(selection) = data.selected.get() else {
            return Default::default();
        };
        data.rows.with(|rows| {
            rows.get(selection.row)
                .map(|row| row.cells())
                .unwrap_or_default()
        })
    });
    let changed = move |change: DatabaseValueChange| {
        if let Some(selection) = data.selected.get_untracked() {
            data.set_cell(selection.row, change.field_id, change.value);
        }
    };
    let picked = move |request: DatabaseBlockPickRequest| {
        if let Some(selection) = data.selected.get_untracked() {
            data.pick_value(selection.row, request);
        }
    };
    let submitted = move |_: Uuid| data.step_selection(1);
    let labels = data.labels;
    let read_only = data.read_only;
    view! {
        <Frame height=CELL_EDITOR_HEIGHT>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Frame @sizing={ItemSize::Intrinsic.min(CELL_LABEL_WIDTH)}>
                    <Text @test_id={"database-view.cell-editor.cell"}>{label}</Text>
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

fn selected_field(data: Data) -> Memo<Option<DatabaseField>> {
    create_memo(move || {
        let field = data.selected.get()?.field?;
        data.fields
            .with(|fields| fields.iter().find(|shown| shown.id == field).cloned())
    })
}

fn spreadsheet_size(data: Data) -> Vec2 {
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
