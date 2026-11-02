use be_sdk_beui::beui::dock::styled::{
    DockBar, DockBarGrip, DockBarSurface, DockPane, DockSplit, SplitSize, DockTab, Docking, DockingLayout,
};
use be_sdk_beui::beui::dock::unstyled::{DockAlign, DockBarPlacement, DockEdge};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, Vec2};
use be_sdk_beui::{Editor, Loaded, narrow_chrome};
use be_sdk_database::DatabaseBlockPickRequest;
use be_sdk_database::DatabaseValueEditor;
use be_sdk_ui_database::DatabaseValueChange;
use be_type_database::database_schema::DatabaseField;
use be_type_database::database_view::DatabaseViewKind;
use uuid::Uuid;

use crate::database_view::root::calendar::CalendarView;
use crate::database_view::root::data::{Data, ViewData};
use crate::database_view::root::header::ViewHeader;
use crate::database_view::root::kanban::Kanban;
use crate::database_view::root::list::{ListView, OpenRow, RowBeside};
use crate::database_view::root::scatter::Scatter;
use crate::database_view::root::spreadsheet::Spreadsheet;
use crate::database_view::sort::{ROW_HEADER_WIDTH, ROW_HEIGHT, column_name, column_width};
use beui::{Align, Direction, ItemSize};

const CELL_LABEL_WIDTH: f32 = 56.0;
const CELL_EDITOR_HEIGHT: Size = Size::Xl;
const CELL_EDITOR_WIDTH: f32 = 360.0;
const STAGE: &str = "stage";
const ROW: &str = "row";
const CELL_BAR: &str = "cell-editor";
const ROW_SHARE: f32 = 0.5;

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
    let narrow = narrow_chrome();
    let header_shown = create_memo(move || {
        let covered = narrow.get()
            && data.kind.get() != DatabaseViewKind::Spreadsheet
            && data.selected_row().is_some();
        chrome.get() && !covered
    });
    let cell_bar = create_memo(move || chrome.get() && spreadsheet.get());
    let opened = open_row(data);
    let beside = create_memo(move || !narrow.get() && opened.get().is_some());
    let content = NodeRef::new();
    editor.content(&content);
    let layout = DockingLayout::<&'static str>::new();
    view! {
        <Docking layout focus={Some(STAGE)}>
            <DockSplit id="view" size={SplitSize::Fraction(1.0 - ROW_SHARE)}>
                <DockPane id="stage" locked=true bare=true>
                    <DockTab id=STAGE title="View">
                        <List spacing=Space::None>
                            <Show condition={header_shown}>
                                <ViewHeader data={data} />
                            </Show>
                            <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                                <Content data={data} panels=true />
                            </Frame>
                        </List>
                    </DockTab>
                </DockPane>
                <Show condition={beside}>
                    <DockPane id="row" locked=true>
                        <DockTab id=ROW title="Row">
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
                        DatabaseViewKind::Calendar => view! {
                            <CalendarView @sizing=ItemSize::Percent(100.0) data={data} />
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
                        @sizing=ItemSize::Fixed(CELL_EDITOR_WIDTH)
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
