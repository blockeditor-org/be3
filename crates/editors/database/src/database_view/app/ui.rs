use block_editor_beui::be_block::database_schema::DatabaseField;
use block_editor_beui::be_block::database_view::DatabaseViewKind;
use block_editor_beui::beui::reactive::{
    Align, Direction, Dynamic, Frame, ItemSize, Layers, List, Memo, NodeRef, Show, component,
    create_effect, create_memo, view,
};
use block_editor_beui::beui::styled::use_theme;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::block_ui::database::{DatabaseBlockPickRequest, DatabaseValueChange};
use block_editor_beui::database::DatabaseValueEditor;
use block_editor_beui::{Editor, Loading, Toolbar, narrow_chrome};
use uuid::Uuid;

use crate::database_view::app::data::{Data, ViewData};
use crate::database_view::app::header::ViewHeader;
use crate::database_view::app::kanban::Kanban;
use crate::database_view::app::list::{ListView, RowBeside};
use crate::database_view::app::scatter::Scatter;
use crate::database_view::app::spreadsheet::Spreadsheet;
use crate::database_view::sort::{ROW_HEADER_WIDTH, ROW_HEIGHT, column_name, column_width};
use block_editor_beui::beui::styled::Text;

const CELL_LABEL_WIDTH: f32 = 56.0;
const CELL_EDITOR_HEIGHT: f32 = 40.0;

#[component]
pub fn DatabaseViewEditor(editor: Editor) -> NodeId {
    let data = ViewData::new(editor);
    create_effect(move || {
        if !data.loaded.get() {
            editor.set_intrinsic_size(None);
            return;
        }
        let size = match data.kind.get() {
            DatabaseViewKind::Spreadsheet => Some(spreadsheet_size(data)),
            DatabaseViewKind::Kanban | DatabaseViewKind::Scatter | DatabaseViewKind::List => None,
        };
        editor.set_intrinsic_size(size);
    });

    let loaded = data.loaded;
    let loading = create_memo(move || !loaded.get());
    let spreadsheet =
        create_memo(move || loaded.get() && data.kind.get() == DatabaseViewKind::Spreadsheet);
    let chrome = editor.chrome_shown();
    let toolbar_chrome = chrome;
    let narrow = narrow_chrome();
    let header_shown = create_memo(move || {
        let covered = narrow.get()
            && data.kind.get() != DatabaseViewKind::Spreadsheet
            && data.selected_row().is_some();
        loaded.get() && chrome.get() && !covered
    });
    let content = NodeRef::new();
    editor.content(&content);
    let theme = use_theme();
    view! {
        <Frame color={theme.background}>
            <List spacing=0.0>
                <Show condition={spreadsheet}>
                    {move || view! {
                        <Toolbar shown={toolbar_chrome}>
                            <CellEditor @sizing=ItemSize::Percent(100.0) data={data} />
                        </Toolbar>
                    }}
                </Show>
                <Show condition={header_shown}>
                    {move || view! {
                        <ViewHeader data={data} />
                    }}
                </Show>
                <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                    <Layers>
                        <Show condition={loading}>
                            <Loading label="Loading the database" />
                        </Show>
                        <Show condition={loaded}>
                            {move || view! {
                                <Content data={data} panels=true />
                            }}
                        </Show>
                    </Layers>
                </Frame>
            </List>
        </Frame>
    }
}

#[component]
pub fn DatabaseViewPreview(editor: Editor) -> NodeId {
    let data = ViewData::new(editor);
    let loaded = data.loaded;
    let loading = create_memo(move || !loaded.get());
    let theme = use_theme();
    view! {
        <Frame color={theme.background}>
            <Layers>
                <Show condition={loading}>
                    <Loading label="Loading the database" />
                </Show>
                <Show condition={loaded}>
                    {move || view! {
                        <Content data={data} panels=false />
                    }}
                </Show>
            </Layers>
        </Frame>
    }
}

#[component]
fn Content(data: Data, panels: bool) -> NodeId {
    let kind = data.kind;
    view! {
        <RowBeside data={data} enabled={panels}>
            <List spacing=0.0>
                <Dynamic value={kind}>
                    {move |kind: DatabaseViewKind| {
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
                    }}
                </Dynamic>
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
            <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                <Frame @sizing={ItemSize::Intrinsic.min(CELL_LABEL_WIDTH)}>
                    <Text @test_id={"database-view.cell-editor.cell"} string={label} />
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
