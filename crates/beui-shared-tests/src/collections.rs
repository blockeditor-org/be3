use beui::datetime::{Date, DateTime, Time};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::DateTimeParts;

const NODES: [(&str, usize); 5] = [
    ("src", 0),
    ("lib.rs", 1),
    ("main.rs", 1),
    ("docs", 0),
    ("guide.md", 1),
];

fn has_children(row: usize) -> bool {
    NODES
        .get(row + 1)
        .is_some_and(|(_, depth)| *depth > NODES[row].1)
}

fn visible_rows(collapsed: &[usize]) -> Vec<usize> {
    let mut rows = Vec::new();
    let mut hidden_below: Option<usize> = None;
    for (row, (_, depth)) in NODES.iter().enumerate() {
        match hidden_below {
            Some(limit) if *depth > limit => continue,
            _ => hidden_below = None,
        }
        rows.push(row);
        if collapsed.contains(&row) {
            hidden_below = Some(*depth);
        }
    }
    rows
}

#[component]
pub fn TreeFixture() -> NodeId {
    let collapsed = create_signal(Vec::<usize>::new());
    let selected = create_signal(None::<usize>);
    let keys = create_memo(move || visible_rows(&collapsed.get()));
    let report = create_memo(move || {
        format!(
            "Selected {} rows {}",
            selected.get().map_or("nothing", |row| NODES[row].0),
            keys.with(Vec::len)
        )
    });
    view! {
        <List spacing=Space::Md>
            <Frame width=Size::Custom(240.0) height=Size::Custom(200.0)>
                <Tree
                    keys
                    item={move |row: usize| TreeItem {
                        label: NODES[row].0.to_owned(),
                        depth: NODES[row].1,
                        expandable: has_children(row),
                        expanded: !collapsed.with_untracked(|collapsed| collapsed.contains(&row)),
                    }}
                    selected
                    row_test_id={|row: usize| format!("tree.{}", NODES[row].0)}
                    menu={|_: usize| Some(TreeMenu {
                        items: view! {
                            <MenuItem label="Rename" />
                            <MenuItem label="Delete" />
                        },
                    })}
                    on_select={move |row: usize| selected.set(Some(row))}
                    on_expand={move |(row, expanded): (usize, bool)| {
                        collapsed.update(|collapsed| {
                            collapsed.retain(|candidate| *candidate != row);
                            if !expanded {
                                collapsed.push(row);
                            }
                        });
                    }}
                >|row: TreeRowFace<usize>|
                    <Text>{NODES[row.key].0}</Text>
                </Tree>
            </Frame>
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn TableFixture() -> NodeId {
    let about = create_signal(false);
    let report = create_memo(move || format!("About open {}", about.get()));
    view! {
        <List spacing=Space::Md>
            <Accordion title="About" open={about}>
                <Text>"Retained nodes, composed."</Text>
            </Accordion>
            <Text>{report}</Text>
            <Table
                label="Planets"
                heading="Planet"
                columns={vec![TableColumn::new("Year"), TableColumn::new("Moons")]}
            >
                <TableRow label="Earth" @test_id="table.earth">
                    <TableCell text="365 days" @test_id="table.earth.year" />
                    <TableCell text="1" @test_id="table.earth.moons" />
                </TableRow>
                <TableRow label="Mars" @test_id="table.mars">
                    <TableCell text="687 days" @test_id="table.mars.year" />
                    <TableCell text="2" @test_id="table.mars.moons" />
                </TableRow>
            </Table>
        </List>
    }
}

#[component]
pub fn DatesFixture() -> NodeId {
    let today = Date::new(2026, 3, 10);
    let day = create_signal(Some(DateTime::new(today, Time::MIDNIGHT)));
    let booked = create_signal(None::<Date>);
    let report = create_memo(move || {
        format!(
            "Day {} booked {}",
            day.get().map_or("none".to_owned(), |day| day.date.label()),
            booked.get().map_or("none".to_owned(), Date::label)
        )
    });
    view! {
        <beui::unstyled::Container>|_|
            <DatesScreen day booked report />
        </beui::unstyled::Container>
    }
}

#[component]
fn DatesScreen(
    day: Signal<Option<DateTime>>,
    booked: Signal<Option<Date>>,
    report: Memo<String>,
) -> NodeId {
    let today = Date::new(2026, 3, 10);
    view! {
        <List spacing=Space::Md>
            <DateTimeField value={day} parts=DateTimeParts::Date label="Day" today={Some(today)} />
            <Calendar selected={booked} today={Some(today)} show={Some(today)} />
            <Text>{report}</Text>
        </List>
    }
}
