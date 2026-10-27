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
