//! Screens that must behave the same on beui's native and HTML cores, each driven by the
//! `drive -` script `src/drive/NAME.drive` (guides/testing_a_gui.md, 7).
//!
//! A script uses only commands both cores answer: `settle`, `pause`, `clock stop|run`, `tree`,
//! `ids`, `find`, `wait`, `gone`, `click` (also `double`, `right`, `middle`), `move`,
//! `down`/`up`, `drag`, `wheel` (a tick is 40 pixels in the browser), `type`, `key`,
//! `clipboard TEXT` (then `key ctrl+v` pastes it), `back` and `resize WxH`. Text metrics
//! differ between the cores, so a script names nodes by test id or by text the screen writes
//! itself; a screen checking a layout prints what it `measure`s in a
//! `Text`. The browser alone also answers `selection none|some`, `role ROLE NAME [STATE...]`
//! (Playwright's `getByRole`; `modal` and `inert` read Chrome's accessibility tree over CDP) and
//! `roles ROLE COUNT`, for scripts that run only there (crates/beui-demo-html's).
//!
//! The browser run's `settle` waits until no frame is wanted at once; a frame a timer will ask
//! for later (an indeterminate progress bar's tick) does not count, as natively.

use beui::primitives::*;
use beui::reactive::*;
use beui::{
    Align, Align2, CanvasView, Color32, Corners, Direction, Dismiss, Extent, FontId, Key, KeyPress,
    Placement, PointerInput, PointerStage, Rect, ScrollPosition, Sides, TextCaret, TextGlyph,
    TextMark, Vec2,
};

mod collections;
mod controls;
mod docking;
mod layers;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod rows;
mod surfaces;
#[cfg(test)]
mod tests;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub use native::FixtureProgram;

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
        "paste" => view! {
            <PasteFixture />
        },
        "text_hooks" => view! {
            <TextHooksFixture />
        },
        "back" => view! {
            <BackFixture />
        },
        "canvas" => view! {
            <CanvasFixture />
        },
        "overlay" => view! {
            <OverlayFixture />
        },
        "covering" => view! {
            <CoveringFixture />
        },
        "portal" => view! {
            <PortalFixture />
        },
        "editing" => view! {
            <EditingFixture />
        },
        "text_item" => view! {
            <TextItemFixture />
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
        "long_menus" => view! {
            <controls::LongMenusFixture />
        },
        "popups" => view! {
            <controls::PopupsFixture />
        },
        "described" => view! {
            <controls::DescribedFixture />
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
        "rows" => view! {
            <rows::RowsFixture />
        },
        "scroll" => view! {
            <rows::ScrollFixture />
        },
        "scroll_chain" => view! {
            <rows::ScrollChainFixture />
        },
        "drag" => view! {
            <rows::DragFixture />
        },
        "reorder" => view! {
            <rows::ReorderFixture />
        },
        "text_area" => view! {
            <controls::TextAreaFixture />
        },
        "pan_zoom" => view! {
            <surfaces::PanZoomFixture />
        },
        "time_list" => view! {
            <surfaces::TimeListFixture />
        },
        "back_slide" => view! {
            <surfaces::BackSlideFixture />
        },
        "floating" => view! {
            <surfaces::FloatingFixture />
        },
        "stack" => view! {
            <surfaces::StackFixture />
        },
        "pointer_lock" => view! {
            <surfaces::PointerLockFixture />
        },
        "action_bar" => view! {
            <surfaces::ActionBarFixture />
        },
        "progress" => view! {
            <surfaces::ProgressFixture />
        },
        "sheet" => view! {
            <surfaces::SheetFixture />
        },
        "picture" => view! {
            <surfaces::PictureFixture />
        },
        "docking" => view! {
            <docking::DockingFixture />
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
fn TextHooksFixture() -> NodeId {
    let texts = NodeRef::new();
    let rows = NodeRef::new();
    let read = create_signal(String::from("Hooks unread"));
    let keys: Vec<u32> = (0..100).collect();
    view! {
        <List spacing=12.0>
            <Frame @node_ref=&texts>
                <List spacing=4.0>
                    <Text color={INK}>"Alpha"</Text>
                    <Text color={INK}>""</Text>
                    <Text color={INK}>"Beta"</Text>
                </List>
            </Frame>
            <Frame width=160.0 height=100.0>
                <Offset>
                    <VirtualList @node_ref=&rows keys={keys} item_size=20.0>|row: ReadSignal<u32>|
                        <Frame height=20.0 fill={BLUE}>
                            <Text color={PAPER}>{format!("Item {}", row.get_untracked())}</Text>
                        </Frame>
                    </VirtualList>
                </Offset>
            </Frame>
            <Button
                id="text_hooks.read"
                label="Read hooks"
                on_click={move || {
                    let found = texts
                        .get()
                        .texts_within()
                        .iter()
                        .map(TreeNode::text)
                        .collect::<Vec<_>>()
                        .join(",");
                    let offset = rows.get().row_offset(&3_u32);
                    read.set(format!("Texts {found} row 3 at {offset:?}"));
                }}
            />
            <Text color={INK}>{read}</Text>
        </List>
    }
}

#[component]
fn PasteFixture() -> NodeId {
    let pasted = create_signal(String::from("nothing"));
    let report = create_memo(move || format!("Pasted {}", pasted.get()));
    view! {
        <List spacing=12.0>
            <Interactive
                @test_id="paste.target"
                focusable=true
                ime=true
                pastes={vec!["text/html".to_owned(), "text/plain".to_owned()]}
                on_paste={move |clip: beui::ClipItem| {
                    pasted.set(format!("{} {}", clip.format, String::from_utf8_lossy(&clip.data)));
                }}
            >
                <Frame width=160.0 height=30.0 fill={BLUE} />
            </Interactive>
            <Text color={INK}>{report}</Text>
        </List>
    }
}

#[component]
fn BackFixture() -> NodeId {
    let catching = create_signal(true);
    let backs = create_signal(0_u32);
    let report = create_memo(move || format!("Backs {}", backs.get()));
    on_back(catching, move |gesture: beui::BackGesture| {
        if gesture == beui::BackGesture::Invoked {
            backs.update(|backs| *backs += 1);
        }
    });
    view! {
        <List spacing=12.0>
            <beui::styled::Checkbox label="Catch back" checked={catching} />
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

/// Two overlays too tall for either side of their anchor in a 300 high window: a menu
/// covers its anchor to stay whole inside the window, and anything else anchored below
/// (a completion, a tooltip) keeps clear of it.
#[component]
fn CoveringFixture() -> NodeId {
    let menu = create_signal(false);
    let below = create_signal(false);
    let anchor = NodeRef::new();
    let target = view! {
        <Interactive @test_id="covering.anchor" @node_ref=&anchor focusable=true>
            <Frame width=80.0 height=30.0 fill={AMBER} />
        </Interactive>
    };
    let anchor_rect = measured_rect(target);
    let trigger = view! {
        <List spacing=0.0>
            <Frame width=200.0 height=100.0 />
            {target}
        </List>
    };
    let opener = |label: &'static str, open: Signal<bool>| {
        view! {
            <Interactive
                @test_id={format!("covering.open-{label}")}
                focusable=true
                on_click={move || open.set(true)}
            >
                <Frame fill={GREEN} padding={Sides::symmetric(12.0, 6.0)}>
                    <Text color={INK}>{label}</Text>
                </Frame>
            </Interactive>
        }
    };
    let placed = |placement: Placement, open: Signal<bool>, name: &'static str| {
        let content = view! {
            <Frame width=120.0 height=250.0 fill={BLUE} @test_id={format!("covering.{name}")} />
        };
        let rect = measured_rect(content);
        let report = create_memo(move || match (rect.get(), anchor_rect.get()) {
            (Some(content), Some(trigger)) => {
                let anchor_bottom = trigger.max.y;
                format!(
                    "{name} covers {}, within {}",
                    content.min.y < anchor_bottom,
                    content.min.y >= 0.0 && content.max.y <= 300.5
                )
            }
            _ => format!("{name} unmeasured"),
        });
        view! {
            <List spacing=4.0>
                <Overlay
                    anchor={&anchor}
                    placement={placement}
                    open={open}
                    on_dismiss={move |_: Dismiss| open.set(false)}
                >
                    {content}
                </Overlay>
                <Text color={INK}>{report}</Text>
            </List>
        }
    };
    view! {
        <List direction=Direction::Horizontal spacing=8.0>
            {trigger}
            <List spacing=4.0>
                {opener("menu", menu)}
                {opener("below", below)}
                {placed(Placement::Menu, menu, "Menu")}
                {placed(Placement::BelowStart, below, "Below")}
            </List>
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
fn TextItemFixture() -> NodeId {
    let stops = [0_usize, 3, 6];
    let step = create_signal(0_usize);
    let at = create_memo(move || Some(stops[step.get() % stops.len()]));
    let marker = view! {
        <Frame @test_id="text_item.marker" width=2.0 fill={BLUE} />
    };
    let text = view! {
        <Text color={INK} font={FontId::monospace(16.0)} overflow={beui::Overflow::Visible}>
            "ab cd"
            <Break />
            "ef"
            <TextItem at={at}>{marker}</TextItem>
        </Text>
    };
    let geometry = measure_text(text);
    let placed = measured_rect(marker);
    let report = create_memo(move || {
        let Some(at) = at.get() else {
            return "Marker hidden".to_owned();
        };
        let caret = geometry
            .get()
            .and_then(|geometry| geometry.caret_rect(at, 2.0));
        match (placed.get(), caret) {
            (Some(placed), Some(caret))
                if (placed.left() - caret.left()).abs() < 1.0
                    && (placed.top() - caret.top()).abs() < 1.0 =>
            {
                format!("Marker at {at} on its caret")
            }
            (placed, caret) => format!("Marker at {at} placed {placed:?} caret {caret:?}"),
        }
    });
    view! {
        <List spacing=12.0>
            <Frame width=240.0 padding={Sides::all(8.0)} fill={PAPER}>{text}</Frame>
            <Text color={INK}>{report}</Text>
            <Button
                id="text_item.next"
                label="Next"
                on_click={move || step.set(step.get_untracked() + 1)}
            />
        </List>
    }
}

#[component]
fn TextInputFixture() -> NodeId {
    let value = create_signal("ab cd".to_owned());
    let secret = create_signal(String::new());
    let sent = create_signal(0);
    let report = create_memo(move || format!("Value {}", value.get()));
    let hidden = create_memo(move || format!("Secret {}", secret.get()));
    let count = create_memo(move || format!("Sent {}", sent.get()));
    let api = ComponentRef::<beui::styled::TextInputApi>::new();
    let read = create_signal(String::from("Selection unread"));
    view! {
        <List spacing=12.0>
        <beui::styled::Form on_submit={move || sent.update(|sent| *sent += 1)}>
            <List spacing=12.0>
                <Frame width=240.0>
                    <beui::styled::TextInput @test_id="text_input.field" value={value} api />
                </Frame>
                <Frame width=240.0>
                    <beui::styled::TextInput
                        @test_id="text_input.password"
                        value={secret}
                        label="Password"
                        kind={beui::styled::InputKind::Password}
                        name="password"
                        autocomplete="current-password"
                    />
                </Frame>
                <beui::styled::Button label="Send" submit=true />
                <Text color={INK}>{report}</Text>
                <Text color={INK}>{hidden}</Text>
                <Text color={INK}>{count}</Text>
            </List>
        </beui::styled::Form>
        <beui::styled::Button
            label="Select 1 to 3"
            on_click={move || api.get().set_selection(1..3)}
        />
        <beui::styled::Button
            label="Read selection"
            on_click={move || {
                let api = api.get();
                let ranges = api
                    .selection()
                    .iter()
                    .map(|range| format!("{}..{}", range.start, range.end))
                    .collect::<Vec<_>>();
                let ranges = match ranges.is_empty() {
                    true => "none".to_owned(),
                    false => ranges.join(" "),
                };
                read.set(format!("Selection {ranges} caret {}", api.caret()));
            }}
        />
        <Text color={INK}>{read}</Text>
        </List>
    }
}
