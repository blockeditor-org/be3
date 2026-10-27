use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::TimeOptionHandle;
use beui::datetime::Time;
use beui::{Align, Direction, Rect};

fn placed(node: NodeId) -> Memo<Option<Rect>> {
    let measured = measure(node);
    create_memo(move || measured.get().map(|measured| measured.rect))
}

#[component]
pub fn PanZoomFixture() -> NodeId {
    let view = create_signal(PanZoomView::IDENTITY);
    let report = create_memo(move || {
        let view = view.get();
        format!(
            "Center {:.0},{:.0} scale {:.2}",
            view.center.x, view.center.y, view.scale
        )
    });
    view! {
        <List spacing=Space::Md>
            <Frame width=Size::Custom(300.0) height=Size::Custom(200.0) fill=ColorRole::Surface>
                <PanZoom view @test_id="pan_zoom.stage">|handle: PanZoomHandle| {
                    let focused = handle.focused;
                    let state = create_memo(move || match focused.get() {
                        true => "Stage focused".to_owned(),
                        false => "Stage idle".to_owned(),
                    });
                    view! {
                        <Frame width=Size::Custom(300.0) height=Size::Custom(200.0)>
                            <Text>{state}</Text>
                        </Frame>
                    }
                }</PanZoom>
            </Frame>
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn TimeListFixture() -> NodeId {
    let chosen = create_signal(None::<Time>);
    let report = create_memo(move || match chosen.get() {
        Some(time) => format!("Chose {}", time.format(beui::datetime::HourCycle::H24)),
        None => "Chose nothing".to_owned(),
    });
    view! {
        <List spacing=Space::Md>
            <Frame width=Size::Custom(160.0) height=Size::Custom(200.0)>
                <beui::unstyled::TimeList
                    value={chosen}
                    step_minutes=30
                    row_height=28.0
                    label="Start"
                    option={|option: TimeOptionHandle| {
                        let TimeOptionHandle { label, selected, focused, .. } = option;
                        let fill = create_memo(move || match (selected.get(), focused.get()) {
                            (true, _) => ColorRole::Accent,
                            (false, true) => ColorRole::Hover,
                            (false, false) => ColorRole::Surface,
                        });
                        view! {
                            <Frame height=Size::Custom(28.0) fill>
                                <Text>{label}</Text>
                            </Frame>
                        }
                    }}
                />
            </Frame>
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn BackSlideFixture() -> NodeId {
    let open = create_signal(true);
    let gone = create_signal(0_u32);
    let report = create_memo(move || format!("Open {} gone {}", open.get(), gone.get()));
    view! {
        <List spacing=Space::Md>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button label="Close" on_click={move || open.set(false)} />
                <Button label="Reopen" on_click={move || open.set(true)} />
            </List>
            <Frame width=Size::Custom(300.0) height=Size::Custom(120.0)>
                <BackSlide
                    open={open}
                    enters=false
                    on_gone={move || gone.update(|gone| *gone += 1)}
                >
                    <Card @test_id="back_slide.page">
                        <Text>"A folder"</Text>
                    </Card>
                </BackSlide>
            </Frame>
            <Text>{report}</Text>
        </List>
    }
}

fn edge_name(edge: Edge) -> &'static str {
    match edge {
        Edge::Top => "top",
        Edge::TopEnd => "top end",
        Edge::Bottom => "bottom",
        Edge::BottomEnd => "bottom end",
    }
}

#[component]
pub fn FloatingFixture() -> NodeId {
    let anchor = NodeRef::new();
    let edge = create_signal(Edge::Bottom);
    let open = create_signal(true);
    let pressed = create_signal(0_u32);
    let area = view! {
        <Frame
            @node_ref=&anchor
            @test_id="floating.area"
            width=Size::Custom(320.0)
            height=Size::Custom(200.0)
            fill=ColorRole::Surface
        />
    };
    let area_rect = placed(area);
    let note_rect = create_signal(None::<Rect>);
    let report = create_memo(move || {
        let place = match (note_rect.get(), area_rect.get()) {
            (Some(note), Some(area)) => {
                let vertical = match (note.min.y - area.min.y).abs() < 1.0 {
                    true => "top",
                    false if (note.max.y - area.max.y).abs() < 1.0 => "bottom",
                    false => "elsewhere",
                };
                let horizontal = match (note.max.x - area.max.x).abs() < 1.0 {
                    true => " end",
                    false => "",
                };
                format!("{vertical}{horizontal}")
            }
            _ => "unmeasured".to_owned(),
        };
        format!(
            "Asked {} placed {place} pressed {}",
            edge_name(edge.get()),
            pressed.get()
        )
    });
    view! {
        <List spacing=Space::Md>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button label="Top" on_click={move || edge.set(Edge::Top)} />
                <Button label="Bottom end" on_click={move || edge.set(Edge::BottomEnd)} />
                <Button label="Hide" on_click={move || open.set(false)} />
            </List>
            {area}
            <Floating anchor edge={edge} open={open}>|| {
                let note = view! {
                    <Card @test_id="floating.note">
                        <Button label="Press" on_click={move || pressed.update(|pressed| *pressed += 1)} />
                    </Card>
                };
                let measured = placed(note);
                create_effect(move || note_rect.set(measured.get()));
                note
            }</Floating>
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn StackFixture() -> NodeId {
    let wide = create_signal(true);
    let width = create_memo(move || match wide.get() {
        true => Length::Size(Size::Custom(900.0)),
        false => Length::Size(Size::Custom(400.0)),
    });
    view! {
        <List spacing=Space::Md>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button label="Narrow" on_click={move || wide.set(false)} />
                <Button label="Wide" on_click={move || wide.set(true)} />
            </List>
            <Frame width>
                <beui::unstyled::Container>|_|
                    <StackBody />
                </beui::unstyled::Container>
            </Frame>
        </List>
    }
}

#[component]
fn StackBody() -> NodeId {
    let first = view! {
        <Card>
            <Text>"First"</Text>
        </Card>
    };
    let second = view! {
        <Card>
            <Text>"Second"</Text>
        </Card>
    };
    let (first_rect, second_rect) = (placed(first), placed(second));
    let report = create_memo(move || match (first_rect.get(), second_rect.get()) {
        (Some(first), Some(second)) if second.min.y >= first.max.y => "Second below".to_owned(),
        (Some(first), Some(second)) if second.min.x >= first.max.x => "Second beside".to_owned(),
        (Some(_), Some(_)) => "Second overlaps".to_owned(),
        _ => "Second unmeasured".to_owned(),
    });
    view! {
        <List spacing=Space::Md>
            <Stack spacing=Space::Md>
                {first}
                {second}
            </Stack>
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn PointerLockFixture() -> NodeId {
    let locked = create_signal(false);
    let report = create_memo(move || format!("Locked {}", locked.get()));
    view! {
        <List spacing=Space::Md>
            <PointerLock locked @test_id="pointer_lock.area">|handle: PointerLockHandle| {
                let label = create_memo(move || match handle.locked.get() {
                    true => "Press Escape to let go".to_owned(),
                    false => "Click to look around".to_owned(),
                });
                view! {
                    <Frame width=Size::Custom(240.0) height=Size::Custom(120.0) fill=ColorRole::Surface>
                        <Text>{label}</Text>
                    </Frame>
                }
            }</PointerLock>
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn ActionBarFixture() -> NodeId {
    let ran = create_signal(String::from("nothing"));
    let action = move |id: &str, label: &'static str| {
        Action::new(id, label, move || ran.set(label.to_owned()))
    };
    let undo = action("bar.undo", "Undo")
        .glyph(beui::icons::ICON_UNDO_2)
        .pin(Pin::Icon)
        .detached();
    let share = action("bar.share", "Share").pin(Pin::Label).detached();
    let rename = action("bar.rename", "Rename").detached();
    let menu = vec![
        MenuNode::action(&undo),
        MenuNode::action(&share),
        MenuNode::Separator,
        MenuNode::action(&rename),
    ];
    let report = create_memo(move || format!("Ran {}", ran.get()));
    view! {
        <List spacing=Space::Md>
            <ActionBar menu test_prefix={"bar".to_owned()} />
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn ProgressFixture() -> NodeId {
    let done = create_signal(false);
    let value = create_memo(move || done.get().then_some(1.0_f32));
    let report = create_memo(move || match done.get() {
        true => "Upload done".to_owned(),
        false => "Uploading".to_owned(),
    });
    view! {
        <List spacing=Space::Md>
            <Frame width=Size::Custom(240.0)>
                <Progress value label="Upload" />
            </Frame>
            <Button label="Finish" on_click={move || done.set(true)} />
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn SheetFixture() -> NodeId {
    let open = create_signal(false);
    let closed = create_signal(0_u32);
    let report = create_memo(move || format!("Sheet open {} closed {}", open.get(), closed.get()));
    view! {
        <List spacing=Space::Md>
            <Button label="Pick" on_click={move || open.set(true)} />
            <Text>{report}</Text>
            <ModalSheet
                open={open}
                rest=0.5
                on_close={move || {
                    closed.update(|closed| *closed += 1);
                    open.set(false);
                }}
            >
                <List spacing=Space::None>
                    <ForEach keys={(1..=3).collect::<Vec<usize>>()}>|row: usize|
                        <ActionRow label={format!("Row {row}")} on_click={move || open.set(false)} />
                    </ForEach>
                </List>
            </ModalSheet>
        </List>
    }
}

#[component]
pub fn PictureFixture() -> NodeId {
    let image = beui::Image::from_rgba(32, 32, vec![200; 32 * 32 * 4]);
    let picture = view! {
        <Picture image={Some(image)} />
    };
    let size = placed(picture);
    let report = create_memo(move || format!("Picture {}", crate::size(size.get())));
    view! {
        <List spacing=Space::Md>
            <Frame width=Size::Custom(20.0) height=Size::Custom(20.0)>{picture}</Frame>
            <Text>{report}</Text>
        </List>
    }
}
