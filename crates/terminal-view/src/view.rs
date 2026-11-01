use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use be_block::ObjectId;
use beui::TextCaret;
use beui::reactive::*;
use beui::styled::*;
use beui::{
    Key, KeyPress, NodeId, Overflow, PasteKind, Pasted, PointerPress, ScrollGesture, TextLayout,
    Vec2,
};
use terminal_types::{TerminalColors, TerminalCursor, TerminalLine, TerminalWheel};

use crate::keys::{TerminalInput, key_event, text_event};
use crate::lines::{Lines, Look, Piece, Shown, color, screen};

const TEXT: TextStyle = TextStyle::CODE;
const PADDING: Space = Space::Sm;
const COPY_KEY: Chord = Chord::ctrl(Key::C).shift();

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fit {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Clone)]
struct Shared<T>(Rc<T>);

impl<T> PartialEq for Shared<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Clone, Copy)]
struct Table {
    history: Memo<Shared<HashMap<ObjectId, Rc<TerminalLine>>>>,
    screen: Memo<Shared<HashMap<ObjectId, Shown>>>,
    colors: Memo<TerminalColors>,
}

impl Table {
    fn shown(&self, key: ObjectId) -> Option<Shown> {
        self.screen
            .with(|screen| screen.0.get(&key).cloned())
            .or_else(|| {
                self.history
                    .with(|history| history.0.get(&key).cloned().map(Shown::history))
            })
    }

    fn text(&self, key: ObjectId) -> String {
        untrack(|| {
            self.shown(key)
                .map(|shown| shown.text())
                .unwrap_or_default()
        })
    }
}

fn cell() -> Option<Vec2> {
    let block = lay_out_text("M", TEXT.font(), TextLayout::DEFAULT)?;
    let cell = Vec2::new(block.lines().first()?.rect.max.x, block.size().y);
    (cell.x > 0.0 && cell.y > 0.0).then_some(cell)
}

#[component]
pub fn TerminalView(
    history: Prop<Lines>,
    rows: Prop<Lines>,
    #[prop(default = None)] cursor: Prop<Option<TerminalCursor>>,
    colors: Prop<TerminalColors>,
    #[prop(default = true)] input: Prop<bool>,
    #[prop(default = false)] wheel: Prop<bool>,
    on_input: Callback<TerminalInput>,
    on_wheel: Callback<TerminalWheel>,
    on_fit: Callback<Fit>,
) -> NodeId {
    let colors = create_memo(move || colors.get());
    let input = create_memo(move || input.get());
    let wheel = create_memo(move || wheel.get());
    let scrolling = create_memo(move || !wheel.get());
    let history = create_memo(move || history.get());
    let rows = create_memo(move || rows.get());
    let cursor = create_memo(move || cursor.get());
    let laid = create_memo(move || Shared(Rc::new(screen(&rows.get(), cursor.get()))));
    let table = Table {
        history: create_memo(move || {
            Shared(Rc::new(history.with(|history| {
                history
                    .iter()
                    .map(|(id, line)| (*id, Rc::clone(line)))
                    .collect()
            })))
        }),
        screen: create_memo(move || Shared(Rc::new(laid.with(|laid| laid.0.1.clone())))),
        colors,
    };
    let screen_keys = create_memo(move || laid.with(|laid| laid.0.0.clone()));
    let keys = create_memo(move || {
        let mut keys: Vec<ObjectId> =
            history.with(|history| history.iter().map(|(id, _)| *id).collect());
        screen_keys.with(|screen| keys.extend(screen.iter().copied()));
        keys
    });
    let background = create_memo(move || color(colors.get().background));
    let api = ComponentRef::<ScrollApi>::new();
    let focus = NodeRef::new();
    let typed = move |typed: TerminalInput| {
        if !input.get_untracked() {
            return;
        }
        api.get().scroll_to_end();
        on_input.call(typed);
    };
    let pastes = create_memo(move || match input.get() {
        true => vec![PasteKind::Text],
        false => Vec::new(),
    });
    let spill = Rc::new(Cell::new(0.0_f32));
    let padding = use_space(PADDING);
    let size_held = create_signal(Vec2::ZERO);
    let size = size_held.read_only();
    let measured = create_memo(move || cell().unwrap_or(Vec2::new(7.0, 15.0)));
    create_effect(move || {
        let size = size.get();
        let inset = 2.0 * padding.get();
        let Some(cell) = cell() else {
            return;
        };
        let fit = Fit {
            cols: ((size.x - inset) / cell.x).floor().max(1.0) as u16,
            rows: ((size.y - inset) / cell.y).floor().max(1.0) as u16,
        };
        untrack(|| on_fit.call(fit));
    });
    let line_height = create_memo(move || measured.get().y);
    let spill = Rc::clone(&spill);
    let wheeled = move |gesture: ScrollGesture| {
        let cell = measured.get_untracked();
        let lines = spill.get() - gesture.delta.y / cell.y;
        let whole = lines.trunc();
        spill.set(lines - whole);
        if whole == 0.0 {
            return;
        }
        on_wheel.call(TerminalWheel {
            lines: whole as i16,
            col: (gesture.local.x / cell.x).max(0.0) as u16,
            row: (gesture.local.y / cell.y).max(0.0) as u16,
        });
    };
    let built = view! {
        <Frame fill={ColorRole::custom(background)}>
            <Scroll follow_end=true api>
                <Frame padding=PADDING>
                    <SelectableText copy_key={Some(COPY_KEY)} select_all_key=None>
                        <Interactive
                            focusable=true
                            @node_ref=&focus
                            @test_id={"terminal.screen"}
                            claims_touch=false
                            clipboard_keys={input}
                            on_press={move |_: PointerPress| {
                                if let Some(node) = focus.try_get() {
                                    node.focus();
                                }
                            }}
                            on_text={move |text: String| typed(TerminalInput::Key(text_event(&text)))}
                            on_key={move |press: KeyPress| {
                                if !press.pressed || !input.get_untracked() {
                                    return false;
                                }
                                let Some(key) = key_event(press) else {
                                    return false;
                                };
                                typed(TerminalInput::Key(key));
                                true
                            }}
                            pastes
                            on_paste={move |pasted: Pasted| {
                                if let Pasted::Text(text) = pasted {
                                    typed(TerminalInput::Paste(text));
                                }
                            }}
                        >
                            <List spacing=Space::None>
                                <Show condition={scrolling}>
                                    <VirtualList
                                        keys
                                        item_size=line_height
                                        text={move |key: ObjectId| table.text(key)}
                                    >|key: ReadSignal<ObjectId>|
                                        <TerminalRow key table />
                                    </VirtualList>
                                </Show>
                                <Show condition={wheel}>
                                    <Interactive
                                        on_wheel={beui::unstyled::gestures::scroll_wheel(None, wheeled.clone())}
                                    >
                                        <VirtualList
                                            keys={screen_keys}
                                            item_size=line_height
                                            text={move |key: ObjectId| table.text(key)}
                                        >|key: ReadSignal<ObjectId>|
                                            <TerminalRow key table />
                                        </VirtualList>
                                    </Interactive>
                                </Show>
                            </List>
                        </Interactive>
                    </SelectableText>
                </Frame>
            </Scroll>
        </Frame>
    };
    let size_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = size_measured.get() {
            size_held.set(measured.size);
        }
    });
    built
}

#[component]
fn TerminalRow(key: ReadSignal<ObjectId>, table: Table) -> NodeId {
    let shown = create_memo(move || table.shown(key.get()));
    let colors = table.colors;
    let pieces = create_memo(move || {
        let colors = colors.get();
        shown.with(|shown| {
            shown
                .as_ref()
                .map(|shown| shown.pieces(colors))
                .unwrap_or_default()
                .into_iter()
                .enumerate()
                .collect::<Vec<_>>()
        })
    });
    let overflow = create_memo(move || {
        shown.with(|shown| {
            shown
                .as_ref()
                .map_or(Overflow::Visible, |shown| shown.overflow)
        })
    });
    let foreground = create_memo(move || color(colors.get().foreground));
    let caret_width = text_area_geometry().caret_width;
    let carets = create_memo(move || {
        let at = shown.with(|shown| shown.as_ref().and_then(Shown::caret));
        at.map(|at| vec![TextCaret::new(at, foreground.get(), caret_width)])
            .unwrap_or_default()
    });
    let text_color = create_memo(move || TextColor::Custom(foreground.get().into()));
    view! {
        <Text style=TEXT color={text_color} overflow carets>
            <ForEach keys={pieces}>|(_, piece): (usize, Piece)|
                <TerminalPiece piece />
            </ForEach>
        </Text>
    }
}

#[component]
fn TerminalPiece(piece: Piece) -> TextChild {
    match piece {
        Piece::Break => view! {
            <Break />
        }
        .into_child(),
        Piece::Text(text, look) => {
            let Look {
                color,
                background,
                bold,
                italic,
                underline,
                strikethrough,
            } = look;
            let weight = match bold {
                true => Weight::Bold,
                false => Weight::Normal,
            };
            let span = match background {
                Some(fill) => view! {
                    <Span
                        weight
                        italic
                        color={TextColor::Custom(color.into())}
                        underline={Some(underline)}
                        strikethrough={Some(strikethrough)}
                        background={ColorRole::Custom(fill.into())}
                    >
                        {text}
                    </Span>
                },
                None => view! {
                    <Span
                        weight
                        italic
                        color={TextColor::Custom(color.into())}
                        underline={Some(underline)}
                        strikethrough={Some(strikethrough)}
                    >
                        {text}
                    </Span>
                },
            };
            span.into_child()
        }
    }
}
