use beui::primitives::*;
use beui::reactive::*;
use beui::{
    Align, Align2, CanvasView, Color32, Corners, Direction, Dismiss, Extent, FontId, Key, KeyPress,
    Placement, PointerInput, PointerStage, Rect, ScrollPosition, Sides, TextCaret, TextGlyph,
    TextMark, Vec2,
};

mod collections;
mod controls;
mod layers;
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

fn measured_rect(node: NodeId) -> Memo<Option<Rect>> {
    let measured = measure(node);
    create_memo(move || measured.get().map(|measured| measured.rect))
}

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
        "offset" => view! {
            <OffsetFixture />
        },
        "virtual_list" => view! {
            <VirtualListFixture />
        },
        "canvas" => view! {
            <CanvasFixture />
        },
        "overlay" => view! {
            <OverlayFixture />
        },
        "portal" => view! {
            <PortalFixture />
        },
        "editing" => view! {
            <EditingFixture />
        },
        "text_input" => view! {
            <TextInputFixture />
        },
        "buttons" => view! {
            <controls::ButtonsFixture />
        },
        "toggles" => view! {
            <controls::TogglesFixture />
        },
        "choices" => view! {
            <controls::ChoicesFixture />
        },
        "menus" => view! {
            <controls::MenusFixture />
        },
        "popups" => view! {
            <controls::PopupsFixture />
        },
        "fields" => view! {
            <controls::FieldsFixture />
        },
        "dialogs" => view! {
            <layers::DialogsFixture />
        },
        "toasts" => view! {
            <layers::ToastsFixture />
        },
        "palette" => view! {
            <layers::PaletteFixture />
        },
        "tree" => view! {
            <collections::TreeFixture />
        },
        "table" => view! {
            <collections::TableFixture />
        },
        "dates" => view! {
            <collections::DatesFixture />
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
    let inner_rect = measured_rect(inner);
    let outer_rect = measured_rect(outer);
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
    let measured = measured_rect(list);
    let report =
        create_memo(move || format!("Rows {} in {}", rows.with(Vec::len), size(measured.get())));
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

#[component]
fn OffsetFixture() -> NodeId {
    let rows = create_signal(10_u32);
    let offset = create_signal(0.0_f32);
    let position = create_signal(None::<ScrollPosition>);
    let report = create_memo(move || match position.get() {
        Some(position) => format!(
            "Offset {:.0} of {:.0} in {:.0}",
            position.offset, position.content, position.viewport
        ),
        None => "Offset unknown".to_owned(),
    });
    let keys = create_memo(move || (0..rows.get()).collect::<Vec<u32>>());
    view! {
        <List spacing=12.0>
            <Frame width=160.0 height=100.0>
                <Offset
                    offset={offset}
                    follow_end=true
                    on_change={move |now: ScrollPosition| position.set(Some(now))}
                >
                    <List spacing=0.0>
                        <ForEach keys={keys}>|row: u32|
                            <Frame
                                height=30.0
                                fill={if row % 2 == 0 { BLUE } else { GREEN }}
                                @test_id={format!("offset.row{row}")}
                            />
                        </ForEach>
                    </List>
                </Offset>
            </Frame>
            <Text color={INK}>{report}</Text>
            <Button
                id="offset.add"
                label="Add a row"
                on_click={move || rows.set(rows.get_untracked() + 1)}
            />
            <Button id="offset.near_top" label="Near the top" on_click={move || offset.set(10.0)} />
        </List>
    }
}

#[component]
fn VirtualListFixture() -> NodeId {
    let offset = create_signal(0.0_f32);
    let keys: Vec<u32> = (0..1000).collect();
    view! {
        <List spacing=12.0>
            <Frame width=160.0 height=200.0>
                <Offset offset={offset}>
                    <VirtualList keys={keys} item_size=20.0>|row: ReadSignal<u32>|
                        <Frame
                            height=30.0
                            fill={if row.get_untracked() % 2 == 0 { BLUE } else { GREEN }}
                            @test_id={format!("rows.row{}", row.get_untracked())}
                        >
                            <Text color={PAPER}>{format!("Row {}", row.get_untracked())}</Text>
                        </Frame>
                    </VirtualList>
                </Offset>
            </Frame>
            <Button id="rows.end" label="To the end" on_click={move || offset.set(29800.0)} />
        </List>
    }
}

#[component]
fn CanvasFixture() -> NodeId {
    let pan = create_signal(Vec2::ZERO);
    let scale = create_signal(1.0_f32);
    let canvas = NodeRef::new();
    let placed = create_signal(None::<Rect>);
    let view = create_memo(move || {
        placed
            .get()
            .map(|rect| CanvasView::new(rect.min + pan.get(), scale.get()))
    });
    let item = view! {
        <Frame width=20.0 height=20.0 fill={AMBER} @test_id="canvas.item" />
    };
    let held = view! {
        <Frame width=240.0 height=120.0 fill={BLUE}>
            <Canvas @node_ref=&canvas view={view}>
                <CanvasItem x=20.0 y=10.0 width=20.0 height=20.0 scales=true>{item}</CanvasItem>
            </Canvas>
        </Frame>
    };
    let canvas_rect = measured_rect(canvas.get());
    create_effect(move || placed.set(canvas_rect.get()));
    let item_rect = measured_rect(item);
    let report = create_memo(move || match (item_rect.get(), canvas_rect.get()) {
        (Some(item), Some(canvas)) => format!(
            "Item {} at {:.0},{:.0}",
            size(Some(item)),
            item.min.x - canvas.min.x,
            item.min.y - canvas.min.y
        ),
        _ => "Item unmeasured".to_owned(),
    });
    view! {
        <List spacing=12.0>
            {held}
            <Text color={INK}>{report}</Text>
            <Button id="canvas.pan" label="Pan" on_click={move || pan.set(Vec2::new(50.0, 0.0))} />
            <Button id="canvas.zoom" label="Zoom" on_click={move || scale.set(2.0)} />
        </List>
    }
}

#[component]
fn OverlayFixture() -> NodeId {
    let open = create_signal(false);
    let dismissed = create_signal(String::from("nothing"));
    let anchor = NodeRef::new();
    let report = create_memo(move || format!("Dismissed {}", dismissed.get()));
    let trigger = view! {
        <Interactive
            @test_id="overlay.open"
            @node_ref=&anchor
            focusable=true
            on_click={move || open.set(true)}
        >
            <Frame fill={AMBER} padding={Sides::symmetric(12.0, 6.0)}>
                <Text color={INK}>"Open"</Text>
            </Frame>
        </Interactive>
    };
    let anchor_rect = measured_rect(trigger);
    view! {
        <List spacing=60.0>
            {trigger}
            <Overlay
                anchor={&anchor}
                placement={Placement::BelowStart}
                open={open}
                on_dismiss={move |reason: Dismiss| {
                    dismissed.set(format!("{reason:?}"));
                    open.set(false);
                }}
            >
                <OverlayContent anchor={anchor_rect} />
            </Overlay>
            <Text color={INK} @test_id="overlay.report">{report}</Text>
        </List>
    }
}

#[component]
fn OverlayContent(anchor: Memo<Option<Rect>>) -> NodeId {
    let content = view! {
        <Frame width=120.0 height=40.0 fill={GREEN} @test_id="overlay.content">
            <Text color={INK}>{"Menu"}</Text>
        </Frame>
    };
    let placed = measured_rect(content);
    let report = create_memo(move || match (placed.get(), anchor.get()) {
        (Some(content), Some(anchor)) => format!(
            "Menu at {:.0},{:.0} below",
            content.min.x - anchor.min.x,
            content.min.y - anchor.max.y
        ),
        _ => "Menu unmeasured".to_owned(),
    });
    view! {
        <List spacing=4.0>
            {content}
            <Frame fill={PAPER}>
                <Text color={INK}>{report}</Text>
            </Frame>
        </List>
    }
}

#[component]
fn PortalFixture() -> NodeId {
    let in_first = create_signal(true);
    let clicks = create_signal(0_u32);
    let lent = view! {
        <Interactive
            @test_id="portal.lent"
            focusable=true
            on_click={move || clicks.set(clicks.get_untracked() + 1)}
        >
            <Frame width=100.0 height=30.0 fill={GREEN} />
        </Interactive>
    };
    let first = create_memo(move || in_first.get().then_some(lent));
    let second = create_memo(move || (!in_first.get()).then_some(lent));
    let first_box = view! {
        <Frame width=140.0 height=50.0 fill={BLUE} padding={Sides::all(10.0)}>
            <Portal node={first} />
        </Frame>
    };
    let second_box = view! {
        <Frame width=140.0 height=50.0 fill={AMBER} padding={Sides::all(10.0)}>
            <Portal node={second} />
        </Frame>
    };
    let lent_rect = measured_rect(lent);
    let first_rect = measured_rect(first_box);
    let second_rect = measured_rect(second_box);
    let report = create_memo(move || {
        let inside = |outer: Option<Rect>| matches!((lent_rect.get(), outer), (Some(lent), Some(outer)) if outer.contains_rect(lent));
        let place = match (inside(first_rect.get()), inside(second_rect.get())) {
            (true, _) => "first",
            (_, true) => "second",
            _ => "nowhere",
        };
        format!("Shown in {place}, clicked {}", clicks.get())
    });
    view! {
        <List spacing=12.0>
            {first_box}
            {second_box}
            <Text color={INK}>{report}</Text>
            <Button
                id="portal.move"
                label="Move"
                on_click={move || in_first.set(!in_first.get_untracked())}
            />
        </List>
    }
}

fn previous(text: &str, at: usize) -> usize {
    text[..at]
        .char_indices()
        .last()
        .map_or(0, |(index, _)| index)
}

fn next(text: &str, at: usize) -> usize {
    text[at..]
        .chars()
        .next()
        .map_or(at, |next| at + next.len_utf8())
}

#[component]
fn EditingFixture() -> NodeId {
    let content = create_signal("ab cd".to_owned());
    let caret = create_signal(0_usize);
    let anchor = create_signal(None::<usize>);
    let selection = create_memo(move || {
        let at = caret.get();
        let from = anchor.get().unwrap_or(at);
        from.min(at)..from.max(at)
    });
    let font = FontId::monospace(16.0);
    let marks = create_memo(move || {
        let selected = selection.get();
        (!selected.is_empty())
            .then(|| TextMark::new(selected, AMBER))
            .into_iter()
            .collect::<Vec<_>>()
    });
    let glyphs = create_memo(move || {
        content.with(|text| {
            text.match_indices(' ')
                .map(|(index, _)| TextGlyph::new(index..index + 1, "\u{b7}", font, BLUE))
                .collect::<Vec<_>>()
        })
    });
    let carets = create_memo(move || vec![TextCaret::new(caret.get(), BLUE, 2.0)]);
    let text = view! {
        <Text
            color={INK}
            font={font}
            overflow={beui::Overflow::Visible}
            marks={marks}
            glyphs={glyphs}
            carets={carets}
        >
            {content}
        </Text>
    };
    let geometry = measure_text(text);
    let on_key = move |press: KeyPress| {
        if !press.pressed {
            return false;
        }
        let held = content.get_untracked();
        let at = caret.get_untracked();
        let moved = match press.key {
            Key::ArrowLeft => previous(&held, at),
            Key::ArrowRight => next(&held, at),
            Key::Home => 0,
            Key::End => held.len(),
            Key::Backspace => {
                let from = previous(&held, at);
                content.update(|text| text.replace_range(from..at, ""));
                anchor.set(None);
                caret.set(from);
                return true;
            }
            _ => return false,
        };
        match press.modifiers.shift {
            true if anchor.get_untracked().is_none() => anchor.set(Some(at)),
            true => {}
            false => anchor.set(None),
        }
        caret.set(moved);
        true
    };
    let on_text = move |typed: String| {
        let at = caret.get_untracked();
        content.update(|text| text.insert_str(at, &typed));
        anchor.set(None);
        caret.set(at + typed.len());
    };
    let on_pointer = move |input: PointerInput| {
        if input.stage == PointerStage::Down
            && let Some(index) = geometry
                .get_untracked()
                .and_then(|geometry| geometry.index_at(input.pos))
        {
            anchor.set(None);
            caret.set(index);
        }
        false
    };
    let report = create_memo(move || {
        let selected = selection.get();
        format!(
            "Text {} caret {} selected {}..{}",
            content.get(),
            caret.get(),
            selected.start,
            selected.end
        )
    });
    let round_trip = create_memo(move || {
        let at = caret.get();
        content.with(|_| ());
        let Some(geometry) = geometry.get() else {
            return "Round trip unmeasured".to_owned();
        };
        let found = geometry
            .caret_rect(at, 2.0)
            .and_then(|rect| geometry.index_at(rect.center()));
        match found {
            Some(found) if found == at => "Round trip ok".to_owned(),
            found => format!("Round trip {at} to {found:?}"),
        }
    });
    view! {
        <List spacing=12.0>
            <Interactive
                @test_id="editing.field"
                focusable=true
                ime=true
                on_key={on_key}
                on_text={on_text}
                on_pointer={on_pointer}
            >
                <Frame width=240.0 padding={Sides::all(8.0)} fill={PAPER}>{text}</Frame>
            </Interactive>
            <Text color={INK}>{report}</Text>
            <Text color={INK}>{round_trip}</Text>
        </List>
    }
}

#[component]
fn TextInputFixture() -> NodeId {
    let value = create_signal("ab cd".to_owned());
    let report = create_memo(move || format!("Value {}", value.get()));
    view! {
        <List spacing=12.0>
            <Frame width=240.0>
                <beui::styled::TextInput @test_id="text_input.field" value={value} />
            </Frame>
            <Text color={INK}>{report}</Text>
        </List>
    }
}
