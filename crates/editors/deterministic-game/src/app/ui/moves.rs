use beui::Overflow;
use block_editor_beui::beui::icons::{
    ICON_CHEVRON_FIRST, ICON_CHEVRON_LAST, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, TextAlign};

use super::{Steps, Table};
use beui::{Align, Direction, ItemSize};

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
    let rows = create_memo(move || table.with(rows));
    let keys = create_memo(move || (0..rows.with(Vec::len)).collect::<Vec<_>>());
    let columns = create_memo(move || table.with(|table| table.columns.clone()));
    let column_keys = create_memo(move || (0..columns.with(Vec::len)).collect::<Vec<_>>());
    let empty = create_memo(move || rows.with(Vec::is_empty));
    let count = steps.count;
    let at_start = create_memo(move || shown.get() == 0);
    let at_end = create_memo(move || shown.get() >= count.get());
    let ended = create_memo(move || table.with(|table| table.ending.is_some()));
    let muted = TextColor::Muted;
    view! {
        <List spacing=Space::Sm>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs2>
                <Text style=TextStyle::CAPTION @sizing=ItemSize::Percent(100.0)>"Moves"</Text>
                <Button
                    icon_only=true
                    glyph=ICON_CHEVRON_FIRST
                    label="First move"
                    size=ButtonSize::Small
                    disabled={at_start}
                    @test_id={"game.history.first"}
                    on_click={move || steps.first()}
                />
                <Button
                    icon_only=true
                    glyph=ICON_CHEVRON_LEFT
                    label="Previous move"
                    size=ButtonSize::Small
                    disabled={at_start}
                    @test_id={"game.history.previous"}
                    on_click={move || steps.previous()}
                />
                <Button
                    icon_only=true
                    glyph=ICON_CHEVRON_RIGHT
                    label="Next move"
                    size=ButtonSize::Small
                    disabled={at_end}
                    @test_id={"game.history.next"}
                    on_click={move || steps.next()}
                />
                <Button
                    icon_only=true
                    glyph=ICON_CHEVRON_LAST
                    label="Latest move"
                    size=ButtonSize::Small
                    disabled={at_end}
                    @test_id={"game.history.last"}
                    on_click={move || steps.last()}
                />
            </List>
            <List direction=Direction::Horizontal spacing=Space::Xs2>
                <Text size=TextSize::Sm color={muted} @sizing=ItemSize::Fixed(NUMBER_WIDTH)>
                    "#"
                </Text>
                <ForEach keys={column_keys}>|column: usize| {
                    let name = create_memo(move || {
                        columns.with(|columns| columns.get(column).cloned().unwrap_or_default())
                    });
                    view! {
                        <Text
                            overflow=Overflow::Ellipsis
                            size=TextSize::Sm
                            color={muted}
                            @sizing=ItemSize::Percent(100.0)
                        >
                            {name}
                        </Text>
                    }
                }</ForEach>
            </List>
            <Show condition={empty}>
                <Text style=TextStyle::CAPTION>"No moves yet"</Text>
            </Show>
            <List spacing=Space::Xs2>
                <ForEach keys>|row: usize|
                    <MoveRow row rows={rows} columns={column_keys} shown={shown} steps={steps} />
                </ForEach>
            </List>
            <Show condition={ended}>
                <Result table={table} />
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
    let muted = TextColor::Muted;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs2>
            <Text size=TextSize::Sm color={muted} @sizing=ItemSize::Fixed(NUMBER_WIDTH)>
                {format!("{}", row + 1)}
            </Text>
            <ForEach keys={columns}>|column: usize|
                <MoveCell
                    row
                    column
                    rows={rows}
                    shown={shown}
                    steps={steps}
                    @sizing=ItemSize::Percent(100.0)
                />
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
    let text = create_memo(move || {
        cell.with(|cell| {
            cell.as_ref()
                .map(|cell| cell.text.clone())
                .unwrap_or_default()
        })
    });
    let chosen = create_memo(move || {
        let shown = shown.get();
        cell.with(|cell| {
            cell.as_ref()
                .is_some_and(|cell| shown > cell.first && shown <= cell.last + 1)
        })
    });
    let open = move || {
        if let Some(cell) = cell.get_untracked() {
            steps.show(cell.last + 1);
        }
    };
    view! {
        <ListRow
            label={text}
            selected={chosen}
            @test_id={format!("game.history.{row}.{column}")}
            on_click={open}
        />
    }
}

#[component]
fn Result(table: Memo<Table>) -> NodeId {
    let score = create_memo(move || {
        table.with(|table| {
            table
                .ending
                .as_ref()
                .and_then(|ending| ending.score.clone())
                .unwrap_or_default()
        })
    });
    let scored = create_memo(move || score.with(|score| !score.is_empty()));
    let description = create_memo(move || {
        table.with(|table| {
            table
                .ending
                .as_ref()
                .map(|ending| ending.description.clone())
                .unwrap_or_default()
        })
    });
    let text = TextColor::Default;
    let muted = TextColor::Muted;
    view! {
        <List spacing=Space::Xs2 align=Align::Center @test_id={"game.result"}>
            <Show condition={scored}>
                <Text size=TextSize::Lg weight=Weight::Bold color={text} align=TextAlign::Center>
                    {score}
                </Text>
            </Show>
            <Text size=TextSize::Sm color={muted} align=TextAlign::Center>{description}</Text>
        </List>
    }
}
