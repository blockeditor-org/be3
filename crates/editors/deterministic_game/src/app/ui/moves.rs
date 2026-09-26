use block_editor_beui::beui::icons::{
    ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_FIRST_PAGE, ICON_LAST_PAGE,
};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, Text, clone, component,
    create_memo, view,
};
use block_editor_beui::beui::styled::{Caption, IconButton, IconButtonSize, use_theme};
use block_editor_beui::beui::unstyled;
use block_editor_beui::beui::{Color32, NodeId, TextAlign};

use super::{Steps, Table};

const TURN_SIZE: f32 = 13.0;
const SCORE_SIZE: f32 = 15.0;
const NUMBER_WIDTH: f32 = 30.0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Cell {
    pub(crate) text: String,
    pub(crate) first: usize,
    pub(crate) last: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Row {
    pub(crate) cells: Vec<Option<Cell>>,
}

pub(crate) fn rows(table: &Table) -> Vec<Row> {
    let width = table.columns.len();
    let mut rows: Vec<Row> = Vec::new();
    let mut previous: Option<usize> = None;
    for (index, turn) in table.history.iter().enumerate() {
        let Some(column) = turn
            .column
            .map(|column| column as usize)
            .filter(|column| *column < width)
        else {
            continue;
        };
        match (previous, rows.last_mut()) {
            (Some(previous), Some(row)) if column == previous => {
                if let Some(cell) = &mut row.cells[column] {
                    cell.text.push(' ');
                    cell.text.push_str(&turn.description);
                    cell.last = index;
                    continue;
                }
            }
            (Some(previous), Some(_)) if column > previous => {}
            _ => rows.push(Row {
                cells: vec![None; width],
            }),
        }
        let row = rows.last_mut().expect("a row was just started");
        row.cells[column] = Some(Cell {
            text: turn.description.clone(),
            first: index,
            last: index,
        });
        previous = Some(column);
    }
    rows
}

#[component]
pub(crate) fn MoveTable(table: Memo<Table>, shown: Memo<usize>, steps: Steps) -> NodeId {
    let rows = create_memo(clone!(table -> move || table.with(rows)));
    let keys = create_memo(clone!(rows -> move || (0..rows.with(Vec::len)).collect::<Vec<_>>()));
    let columns = create_memo(clone!(table -> move || table.with(|table| table.columns.clone())));
    let column_keys =
        create_memo(clone!(columns -> move || (0..columns.with(Vec::len)).collect::<Vec<_>>()));
    let empty = create_memo(clone!(rows -> move || rows.with(Vec::is_empty)));
    let count = steps.count.clone();
    let at_start = create_memo(clone!(shown -> move || shown.get() == 0));
    let at_end = create_memo(clone!(shown count -> move || shown.get() >= count.get()));
    let ended = create_memo(clone!(table -> move || table.with(|table| table.ending.is_some())));
    let (first, previous, next, last) =
        (steps.clone(), steps.clone(), steps.clone(), steps.clone());
    let theme = use_theme();
    let muted = create_memo(clone!(theme -> move || theme.get().text_muted));
    let header_keys = column_keys.clone();
    view! {
        <List spacing=6.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=2.0>
                <Caption content="Moves" @sizing=ItemSize::Percent(100.0) />
                <IconButton
                    glyph={ICON_FIRST_PAGE.to_owned()}
                    label="First move"
                    size=IconButtonSize::Compact
                    disabled={at_start.clone()}
                    @test_id={"game.history.first"}
                    on_click={move || first.first()}
                />
                <IconButton
                    glyph={ICON_CHEVRON_LEFT.to_owned()}
                    label="Previous move"
                    size=IconButtonSize::Compact
                    disabled={at_start}
                    @test_id={"game.history.previous"}
                    on_click={move || previous.previous()}
                />
                <IconButton
                    glyph={ICON_CHEVRON_RIGHT.to_owned()}
                    label="Next move"
                    size=IconButtonSize::Compact
                    disabled={at_end.clone()}
                    @test_id={"game.history.next"}
                    on_click={move || next.next()}
                />
                <IconButton
                    glyph={ICON_LAST_PAGE.to_owned()}
                    label="Latest move"
                    size=IconButtonSize::Compact
                    disabled={at_end}
                    @test_id={"game.history.last"}
                    on_click={move || last.last()}
                />
            </List>
            <List direction=Direction::Horizontal spacing=2.0>
                <Text
                    string="#"
                    font_size=TURN_SIZE
                    color={muted.clone()}
                    @sizing=ItemSize::Fixed(NUMBER_WIDTH)
                />
                <ForEach keys={header_keys}>
                    {move |column: usize| {
                        let name = create_memo(clone!(columns -> move || {
                            columns.with(|columns| columns.get(column).cloned().unwrap_or_default())
                        }));
                        view! {
                            <Text
                                string={name}
                                font_size=TURN_SIZE
                                color={muted.clone()}
                                @sizing=ItemSize::Percent(100.0)
                            />
                        }
                    }}
                </ForEach>
            </List>
            <Show condition={empty}>
                <Caption content="No moves yet" />
            </Show>
            <List spacing=1.0>
                <ForEach keys>
                    {move |row: usize| view! {
                        <MoveRow
                            row
                            rows={rows.clone()}
                            columns={column_keys.clone()}
                            shown={shown.clone()}
                            steps={steps.clone()}
                        />
                    }}
                </ForEach>
            </List>
            <Show condition={ended}>
                <Result table />
            </Show>
        </List>
    }
}

#[component]
fn MoveRow(
    row: usize,
    rows: Memo<Vec<Row>>,
    columns: Memo<Vec<usize>>,
    shown: Memo<usize>,
    steps: Steps,
) -> NodeId {
    let theme = use_theme();
    let muted = create_memo(clone!(theme -> move || theme.get().text_muted));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=2.0>
            <Text
                string={format!("{}", row + 1)}
                font_size=TURN_SIZE
                color={muted}
                @sizing=ItemSize::Fixed(NUMBER_WIDTH)
            />
            <ForEach keys={columns}>
                {move |column: usize| view! {
                    <MoveCell
                        row
                        column
                        rows={rows.clone()}
                        shown={shown.clone()}
                        steps={steps.clone()}
                        @sizing=ItemSize::Percent(100.0)
                    />
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn MoveCell(
    row: usize,
    column: usize,
    rows: Memo<Vec<Row>>,
    shown: Memo<usize>,
    steps: Steps,
) -> NodeId {
    let cell = create_memo(move || {
        rows.with(|rows| {
            rows.get(row)
                .and_then(|row| row.cells.get(column).cloned())
                .flatten()
        })
    });
    let text = create_memo(clone!(cell -> move || {
        cell.with(|cell| cell.as_ref().map(|cell| cell.text.clone()).unwrap_or_default())
    }));
    let theme = use_theme();
    let chosen = create_memo(clone!(cell -> move || {
        let shown = shown.get();
        cell.with(|cell| {
            cell.as_ref()
                .is_some_and(|cell| shown > cell.first && shown <= cell.last + 1)
        })
    }));
    let fill = create_memo(clone!(theme chosen -> move || match chosen.get() {
        true => theme.get().accent_soft,
        false => Color32::TRANSPARENT,
    }));
    let color = create_memo(clone!(theme -> move || theme.get().text));
    let open = clone!(cell -> move || {
        if let Some(cell) = cell.get_untracked() {
            steps.show(cell.last + 1);
        }
    });
    view! {
        <unstyled::Button
            @test_id={format!("game.history.{row}.{column}")}
            on_click={open}
            content={move |_: unstyled::ButtonHandle| view! {
                <Frame color={fill.clone()} radius=4 padding_horizontal=6.0 padding_vertical=3.0>
                    <Text string={text.clone()} font_size=TURN_SIZE color={color.clone()} />
                </Frame>
            }}
        />
    }
}

#[component]
fn Result(table: Memo<Table>) -> NodeId {
    let score = create_memo(clone!(table -> move || {
        table.with(|table| {
            table
                .ending
                .as_ref()
                .and_then(|ending| ending.score.clone())
                .unwrap_or_default()
        })
    }));
    let scored = create_memo(clone!(score -> move || score.with(|score| !score.is_empty())));
    let description = create_memo(clone!(table -> move || {
        table.with(|table| {
            table
                .ending
                .as_ref()
                .map(|ending| ending.description.clone())
                .unwrap_or_default()
        })
    }));
    let theme = use_theme();
    let text = create_memo(clone!(theme -> move || theme.get().text));
    let muted = create_memo(clone!(theme -> move || theme.get().text_muted));
    view! {
        <List spacing=2.0 align=Align::Center @test_id={"game.result"}>
            <Show condition={scored}>
                <Text
                    string={score}
                    font_size=SCORE_SIZE
                    bold=true
                    color={text}
                    align=TextAlign::Center
                />
            </Show>
            <Text string={description} font_size=TURN_SIZE color={muted} align=TextAlign::Center />
        </List>
    }
}
