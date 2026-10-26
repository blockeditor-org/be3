use beui::Overflow;
use block_editor_beui::beui::icons::{
    ICON_ARROW_DOWN, ICON_ARROW_UP, ICON_SQUARE, ICON_SQUARE_CHECK,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Color32, Key, KeyPress, NodeId, TextAlign};
use block_editor_beui::block_ui::database::cell_text;
use database_types::database::{DatabaseColor, DatabaseValue};
use database_types::database_schema::{DatabaseField, DatabaseFieldType};
use database_types::database_view::{DatabaseView, DatabaseViewSort, SortDirection};
use std::collections::HashMap;
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
    let display = display(data);
    let fields = data.fields;
    let keys = create_memo(move || {
        display.with(|display| display.iter().map(|row| row.index).collect::<Vec<usize>>())
    });
    let empty = create_memo(move || fields.with(Vec::is_empty));
    let filled = create_memo(move || !empty.get());
    let keyboard = Callback::new(keyboard(data, display));
    view! {
        <Frame fill=ColorRole::Background>
            <List spacing=0.0>
                <Show condition={empty}>
                    <Frame padding=12.0>
                        <Text style=TextStyle::CAPTION>"This database has no columns yet."</Text>
                    </Frame>
                </Show>
                <Show condition={filled}>
                    <Grid
                        @sizing=ItemSize::Percent(100.0)
                        data={data}
                        header={data}
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
    let positions = create_memo(move || {
        display.with(|display| {
            display
                .iter()
                .enumerate()
                .map(|(position, row)| (row.index, position))
                .collect::<HashMap<usize, usize>>()
        })
    });
    view! {
        <List spacing=0.0>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <Scroll direction=Direction::Horizontal>
                    <Interactive focusable=true on_key={on_key}>
                        <List spacing=0.0>
                            <HeaderRow data={header} />
                            <VirtualList
                                keys={keys}
                                item_size=ROW_HEIGHT
                                overscan={ROW_HEIGHT * 2.0}
                            >|index: ReadSignal<usize>|
                                <Row
                                    data={data}
                                    display={display}
                                    positions={positions}
                                    index={index}
                                />
                            </VirtualList>
                        </List>
                    </Interactive>
                </Scroll>
            </Scroll>
        </List>
    }
}

#[component]
fn HeaderRow(data: Data) -> NodeId {
    let fields = data.fields;
    let keys = create_memo(move || {
        fields.with(|fields| fields.iter().map(|field| field.id).collect::<Vec<Uuid>>())
    });
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame
                width=ROW_HEADER_WIDTH
                height=ROW_HEIGHT
                fill=ColorRole::Surface
                border={Border::new(ColorRole::Border).width(BORDER)}
            />
            <ForEach keys={keys}>|id: Uuid| {
                let field = create_memo(move || {
                    fields.with(|fields| fields.iter().find(|field| field.id == id).cloned())
                });
                let position = create_memo(move || {
                    fields.with(|fields| fields.iter().position(|field| field.id == id))
                });
                let label = create_memo(move || {
                    let name = field.with(|field| {
                        field.as_ref().map_or_else(String::new, |field| field.name.clone())
                    });
                    match position.get() {
                        Some(position) => format!("{}  {name}", column_name(position)),
                        None => name,
                    }
                });
                let width = create_memo(move || {
                    field.with(|field| {
                        field.as_ref().map_or(0.0, |field| column_width(field.field_type))
                    })
                });
                let sorted = sorted_direction(data.sort, id);
                let resort = move || {
                    let sort = next_sort(data.sort.get_untracked(), id);
                    data.operate_view(DatabaseView::set_sort(sort));
                };
                view! {
                    <HeaderCell
                        label={label}
                        width={width}
                        sorted={sorted}
                        @test_id={format!("database-view.column.{id}")}
                        on_click={resort}
                    />
                }
            }</ForEach>
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
    let glyph = create_memo(move || match sorted.get() {
        Some(SortDirection::Ascending) => Some(ICON_ARROW_UP),
        Some(SortDirection::Descending) => Some(ICON_ARROW_DOWN),
        None => None,
    });
    let marked = create_memo(move || sorted.get().is_some());
    view! {
        <Frame
            width=width
            height=ROW_HEIGHT
            fill=ColorRole::Surface
            border={Border::new(ColorRole::Border).width(BORDER)}
        >
            <Interactive on_click={on_click}>
                <Frame padding={Sides::horizontal(CELL_PADDING)}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                        <Text>{label}</Text>
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                        <Show condition={marked}>
                            <Text color=TextColor::Muted align=TextAlign::Center>
                                <Icon glyph={glyph} />
                            </Text>
                        </Show>
                    </List>
                </Frame>
            </Interactive>
        </Frame>
    }
}

#[component]
fn Row(
    data: Data,
    display: Memo<Vec<DisplayRow>>,
    positions: Memo<HashMap<usize, usize>>,
    index: ReadSignal<usize>,
) -> NodeId {
    let position =
        create_memo(move || positions.with(|positions| positions.get(&index.get()).copied()));
    let entry = create_memo(move || {
        let position = position.get()?;
        display.with(|display| display.get(position).cloned())
    });
    let number = create_memo(move || {
        position
            .get()
            .map_or_else(String::new, |position| (position + 1).to_string())
    });
    let selected = create_memo(move || {
        data.selected
            .get()
            .is_some_and(|selection| selection.row == index.get())
    });
    let fields = data.fields;
    let keys = create_memo(move || {
        fields.with(|fields| fields.iter().map(|field| field.id).collect::<Vec<Uuid>>())
    });
    let theme = use_theme();
    let header_fill = create_memo(move || match selected.get() {
        true => theme.accent_soft.get(),
        false => theme.surface.get(),
    });
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame
                @sizing=ItemSize::Fixed(ROW_HEADER_WIDTH)
                height=ROW_HEIGHT
                fill={ColorRole::custom(header_fill)}
                border={Border::new(ColorRole::Border).width(BORDER)}
            >
                <Text style=TextStyle::CAPTION align=TextAlign::Center>{number}</Text>
            </Frame>
            <ForEach keys={keys}>|id: Uuid| {
                let field = create_memo(move || {
                    fields.with(|fields| fields.iter().find(|field| field.id == id).cloned())
                });
                view! {
                    <Cell data={data} entry={entry} field={field} row={index} />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn Cell(
    data: Data,
    entry: Memo<Option<DisplayRow>>,
    field: Memo<Option<DatabaseField>>,
    row: ReadSignal<usize>,
) -> NodeId {
    let id = field
        .get_untracked()
        .map(|field| field.id)
        .unwrap_or_default();
    let width = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .map_or(0.0, |field| column_width(field.field_type))
        })
    });
    let value =
        create_memo(move || entry.with(|entry| entry.as_ref().and_then(|entry| entry.row.get(id))));
    let text = create_memo(move || {
        let (Some(entry), Some(field)) = (entry.get(), field.get()) else {
            return String::new();
        };
        if field.field_type == DatabaseFieldType::Boolean {
            return String::new();
        }
        data.labels
            .with(|labels| cell_text(&entry.row, &field, labels))
    });
    let selected = create_memo(move || {
        data.selected
            .get()
            .is_some_and(|selection| selection.row == row.get() && selection.field == Some(id))
    });
    let boolean = create_memo(move || {
        field.with(|field| {
            field
                .as_ref()
                .is_some_and(|field| field.field_type == DatabaseFieldType::Boolean)
        })
    });
    let checked = create_memo(move || matches!(value.get(), Some(DatabaseValue::Boolean(true))));
    let check_glyph = create_memo(move || match checked.get() {
        true => ICON_SQUARE_CHECK,
        false => ICON_SQUARE,
    });
    let colored = create_memo(move || matches!(value.get(), Some(DatabaseValue::Color(_))));
    let swatch = create_memo(move || match value.get() {
        Some(DatabaseValue::Color(color)) => swatch_color(color),
        _ => Color32::TRANSPARENT,
    });
    let align = create_memo(move || {
        let numeric = field.with(|field| {
            field
                .as_ref()
                .is_some_and(|field| field.field_type == DatabaseFieldType::Number)
        });
        match numeric {
            true => TextAlign::End,
            false => TextAlign::Start,
        }
    });
    let element_test_id = create_memo(move || format!("database-view.cell.{}.{id}", row.get()));
    let click = move || {
        let row = row.get_untracked();
        data.select(row, Some(id));
        if boolean.get_untracked() {
            let value = !checked.get_untracked();
            data.set_cell(row, id, Some(DatabaseValue::Boolean(value)));
        }
    };
    let shown = NodeRef::new();
    create_effect(move || {
        if selected.get()
            && let Some(node) = shown.try_get()
        {
            with_document(|document| document.reveal_node(node));
        }
    });
    let theme = use_theme();
    let glyph_color = TextColor::Default;
    let fill = create_memo(move || match selected.get() {
        true => theme.accent_soft.get(),
        false => theme.background.get(),
    });
    let outline = create_memo(move || match selected.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    });
    view! {
        <Frame
            @node_ref=&shown
            width=width
            height=ROW_HEIGHT
            fill={ColorRole::custom(fill)}
            border={create_memo(move || Some(Border::new(ColorRole::Custom(outline.get())).width(BORDER)))}
        >
            <Interactive @test_id={element_test_id} on_click={click}>
                <Frame padding={Sides::horizontal(CELL_PADDING)}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=CELL_PADDING>
                        <Show condition={boolean}>
                            <Text color={glyph_color} align=TextAlign::Center>
                                <Icon glyph={check_glyph} />
                            </Text>
                        </Show>
                        <Show condition={colored}>
                            <Frame
                                @sizing=ItemSize::Fixed(SWATCH_WIDTH)
                                height=SWATCH_HEIGHT
                                fill={ColorRole::custom(swatch)}
                                border={Border::new(ColorRole::Border).width(BORDER)}
                                radius=Radius::Xs
                            />
                        </Show>
                        <Text
                            @sizing=ItemSize::Percent(100.0)
                            align={align}
                            overflow=Overflow::Ellipsis
                        >
                            {text}
                        </Text>
                    </List>
                </Frame>
            </Interactive>
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

pub fn display(data: Data) -> Memo<Vec<DisplayRow>> {
    let rows = data.rows;
    let fields = data.fields;
    let labels = data.labels;
    let sort = data.sort;
    let filters = data.filters;
    let selected = data.selected;
    create_memo(move || {
        rows.with(|rows| {
            fields.with(|fields| {
                labels.with(|labels| {
                    filters.with(|filters| {
                        display_rows(
                            rows,
                            selected.get().map(|selection| selection.row),
                            sort.get(),
                            filters,
                            fields,
                            labels,
                        )
                    })
                })
            })
        })
    })
}

fn keyboard(data: Data, display: Memo<Vec<DisplayRow>>) -> impl Fn(KeyPress) -> bool + use<> {
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
        move_selection(data, display, step)
    }
}

fn move_selection(data: Data, display: Memo<Vec<DisplayRow>>, (dx, dy): (isize, isize)) -> bool {
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
