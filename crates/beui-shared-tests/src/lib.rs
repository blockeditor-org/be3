use beui::primitives::*;
use beui::reactive::*;
use beui::{Align, Align2, Color32, Corners, Direction, Extent, KeyPress, Rect, Sides};

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(test)]
mod tests;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub use native::FixtureApp;

const PAPER: Color32 = Color32::from_rgb(250, 250, 247);
const INK: Color32 = Color32::from_rgb(30, 30, 40);
const BLUE: Color32 = Color32::from_rgb(70, 110, 220);
const GREEN: Color32 = Color32::from_rgb(60, 170, 110);
const AMBER: Color32 = Color32::from_rgb(235, 170, 50);

fn size(rect: Option<Rect>) -> String {
    match rect {
        Some(rect) => format!("{:.0}x{:.0}", rect.width(), rect.height()),
        None => "unmeasured".to_owned(),
    }
}

#[component]
pub fn Fixture(name: String) -> Root {
    let screen = match name.as_str() {
        "frame" => view! {
            <FrameFixture />
        },
        "list" => view! {
            <ListFixture />
        },
        "text" => view! {
            <TextFixture />
        },
        "focus" => view! {
            <FocusFixture />
        },
        "interactive" => view! {
            <InteractiveFixture />
        },
        other => {
            let message = format!("No fixture is named {other}");
            view! {
                <Text color={INK}>{message}</Text>
            }
        }
    };
    view! {
        <Root title="beui shared tests" clear_color={PAPER}>
            <Frame align={Align2::START} padding={Sides::all(16.0)}>{screen}</Frame>
        </Root>
    }
}

#[component]
fn Button(id: &'static str, label: &'static str, on_click: ClickCallback) -> NodeId {
    view! {
        <Interactive @test_id={id} focusable=true on_click={on_click}>
            <Frame fill={AMBER} padding={Sides::symmetric(12.0, 6.0)} radius={Corners::all(4.0)}>
                <Text color={INK}>{label}</Text>
            </Frame>
        </Interactive>
    }
}

#[component]
fn FrameFixture() -> NodeId {
    let wide = create_signal(false);
    let width = create_memo(move || Extent::px(if wide.get() { 100.0 } else { 50.0 }));
    let inner = view! {
        <Frame width={width} height=20.0 fill={GREEN} />
    };
    let outer = view! {
        <Frame
            @test_id="frame.outer"
            width=200.0
            height=80.0
            padding={Sides::all(10.0)}
            align={Align2::all(Align::End)}
            fill={BLUE}
        >
            {inner}
        </Frame>
    };
    let inner_rect = measure(inner);
    let outer_rect = measure(outer);
    let inner_rect = create_memo(move || inner_rect.get().map(|measured| measured.rect));
    let outer_rect = create_memo(move || outer_rect.get().map(|measured| measured.rect));
    let report = create_memo(move || match (inner_rect.get(), outer_rect.get()) {
        (Some(inner), Some(outer)) => format!(
            "Inner {} at {:.0},{:.0}",
            size(Some(inner)),
            inner.min.x - outer.min.x,
            inner.min.y - outer.min.y
        ),
        _ => "Inner unmeasured".to_owned(),
    });
    view! {
        <List spacing=12.0>
            {outer}
            <Text color={INK}>{report}</Text>
            <Button
                id="frame.grow"
                label="Grow"
                on_click={move || wide.set(!wide.get_untracked())}
            />
        </List>
    }
}

#[component]
fn ListFixture() -> NodeId {
    let rows = create_signal(vec![1_u32, 2, 3]);
    let list = view! {
        <List @test_id="list.rows" spacing=10.0>
            <ForEach keys={rows}>|row: u32|
                <Frame width=160.0 height=20.0 fill={GREEN} @test_id={format!("list.row{row}")} />
            </ForEach>
        </List>
    };
    let measured = measure(list);
    let report = create_memo(move || {
        format!(
            "Rows {} in {}",
            rows.with(Vec::len),
            size(measured.get().map(|measured| measured.rect))
        )
    });
    view! {
        <List spacing=12.0>
            {list}
            <Text color={INK}>{report}</Text>
            <Button
                id="list.add"
                label="Add a row"
                on_click={move || rows.update(|rows| rows.push(rows.len() as u32 + 1))}
            />
            <Button
                id="list.reverse"
                label="Reverse"
                on_click={move || rows.update(|rows| rows.reverse())}
            />
        </List>
    }
}

#[component]
fn TextFixture() -> NodeId {
    let last = create_signal("italic".to_owned());
    let count = create_signal(0_u32);
    let counted = create_memo(move || format!("Count {}", count.get()));
    view! {
        <List spacing=12.0>
            <Text color={INK} @test_id="text.styled">
                "Plain, "
                <Span bold={Some(true)}>"bold"</Span>
                " and "
                <Span italic={Some(true)}>{last}</Span>
            </Text>
            <Text color={INK}>{counted}</Text>
            <Button
                id="text.change"
                label="Change"
                on_click={move || {
                    last.set("underlined".to_owned());
                    count.set(count.get_untracked() + 1);
                }}
            />
        </List>
    }
}

#[component]
fn FocusFixture() -> NodeId {
    let focused = create_signal("none");
    let focus_on = move |name: &'static str| {
        move |now: bool| {
            if now {
                focused.set(name);
            } else if focused.get_untracked() == name {
                focused.set("none");
            }
        }
    };
    let report = create_memo(move || format!("Focused {}", focused.get()));
    view! {
        <List spacing=12.0>
            <List direction={Direction::Horizontal} spacing=12.0>
                <Interactive
                    @test_id="focus.first"
                    focusable=true
                    on_focus_change={focus_on("first")}
                >
                    <Frame width=80.0 height=40.0 fill={BLUE} />
                </Interactive>
                <Interactive
                    @test_id="focus.second"
                    focusable=true
                    on_focus_change={focus_on("second")}
                >
                    <Frame width=80.0 height=40.0 fill={GREEN} />
                </Interactive>
            </List>
            <Text color={INK}>{report}</Text>
        </List>
    }
}

#[component]
fn InteractiveFixture() -> NodeId {
    let clicks = create_signal(0_u32);
    let pressed = create_signal(String::from("nothing"));
    let typed = create_signal(String::new());
    let clicked = create_memo(move || format!("Clicked {}", clicks.get()));
    let keys = create_memo(move || format!("Pressed {}", pressed.get()));
    let text = create_memo(move || format!("Typed {}", typed.get()));
    view! {
        <List spacing=12.0>
            <Button
                id="interactive.count"
                label="Count"
                on_click={move || clicks.set(clicks.get_untracked() + 1)}
            />
            <Text color={INK}>{clicked}</Text>
            <Interactive
                @test_id="interactive.keys"
                focusable=true
                on_key={move |press: KeyPress| {
                    if press.pressed {
                        pressed.set(format!("{:?}", press.key));
                    }
                    false
                }}
            >
                <Frame width=160.0 height=30.0 fill={BLUE} />
            </Interactive>
            <Text color={INK}>{keys}</Text>
            <Interactive
                @test_id="interactive.typing"
                focusable=true
                ime=true
                on_text={move |text: String| typed.update(|typed| typed.push_str(&text))}
            >
                <Frame width=160.0 height=30.0 fill={GREEN} />
            </Interactive>
            <Text color={INK}>{text}</Text>
        </List>
    }
}
