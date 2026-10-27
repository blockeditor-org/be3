use crate::measured_rect;
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};
use beui::{Align, Direction, ScrollPosition};

#[component]
pub fn RowsFixture() -> NodeId {
    let chosen = create_signal(String::from("nothing"));
    let banner = create_signal(Some(String::from("Saving failed")));
    let report = create_memo(move || format!("Chose {}", chosen.get()));
    view! {
        <List spacing=Space::Md>
            <Banner message={banner} on_dismiss={move || banner.set(None)} />
            <Frame width=Size::Custom(320.0)>
                <List spacing=Space::Xs2>
                    <ListRow
                        label="Wi-Fi"
                        detail="Home network"
                        trailing=RowTrailing::Chevron
                        on_click={move || chosen.set("Wi-Fi".to_owned())}
                    />
                    <ActionRow
                        label="Sign out"
                        on_click={move || chosen.set("Sign out".to_owned())}
                    />
                </List>
            </Frame>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Link label="Read more" on_click={move || chosen.set("the link".to_owned())} />
                <SplitButton
                    label="Export"
                    items={view! {
                        <MenuItem label="As PDF" />
                    }}
                    on_click={move || chosen.set("Export".to_owned())}
                    on_select={move |_: Vec<usize>| chosen.set("As PDF".to_owned())}
                />
                <Progress value={create_memo(|| Some(0.5_f32))} label="Working" />
            </List>
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn ScrollFixture() -> NodeId {
    let position = create_signal(String::from("unknown"));
    view! {
        <List spacing=Space::Md>
            <Frame width=Size::Custom(200.0) height=Size::Custom(120.0)>
                <Scroll
                    on_change={move |at: ScrollPosition| {
                        position.set(match at.offset >= at.max_offset() - 0.5 {
                            true => "end".to_owned(),
                            false if at.offset <= 0.5 => "start".to_owned(),
                            false => "middle".to_owned(),
                        })
                    }}
                >
                    <ForEach keys={(1..=30).collect::<Vec<u32>>()}>|row: u32|
                        <Button
                            label={format!("Item {row}")}
                            @test_id={format!("scroll.item{row}")}
                        />
                    </ForEach>
                </Scroll>
            </Frame>
            <Text>{create_memo(move || format!("Scrolled to the {}", position.get()))}</Text>
        </List>
    }
}

#[component]
pub fn DragFixture() -> NodeId {
    let dropped = create_signal(String::from("nothing"));
    let report = create_memo(move || format!("Dropped {}", dropped.get()));
    let apple = create_memo(|| Some("apple".to_owned()));
    view! {
        <List spacing=Space::Xl>
            <Draggable
                payload={apple}
                preview={|fruit: String| view! {
                    <Text>{fruit}</Text>
                }}
            >|_: DragHandle|
                <Card @test_id="drag.apple">
                    <Text>"Apple"</Text>
                </Card>
            </Draggable>
            <DropTarget
                on_drop={move |(fruit, _): (String, DragPoint)| dropped.set(fruit)}
            >|_: DropHandle|
                <Frame
                    width=Size::Custom(200.0)
                    height=Size::Custom(80.0)
                    fill=ColorRole::Surface
                    @test_id="drag.basket"
                >
                    <Text>"Basket"</Text>
                </Frame>
            </DropTarget>
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn ReorderFixture() -> NodeId {
    let fruits = create_signal(vec!["apple", "banana", "cherry", "date"]);
    let report = create_memo(move || format!("Order: {}", fruits.get().join(" ")));
    let shelves = create_signal(vec![vec!["leek", "kale", "okra"], vec!["corn"]]);
    let shelved = create_memo(move || {
        shelves.with(|shelves| {
            format!(
                "Shelves: {} | {}",
                shelves[0].join(" "),
                shelves[1].join(" ")
            )
        })
    });
    let first = view! {
        <Shelf shelf=0 shelves />
    };
    let height = measured_rect(first);
    let closing = create_memo(move || {
        let length = height.get().map_or(0.0, |rect| rect.height());
        let spacing = SHELF_GAP.px();
        let full = 3.0 * SHELF_ROW + 2.0 * spacing;
        let closed = 2.0 * SHELF_ROW + spacing;
        match length {
            length if length >= full - 0.5 => "Shelf full".to_owned(),
            length if length <= closed + 0.5 => "Shelf closed".to_owned(),
            _ => "Shelf closing".to_owned(),
        }
    });
    view! {
        <List spacing=Space::Xl>
            <Frame width=Size::Custom(240.0)>
                <Reorder
                    keys={fruits}
                    payload={Func::new(|fruit: &'static str| Some(fruit))}
                    text={|fruit: &'static str| fruit.to_owned()}
                    preview={|fruit: &'static str| view! {
                        <Card>
                            <Text>{fruit}</Text>
                        </Card>
                    }}
                    on_move={move |moved: ReorderMove<&'static str, &'static str>| {
                        fruits.update(|fruits| moved_to(fruits, moved.payload, moved.after));
                    }}
                >|reorder: ReorderHandle<&'static str, &'static str>|
                    <List spacing=Space::Xs>
                        <ForEach keys={reorder.order}>|fruit: &'static str|
                            <ReorderItem reorder item=fruit>|_: ReorderItemHandle|
                                <Card @test_id={format!("reorder.{fruit}")}>
                                    <Text>{fruit}</Text>
                                </Card>
                            </ReorderItem>
                        </ForEach>
                    </List>
                </Reorder>
            </Frame>
            <Text @test_id="reorder.report">{report}</Text>
            <List direction=Direction::Horizontal spacing=Space::Xl align=Align::Start>
                {first}
                <Shelf shelf=1 shelves />
            </List>
            <Text>{shelved}</Text>
            <Text>{closing}</Text>
        </List>
    }
}

const SHELF_ROW: f32 = 30.0;
const SHELF_GAP: Space = Space::Xs;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Vegetable(&'static str);

fn moved_to<K: PartialEq>(keys: &mut Vec<K>, moved: K, after: Option<K>) {
    keys.retain(|key| *key != moved);
    let at = after
        .and_then(|after| keys.iter().position(|key| *key == after))
        .map_or(0, |at| at + 1);
    keys.insert(at, moved);
}

#[component]
fn Shelf(shelf: usize, shelves: Signal<Vec<Vec<&'static str>>>) -> NodeId {
    let keys = create_memo(move || {
        shelves.with(|shelves| {
            shelves[shelf]
                .iter()
                .copied()
                .map(Vegetable)
                .collect::<Vec<_>>()
        })
    });
    view! {
        <Frame width=Size::Custom(120.0) @test_id={format!("shelf.{shelf}")}>
            <Reorder
                keys={keys}
                payload={Func::new(|vegetable: Vegetable| Some(vegetable))}
                accepts={Func::new(|_: Vegetable| true)}
                preview={|vegetable: Vegetable| view! {
                    <Frame
                        width=Size::Custom(120.0)
                        height=Size::Custom(SHELF_ROW)
                        fill=ColorRole::Accent
                    >
                        <Text>{vegetable.0}</Text>
                    </Frame>
                }}
                on_move={move |moved: ReorderMove<Vegetable, Vegetable>| shelves.update(|shelves| {
                    for list in shelves.iter_mut() {
                        list.retain(|vegetable| *vegetable != moved.payload.0);
                    }
                    let mut list: Vec<Vegetable> = shelves[shelf].iter().copied().map(Vegetable).collect();
                    moved_to(&mut list, moved.payload, moved.after);
                    shelves[shelf] = list.into_iter().map(|vegetable| vegetable.0).collect();
                })}
            >|reorder: ReorderHandle<Vegetable, Vegetable>|
                <List spacing=SHELF_GAP>
                    <ForEach keys={reorder.order}>|vegetable: Vegetable|
                        <ReorderItem reorder item=vegetable>|_: ReorderItemHandle|
                            <Frame
                                height=Size::Custom(SHELF_ROW)
                                fill=ColorRole::Surface
                                @test_id={format!("shelf.{}", vegetable.0)}
                            >
                                <Text>{vegetable.0}</Text>
                            </Frame>
                        </ReorderItem>
                    </ForEach>
                </List>
            </Reorder>
        </Frame>
    }
}
