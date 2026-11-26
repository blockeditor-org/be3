use be_sdk_beui::beui::icons::{
    ICON_CHEVRON_FIRST, ICON_CHEVRON_LAST, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, TextAlign};
use beui::Overflow;

use super::{Steps, Table};
use beui::{Align, Direction};

const NUMBER_WIDTH: f32 = 30.0;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Cell {
    pub(crate) text: String,
    pub(crate) first: usize,
    pub(crate) last: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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
    let columns = create_memo(move || table.with(|table| table.columns.clone()));
    let count = steps.count;
    let at_start = create_memo(move || shown.get() == 0);
    let at_end = create_memo(move || shown.get() >= count.get());
    let ended = create_memo(move || table.with(|table| table.ending.is_some()));
    let muted = TextColor::Muted;
    view! {
        <List spacing=Space::Sm>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs2>
                <ListItem grow=1.0>
                    <Text style=TextStyle::CAPTION>"Moves"</Text>
                </ListItem>
                <Button
                    icon_only=true
                    glyph=ICON_CHEVRON_FIRST
                    label="First move"
                    disabled={at_start}
                    @test_id={"game.history.first"}
                    on_click={move || steps.first()}
                />
                <Button
                    icon_only=true
                    glyph=ICON_CHEVRON_LEFT
                    label="Previous move"
                    disabled={at_start}
                    @test_id={"game.history.previous"}
                    on_click={move || steps.previous()}
                />
                <Button
                    icon_only=true
                    glyph=ICON_CHEVRON_RIGHT
                    label="Next move"
                    disabled={at_end}
                    @test_id={"game.history.next"}
                    on_click={move || steps.next()}
                />
                <Button
                    icon_only=true
                    glyph=ICON_CHEVRON_LAST
                    label="Latest move"
                    disabled={at_end}
                    @test_id={"game.history.last"}
                    on_click={move || steps.last()}
                />
            </List>
            <List direction=Direction::Horizontal spacing=Space::Xs2>
                <ListItem length=NUMBER_WIDTH>
                    <Text size=TextSize::Sm color={muted}>"#"</Text>
                </ListItem>
                <ForEach value={columns}>|name: String|
                    <ListItem grow=1.0>
                        <Text overflow=Overflow::Ellipsis size=TextSize::Sm color={muted}>
                            {name}
                        </Text>
                    </ListItem>
                </ForEach>
            </List>
            <List spacing=Space::Xs2>
                <ForEach
                    value={rows}
                    fallback={move || view! {
                        <Text style=TextStyle::CAPTION>"No moves yet"</Text>
                    }}
                >|row: Row, index: ReadSignal<usize>|
                    <MoveRow row index shown={shown} steps={steps} />
                </ForEach>
            </List>
            <Show value={ended}>
                <Result table={table} />
            </Show>
        </List>
    }
}

#[component]
fn MoveRow(row: Row, index: ReadSignal<usize>, shown: Memo<usize>, steps: Steps) -> NodeId {
    let number = create_memo(move || format!("{}", index.get() + 1));
    let muted = TextColor::Muted;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs2>
            <ListItem length=NUMBER_WIDTH>
                <Text size=TextSize::Sm color={muted}>{number}</Text>
            </ListItem>
            <ForEach value={row.cells}>|cell: Option<Cell>, column: ReadSignal<usize>|
                <ListItem grow=1.0>
                    <MoveCell row={index} column cell shown={shown} steps={steps} />
                </ListItem>
            </ForEach>
        </List>
    }
}

#[component]
fn MoveCell(
    row: ReadSignal<usize>,
    column: ReadSignal<usize>,
    cell: Option<Cell>,
    shown: Memo<usize>,
    steps: Steps,
) -> NodeId {
    let span = cell.as_ref().map(|cell| (cell.first, cell.last));
    let chosen = create_memo(move || {
        let shown = shown.get();
        span.is_some_and(|(first, last)| shown > first && shown <= last + 1)
    });
    let open = move || {
        if let Some((_, last)) = span {
            steps.show(last + 1);
        }
    };
    let label = cell.map(|cell| cell.text).unwrap_or_default();
    view! {
        <ListRow
            label={label}
            selected={chosen}
            @test_id={move || format!("game.history.{}.{}", row.get(), column.get())}
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
            <Show value={move || score.with(|score| !score.is_empty())}>
                <Text size=TextSize::Lg weight=Weight::Bold color={text} align=TextAlign::Center>
                    {score}
                </Text>
            </Show>
            <Text size=TextSize::Sm color={muted} align=TextAlign::Center>{description}</Text>
        </List>
    }
}
