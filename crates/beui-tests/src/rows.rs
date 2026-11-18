use crate::measured_rect;
use beui::primitives::Interactive;
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget, use_drop_site};
use beui::{Align, Direction, ScrollPosition};
use beui::{PointerPress, Pos2, Rect, Vec2};

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
                        <MenuItem
                            label="As PDF"
                            on_click={move || chosen.set("As PDF".to_owned())}
                        />
                    }}
                    on_click={move || chosen.set("Export".to_owned())}
                />
                <Progress value={Some(0.5_f32)} label="Working" />
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
                    <ForEach value={(1..=30).collect::<Vec<u32>>()}>|row: u32|
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

/// A virtual list in a scroll at row 40, whose rows above it, built by its overscan, grow:
/// the list keeps row 40 where it is shown, also where the browser does not anchor the scroll
/// itself.
#[component]
pub fn VirtualAnchorFixture() -> NodeId {
    const ROW: f32 = 20.0;
    let grown = create_signal(false);
    let anchor = create_signal(None::<Rect>);
    let above = create_signal(None::<Rect>);
    let keys: Vec<u32> = (0..200).collect();
    let scroll = view! {
        <Scroll offset={40.0 * ROW}>
            <VirtualList
                value={keys}
                item_size=ROW
                overscan={ROW * 20.0}
                anchor={Some(40_u32)}
            >|row: ReadSignal<u32>| {
                let built = view! {
                    <Frame
                        height={create_memo(move || {
                            let grows = grown.get() && (30..40).contains(&row.get());
                            Size::Custom(if grows { ROW * 3.0 } else { ROW })
                        })}
                        @test_id={create_memo(move || format!("anchor.row{}", row.get()))}
                    >
                        <Text>{create_memo(move || format!("Row {}", row.get()))}</Text>
                    </Frame>
                };
                let rect = measured_rect(built);
                create_effect(move || match row.get() {
                    40 => anchor.set(rect.get()),
                    39 => above.set(rect.get()),
                    _ => {}
                });
                built
            }</VirtualList>
        </Scroll>
    };
    let shown = measured_rect(scroll);
    let report = create_memo(move || match (above.get(), anchor.get(), shown.get()) {
        (Some(above), Some(anchor), Some(shown)) => format!(
            "Row 39 {:.0} tall, row 40 at {:.0}",
            above.height(),
            anchor.min.y - shown.min.y
        ),
        _ => "Unmeasured".to_owned(),
    });
    view! {
        <List spacing=Space::Md>
            <Frame width=Size::Custom(200.0) height=Size::Custom(200.0)>{scroll}</Frame>
            <Button label="Grow" @test_id="anchor.grow" on_click={move || grown.set(true)} />
            <Text>{report}</Text>
        </List>
    }
}

fn reach(at: ScrollPosition) -> &'static str {
    match at.offset >= at.max_offset() - 0.5 {
        true => "end",
        false if at.offset <= 0.5 => "start",
        false => "middle",
    }
}

#[component]
pub fn ScrollChainFixture() -> NodeId {
    let outer = create_signal("unknown");
    let inner = create_signal("unknown");
    let report =
        create_memo(move || format!("Outer at the {}, inner at the {}", outer.get(), inner.get()));
    view! {
        <List spacing=Space::Md>
            <Frame width=Size::Custom(240.0) height=Size::Custom(200.0)>
                <Scroll on_change={move |at: ScrollPosition| outer.set(reach(at))}>
                    <Text>"Above"</Text>
                    <Frame height=Size::Custom(120.0)>
                        <Scroll on_change={move |at: ScrollPosition| inner.set(reach(at))}>
                            <ForEach value={(1..=20).collect::<Vec<u32>>()}>|row: u32|
                                <Button
                                    label={format!("Inner {row}")}
                                    @test_id={format!("nest.inner{row}")}
                                />
                            </ForEach>
                        </Scroll>
                    </Frame>
                    <ForEach value={(1..=10).collect::<Vec<u32>>()}>|row: u32|
                        <Button
                            label={format!("Outer {row}")}
                            @test_id={format!("nest.outer{row}")}
                        />
                    </ForEach>
                </Scroll>
            </Frame>
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn DragFixture() -> NodeId {
    let dropped = create_signal(String::from("nothing"));
    let report = create_memo(move || format!("Dropped {}", dropped.get()));
    view! {
        <List spacing=Space::Xl>
            <Draggable
                payload={Some("apple".to_owned())}
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
            <Frame width=Size::Custom(200.0) height=Size::Custom(120.0)>
                <Scroll>
                    <Draggable
                        payload={Some("pear".to_owned())}
                        preview={|fruit: String| view! {
                            <Text>{fruit}</Text>
                        }}
                    >|_: DragHandle|
                        <Frame
                            height=Size::Custom(60.0)
                            fill=ColorRole::Surface
                            @test_id="drag.pear"
                        >
                            <Text>"Pear"</Text>
                        </Frame>
                    </Draggable>
                    <Frame height=Size::Custom(40.0) />
                    <DropTarget
                        on_drop={move |(fruit, _): (String, DragPoint)| dropped.set(fruit)}
                    >|_: DropHandle|
                        <Frame
                            height=Size::Custom(60.0)
                            fill=ColorRole::Surface
                            @test_id="drag.box"
                        >
                            <Text>"Box"</Text>
                        </Frame>
                    </DropTarget>
                </Scroll>
            </Frame>
            <Text>{report}</Text>
        </List>
    }
}

/// An item lifted by an `Interactive`'s `drag` and dropped on a site: the site shows what
/// is over it while carried and writes only at the release.
#[component]
pub fn CarryFixture() -> NodeId {
    let dropped = create_signal(String::from("nothing"));
    let basket = NodeRef::new();
    let site = use_drop_site(
        Func::new(|_: String| true),
        Func::new(move |_: String| {
            let rest = Rect::from_min_size(Pos2::ZERO, Vec2::new(40.0, 20.0));
            beui::drag::fixed(beui::drag::Candidate::new((), rest, basket))
        }),
        Callback::new(move |(fruit, ()): (String, ())| dropped.set(fruit)),
    );
    let report = create_memo(move || match site.over.get() {
        Some(()) => "Over basket".to_owned(),
        None => format!("Dropped {}", dropped.get()),
    });
    view! {
        <List spacing=Space::Xl>
            <Interactive
                drag={Func::new(|_: PointerPress| Some(beui::drag::Carry::new("apple".to_owned())))}
            >
                <Card @test_id="carry.apple">
                    <Text>"Apple"</Text>
                </Card>
            </Interactive>
            <Frame
                @node_ref=&basket
                width=Size::Custom(200.0)
                height=Size::Custom(80.0)
                fill=ColorRole::Surface
                @test_id="carry.basket"
            >
                <Text>"Basket"</Text>
            </Frame>
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
                    value={fruits}
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
                        <ForEach value={reorder.order} handle={reorder.rows}>|fruit: &'static str|
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
                <LongList />
            </List>
            <Text>{shelved}</Text>
            <Text>{closing}</Text>
        </List>
    }
}

#[component]
fn LongList() -> NodeId {
    let rows = create_signal((1..=12).collect::<Vec<u32>>());
    let reached = create_signal("start");
    let report = create_memo(move || {
        let below = rows.with(|rows| rows.iter().position(|row| *row == 1).unwrap_or(0) >= 8);
        format!(
            "Long list at the {}, row 1 {}",
            reached.get(),
            if below {
                "below the fold"
            } else {
                "above the fold"
            }
        )
    });
    view! {
        <List spacing=Space::Xs>
            <Frame width=Size::Custom(120.0) height=Size::Custom(160.0) @test_id="long.view">
                <Scroll on_change={move |at: ScrollPosition| reached.set(reach(at))}>
                    <Reorder
                        value={rows}
                        payload={Func::new(|row: u32| Some(row))}
                        text={|row: u32| format!("Row {row}")}
                        preview={|row: u32| view! {
                            <Frame
                                width=Size::Custom(120.0)
                                height=Size::Custom(SHELF_ROW)
                                fill=ColorRole::Accent
                            >
                                <Text>{format!("Row {row}")}</Text>
                            </Frame>
                        }}
                        on_move={move |moved: ReorderMove<u32, u32>| {
                            rows.update(|rows| moved_to(rows, moved.payload, moved.after));
                        }}
                    >|reorder: ReorderHandle<u32, u32>|
                        <List spacing=SHELF_GAP>
                            <ForEach value={reorder.order} handle={reorder.rows}>|row: u32|
                                <ReorderItem reorder item=row>|_: ReorderItemHandle|
                                    <Frame
                                        height=Size::Custom(SHELF_ROW)
                                        fill=ColorRole::Surface
                                        @test_id={format!("long.{row}")}
                                    >
                                        <Text>{format!("Row {row}")}</Text>
                                    </Frame>
                                </ReorderItem>
                            </ForEach>
                        </List>
                    </Reorder>
                </Scroll>
            </Frame>
            <Text>{report}</Text>
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
                value={keys}
                payload={Func::new(|vegetable: Vegetable| Some(vegetable))}
                accepts={Func::new(|_: Vegetable| true)}
                preview={|vegetable: Vegetable| view! {
                    <Frame
                        width=Size::Custom(120.0)
                        height=Size::Custom(SHELF_ROW)
                        fill=ColorRole::Accent
                        @test_id="shelf.preview"
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
                    <ForEach value={reorder.order} handle={reorder.rows}>|vegetable: Vegetable|
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
