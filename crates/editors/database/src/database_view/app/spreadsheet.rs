use block_editor_beui::be_block::database::{DatabaseColor, DatabaseValue};
use block_editor_beui::be_block::database_schema::{DatabaseField, DatabaseFieldType};
use block_editor_beui::be_block::database_view::{DatabaseView, DatabaseViewSort, SortDirection};
use block_editor_beui::beui::icons::{
    ICON_ARROW_DOWNWARD, ICON_ARROW_UPWARD, ICON_CHECK_BOX, ICON_CHECK_BOX_OUTLINE_BLANK,
};
use block_editor_beui::beui::reactive::{
    Align, Callback, ClickCallback, ClickCatcher, Direction, Focusable, ForEach, Frame, ItemSize,
    List, Memo, NodeRef, Prop, Show, Spacer, clone, component, create_effect, create_memo, view,
    with_document,
};
use block_editor_beui::beui::styled::{Body, Caption, Icon, Scroll, use_theme};
use block_editor_beui::beui::{Color32, Key, KeyPress, NodeId, TextAlign};
use block_editor_beui::block_ui::database::cell_text;
use uuid::Uuid;

use crate::database_view::app::data::Data;
use crate::database_view::sort::{
    DisplayRow, ROW_HEADER_WIDTH, ROW_HEIGHT, column_name, column_width, display_rows, next_sort,
};

const BORDER: f32 = 1.0;
const CELL_PADDING: f32 = 6.0;
const SWATCH_WIDTH: f32 = 18.0;
const SWATCH_HEIGHT: f32 = 14.0;

#[component]
pub fn Spreadsheet(data: Data) -> NodeId {
    let display = display(&data);
    let fields = data.fields.clone();
    let keys = create_memo(clone!(display -> move || {
        display.with(|display| display.iter().map(|row| row.index).collect::<Vec<usize>>())
    }));
    let empty = create_memo(clone!(fields -> move || fields.with(Vec::is_empty)));
    let filled = create_memo(clone!(empty -> move || !empty.get()));
    let keyboard = keyboard(&data, display.clone());
    let rows_data = data.clone();
    let header_data = data.clone();
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=0.0>
                <Show condition={empty}>
                    <Frame padding_horizontal=12.0 padding_vertical=12.0>
                        <Caption content="This database has no columns yet." />
                    </Frame>
                </Show>
                <Show condition={filled}>
                    <Grid
                        @sizing=ItemSize::Percent(100.0)
                        data={rows_data}
                        header={header_data}
                        display={display}
                        keys={keys}
                        on_key={keyboard}
                    />
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn Grid(
    data: Data,
    header: Data,
    display: Memo<Vec<DisplayRow>>,
    keys: Memo<Vec<usize>>,
    on_key: Callback<KeyPress, bool>,
) -> NodeId {
    view! {
        <List spacing=0.0>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <Scroll direction=Direction::Horizontal>
                    <Focusable on_key={move |press: KeyPress| on_key.call(press)}>
                        <List spacing=0.0>
                            <HeaderRow data={header} />
                            <ForEach keys={keys}>
                                {move |index: usize| {
                                    let data = data.clone();
                                    let display = display.clone();
                                    view! {
                                        <Row data={data} display={display} index={index} />
                                    }
                                }}
                            </ForEach>
                        </List>
                    </Focusable>
                </Scroll>
            </Scroll>
        </List>
    }
}

#[component]
fn HeaderRow(data: Data) -> NodeId {
    let fields = data.fields.clone();
    let keys = create_memo(clone!(fields -> move || {
        fields.with(|fields| fields.iter().map(|field| field.id).collect::<Vec<Uuid>>())
    }));
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame
                width=ROW_HEADER_WIDTH
                height=ROW_HEIGHT
                color={theme.surface.clone()}
                outline={theme.border.clone()}
                outline_width=BORDER
                outline_visible=true
            />
            <ForEach keys={keys}>
                {move |id: Uuid| {
                    let data = data.clone();
                    let fields = fields.clone();
                    let field = create_memo(clone!(fields -> move || {
                        fields.with(|fields| fields.iter().find(|field| field.id == id).cloned())
                    }));
                    let position = create_memo(clone!(fields -> move || {
                        fields.with(|fields| fields.iter().position(|field| field.id == id))
                    }));
                    let label = create_memo(clone!(field position -> move || {
                        let name = field.with(|field| {
                            field.as_ref().map_or_else(String::new, |field| field.name.clone())
                        });
                        match position.get() {
                            Some(position) => format!("{}  {name}", column_name(position)),
                            None => name,
                        }
                    }));
                    let width = create_memo(clone!(field -> move || {
                        field.with(|field| {
                            field.as_ref().map_or(0.0, |field| column_width(field.field_type))
                        })
                    }));
                    let sorted = sorted_direction(data.sort.clone(), id);
                    let resort = clone!(data -> move || {
                        let sort = next_sort(data.sort.get_untracked(), id);
                        data.operate_view(DatabaseView::set_sort(sort));
                    });
                    view! {
                        <HeaderCell
                            label={label}
                            width={width}
                            sorted={sorted}
                            @test_id={format!("database-view.column.{id}")}
                            on_click={resort}
                        />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn HeaderCell(
    label: Prop<String>,
    width: Prop<f32>,
    sorted: Prop<Option<SortDirection>>,
    on_click: ClickCallback,
) -> NodeId {
    let sorted = create_memo(move || sorted.get());
    let glyph = create_memo(clone!(sorted -> move || match sorted.get() {
        Some(SortDirection::Ascending) => ICON_ARROW_UPWARD.to_owned(),
        Some(SortDirection::Descending) => ICON_ARROW_DOWNWARD.to_owned(),
        None => String::new(),
    }));
    let marked = create_memo(clone!(sorted -> move || sorted.get().is_some()));
    let theme = use_theme();
    view! {
        <Frame
            width={width}
            height=ROW_HEIGHT
            color={theme.surface.clone()}
            outline={theme.border.clone()}
            outline_width=BORDER
            outline_visible=true
        >
            <ClickCatcher on_click={move || on_click.call()}>
                <Frame padding_horizontal=CELL_PADDING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                        <Body content={label} color={theme.text.clone()} />
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                        <Show condition={marked}>
                            <Icon glyph={glyph} color={theme.text_muted.clone()} />
                        </Show>
                    </List>
                </Frame>
            </ClickCatcher>
        </Frame>
    }
}

#[component]
fn Row(data: Data, display: Memo<Vec<DisplayRow>>, index: usize) -> NodeId {
    let entry = create_memo(clone!(display -> move || {
        display.with(|display| display.iter().find(|row| row.index == index).cloned())
    }));
    let position = create_memo(clone!(display -> move || {
        display.with(|display| display.iter().position(|row| row.index == index))
    }));
    let number = create_memo(clone!(position -> move || {
        position.get().map_or_else(String::new, |position| (position + 1).to_string())
    }));
    let selected = create_memo(clone!(data -> move || {
        data.selected.get().is_some_and(|selection| selection.row == index)
    }));
    let fields = data.fields.clone();
    let keys = create_memo(clone!(fields -> move || {
        fields.with(|fields| fields.iter().map(|field| field.id).collect::<Vec<Uuid>>())
    }));
    let theme = use_theme();
    let header_fill = create_memo(clone!(theme selected -> move || match selected.get() {
        true => theme.accent_soft.get(),
        false => theme.surface.get(),
    }));
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame
                @sizing=ItemSize::Fixed(ROW_HEADER_WIDTH)
                height=ROW_HEIGHT
                color={header_fill}
                outline={theme.border.clone()}
                outline_width=BORDER
                outline_visible=true
            >
                <Caption content={number} align=TextAlign::Center />
            </Frame>
            <ForEach keys={keys}>
                {move |id: Uuid| {
                    let data = data.clone();
                    let entry = entry.clone();
                    let fields = fields.clone();
                    let field = create_memo(clone!(fields -> move || {
                        fields.with(|fields| fields.iter().find(|field| field.id == id).cloned())
                    }));
                    view! {
                        <Cell data={data} entry={entry} field={field} row={index} />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn Cell(
    data: Data,
    entry: Memo<Option<DisplayRow>>,
    field: Memo<Option<DatabaseField>>,
    row: usize,
) -> NodeId {
    let id = field
        .get_untracked()
        .map(|field| field.id)
        .unwrap_or_default();
    let width = create_memo(clone!(field -> move || {
        field.with(|field| field.as_ref().map_or(0.0, |field| column_width(field.field_type)))
    }));
    let value = create_memo(clone!(entry -> move || {
        entry.with(|entry| entry.as_ref().and_then(|entry| entry.row.value(id).cloned()))
    }));
    let text = create_memo(clone!(entry field data -> move || {
        let (Some(entry), Some(field)) = (entry.get(), field.get()) else {
            return String::new();
        };
        if field.field_type == DatabaseFieldType::Boolean {
            return String::new();
        }
        data.labels.with(|labels| cell_text(&entry.row, &field, labels))
    }));
    let selected = create_memo(clone!(data -> move || {
        data.selected
            .get()
            .is_some_and(|selection| selection.row == row && selection.field == Some(id))
    }));
    let boolean = create_memo(clone!(field -> move || {
        field.with(|field| {
            field
                .as_ref()
                .is_some_and(|field| field.field_type == DatabaseFieldType::Boolean)
        })
    }));
    let checked = create_memo(clone!(value -> move || {
        matches!(value.get(), Some(DatabaseValue::Boolean(true)))
    }));
    let check_glyph = create_memo(clone!(checked -> move || match checked.get() {
        true => ICON_CHECK_BOX.to_owned(),
        false => ICON_CHECK_BOX_OUTLINE_BLANK.to_owned(),
    }));
    let colored = create_memo(clone!(value -> move || {
        matches!(value.get(), Some(DatabaseValue::Color(_)))
    }));
    let swatch = create_memo(clone!(value -> move || match value.get() {
        Some(DatabaseValue::Color(color)) => swatch_color(color),
        _ => Color32::TRANSPARENT,
    }));
    let align = create_memo(clone!(field -> move || {
        let numeric = field.with(|field| {
            field
                .as_ref()
                .is_some_and(|field| field.field_type == DatabaseFieldType::Number)
        });
        match numeric {
            true => TextAlign::End,
            false => TextAlign::Start,
        }
    }));
    let click = clone!(data boolean checked -> move || {
        data.select(row, Some(id));
        if boolean.get_untracked() {
            let value = !checked.get_untracked();
            data.set_cell(row, id, Some(DatabaseValue::Boolean(value)));
        }
    });
    let shown = NodeRef::new();
    create_effect(clone!(selected shown -> move || {
        if selected.get()
            && let Some(node) = shown.try_get()
        {
            with_document(|document| document.reveal_node(node));
        }
    }));
    let theme = use_theme();
    let glyph_color = theme.text.clone();
    let fill = create_memo(clone!(theme selected -> move || match selected.get() {
        true => theme.accent_soft.get(),
        false => theme.background.get(),
    }));
    let outline = create_memo(clone!(theme selected -> move || match selected.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    }));
    view! {
        <Frame
            @node_ref=&shown
            width={width}
            height=ROW_HEIGHT
            color={fill}
            outline={outline}
            outline_width=BORDER
            outline_visible=true
        >
            <ClickCatcher @test_id={format!("database-view.cell.{row}.{id}")} on_click={click}>
                <Frame padding_horizontal=CELL_PADDING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=CELL_PADDING>
                        <Show condition={boolean}>
                            <Icon glyph={check_glyph} color={glyph_color} />
                        </Show>
                        <Show condition={colored}>
                            <Frame
                                @sizing=ItemSize::Fixed(SWATCH_WIDTH)
                                height=SWATCH_HEIGHT
                                color={swatch}
                                outline={theme.border.clone()}
                                outline_width=BORDER
                                outline_visible=true
                                radius=2
                            />
                        </Show>
                        <Body
                            @sizing=ItemSize::Percent(100.0)
                            content={text}
                            align={align}
                            color={theme.text.clone()}
                        />
                    </List>
                </Frame>
            </ClickCatcher>
        </Frame>
    }
}

fn swatch_color(color: DatabaseColor) -> Color32 {
    Color32::from_rgba_unmultiplied(color.red, color.green, color.blue, color.alpha)
}

fn sorted_direction(
    sort: Memo<Option<DatabaseViewSort>>,
    field_id: Uuid,
) -> Memo<Option<SortDirection>> {
    create_memo(move || {
        sort.get()
            .filter(|sort| sort.field_id == field_id)
            .map(|sort| sort.direction)
    })
}

pub fn display(data: &Data) -> Memo<Vec<DisplayRow>> {
    let rows = data.rows.clone();
    let fields = data.fields.clone();
    let labels = data.labels.clone();
    let sort = data.sort.clone();
    let selected = data.selected.clone();
    create_memo(move || {
        rows.with(|rows| {
            fields.with(|fields| {
                labels.with(|labels| {
                    display_rows(
                        rows,
                        selected.get().map(|selection| selection.row),
                        sort.get(),
                        fields,
                        labels,
                    )
                })
            })
        })
    })
}

fn keyboard(data: &Data, display: Memo<Vec<DisplayRow>>) -> impl Fn(KeyPress) -> bool + use<> {
    let data = data.clone();
    move |press: KeyPress| {
        if !press.pressed {
            return false;
        }
        let step = match press.key {
            Key::ArrowLeft => (-1, 0),
            Key::ArrowRight => (1, 0),
            Key::ArrowUp => (0, -1),
            Key::ArrowDown => (0, 1),
            Key::Delete | Key::Backspace => {
                let Some(selection) = data.selected.get_untracked() else {
                    return false;
                };
                let Some(field) = selection.field else {
                    return false;
                };
                data.set_cell(selection.row, field, None);
                return true;
            }
            _ => return false,
        };
        move_selection(&data, &display, step)
    }
}

fn move_selection(data: &Data, display: &Memo<Vec<DisplayRow>>, (dx, dy): (isize, isize)) -> bool {
    let rows = display.get_untracked();
    let fields = data.fields.get_untracked();
    if rows.is_empty() || fields.is_empty() {
        return false;
    }
    let (position, column) = match data.selected.get_untracked() {
        Some(selection) => {
            let position = rows
                .iter()
                .position(|row| row.index == selection.row)
                .unwrap_or(0);
            let column = selection
                .field
                .and_then(|field| fields.iter().position(|shown| shown.id == field))
                .unwrap_or(0);
            (
                position.saturating_add_signed(dy).min(rows.len() - 1),
                column.saturating_add_signed(dx).min(fields.len() - 1),
            )
        }
        None => (0, 0),
    };
    data.select(rows[position].index, Some(fields[column].id));
    true
}
