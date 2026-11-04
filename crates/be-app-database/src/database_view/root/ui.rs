use be_sdk_beui::beui::dock::styled::{DockBarGrip, DockBarSurface};
use be_sdk_beui::beui::dock::{DockAlign, DockBarPlacement, DockEdge, DockMode};
use be_sdk_beui::beui::dock::{
    DockBar, DockPane, DockSplit, DockTab, Docking, DockingLayout, SplitSize,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, Vec2};
use be_sdk_beui::{Editor, Loaded};
use be_sdk_database::DatabaseBlockPickRequest;
use be_sdk_database::DatabaseValueEditor;
use be_sdk_ui_database::DatabaseValueChange;
use be_type_database::database::short_id;
use be_type_database::database_schema::DatabaseField;
use be_type_database::database_view::DatabaseViewKind;
use uuid::Uuid;

use crate::database_view::root::calendar::CalendarView;
use crate::database_view::root::data::{Data, ViewData};
use crate::database_view::root::header::ViewHeader;
use crate::database_view::root::kanban::Kanban;
use crate::database_view::root::list::{ListView, OpenRow};
use crate::database_view::root::scatter::Scatter;
use crate::database_view::root::spreadsheet::Spreadsheet;
use crate::database_view::sort::{ROW_HEADER_WIDTH, ROW_HEIGHT, column_name, column_width};
use beui::{Align, Direction};

const CELL_LABEL_WIDTH: f32 = 56.0;
const CELL_EDITOR_HEIGHT: Size = Size::Xl;
const CELL_EDITOR_WIDTH: f32 = 360.0;
const STAGE: &str = "stage";
const ROW: &str = "row";
const CELL_BAR: &str = "cell-editor";
const ROW_WIDTH: f32 = 460.0;

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
            DatabaseViewKind::Kanban
            | DatabaseViewKind::Scatter
            | DatabaseViewKind::List
            | DatabaseViewKind::Calendar => None,
        };
        editor.set_intrinsic_size(size);
    });

    let spreadsheet = create_memo(move || data.kind.get() == DatabaseViewKind::Spreadsheet);
    let chrome = editor.chrome_shown();
    let cell_bar = create_memo(move || chrome.get() && spreadsheet.get());
    let opened = open_row(data);
    let beside = create_memo(move || opened.get().is_some());
    let code = create_memo(move || {
        opened
            .get()
            .map(|row| short_id(&data.prefix.get(), row.as_uuid()))
            .unwrap_or_default()
    });
    let content = NodeRef::new();
    editor.content(&content);
    let layout = DockingLayout::<&'static str>::new();
    create_effect(move || {
        if opened.get().is_some() && layout.mode().get_untracked() == DockMode::Compact {
            layout.show(&ROW);
        }
    });
    view! {
        <Docking layout focus={Some(STAGE)}>
            <DockSplit id="view" size={SplitSize::Second(ROW_WIDTH)}>
                <DockPane id="stage" locked=true bare=true>
                    <DockTab id=STAGE title="View">
                        <List spacing=Space::None>
                            <Show condition={chrome}>
                                <ViewHeader data={data} />
                            </Show>
                            <ListItem grow=1.0>
                                <Frame @node_ref={&content}>
                                    <Content data={data} />
                                </Frame>
                            </ListItem>
                        </List>
                    </DockTab>
                </DockPane>
                <Show condition={beside}>
                    <DockPane id="row" locked=true>
                        <DockTab id=ROW title={code} on_close={move || data.deselect()}>
                            <OpenRow data={data} row={opened} />
                        </DockTab>
                    </DockPane>
                </Show>
            </DockSplit>
            <Show condition={cell_bar}>
                <DockBar
                    id=CELL_BAR
                    parent=STAGE
                    placement={DockBarPlacement::Beside(DockEdge::Top, DockAlign::Start)}
                >
                    <DockBarSurface spacing=Space::Md @test_id={"database-view.cell-editor"}>
                        <DockBarGrip />
                        <CellEditor data={data} />
                    </DockBarSurface>
                </DockBar>
            </Show>
        </Docking>
    }
}

fn open_row(data: Data) -> Memo<Option<be_sdk_beui::be_block::ObjectId>> {
    create_memo(move || match data.kind.get() {
        DatabaseViewKind::Spreadsheet => None,
        _ => data.selected_row(),
    })
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
        <Content data={data} />
    }
}

#[component]
fn Content(data: Data) -> NodeId {
    let kind = data.kind;
    view! {
        <List spacing=Space::None>
            <Dynamic value={kind}>|kind: DatabaseViewKind| {
                match kind {
                    DatabaseViewKind::Spreadsheet => view! {
                        <ListItem grow=1.0>
                            <Spreadsheet data={data} />
                        </ListItem>
                    },
                    DatabaseViewKind::Kanban => view! {
                        <ListItem grow=1.0>
                            <Kanban data={data} />
                        </ListItem>
                    },
                    DatabaseViewKind::Scatter => view! {
                        <ListItem grow=1.0>
                            <Scatter data={data} />
                        </ListItem>
                    },
                    DatabaseViewKind::List => view! {
                        <ListItem grow=1.0>
                            <ListView data={data} />
                        </ListItem>
                    },
                    DatabaseViewKind::Calendar => view! {
                        <ListItem grow=1.0>
                            <CalendarView data={data} />
                        </ListItem>
                    },
                }
            }</Dynamic>
        </List>
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
                <ListItem min=CELL_LABEL_WIDTH>
                    <Frame>
                        <Text @test_id={"database-view.cell-editor.cell"}>{label}</Text>
                    </Frame>
                </ListItem>
                <Show condition={chosen}>
                    <ListItem length=CELL_EDITOR_WIDTH>
                        <DatabaseValueEditor
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
                    </ListItem>
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
