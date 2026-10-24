use beui::reactive::*;
use beui::styled::*;
use beui::{
    Color32, Direction, FontId, KeyPress, NodeId, PasteKind, Pasted, ScrollGesture, TextLayout,
    Vec2,
};

use crate::keys::{TerminalInput, key_event, text_event};
use crate::screen::{Span, TerminalScreen};

const FONT_SIZE: f32 = 13.0;
const PADDING: f32 = 6.0;
const LINE_SPACING: f32 = 1.2;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fit {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: u32,
    pub cell_height: u32,
}

#[component]
pub fn TerminalView(
    screen: TerminalScreen,
    on_input: Callback<TerminalInput>,
    on_fit: Callback<Fit>,
    #[prop(default = false)] pastes: bool,
) -> NodeId {
    let typed = move |input: TerminalInput| {
        screen.scroll_to_bottom();
        on_input.call(input);
    };
    view! {
        <LayoutBuilder>|size: ReadSignal<Vec2>| {
            create_effect(move || {
                let size = size.get();
                let Some(cell) = layout_text("M", FontId::monospace(FONT_SIZE), TextLayout::DEFAULT)
                    .map(|galley| galley.size())
                else {
                    return;
                };
                if cell.x <= 0.0 || cell.y <= 0.0 {
                    return;
                }
                let fit = Fit {
                    cols: ((size.x - 2.0 * PADDING) / cell.x).floor().max(1.0) as u16,
                    rows: ((size.y - 2.0 * PADDING) / cell.y).floor().max(1.0) as u16,
                    cell_width: cell.x as u32,
                    cell_height: cell.y as u32,
                };
                untrack(|| on_fit.call(fit));
            });
            let rows = screen.rows();
            let keys = create_memo(move || (0..rows.with(Vec::len)).collect::<Vec<_>>());
            let background = screen.background();
            let pastes = match pastes {
                true => vec![PasteKind::Text],
                false => Vec::new(),
            };
            view! {
                <Interactive
                    focusable=true
                    @test_id={"terminal.screen"}
                    on_text={move |text: String| typed(TerminalInput::Key(text_event(&text)))}
                    on_key={move |press: KeyPress| {
                        if !press.pressed {
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
                    on_scroll={move |gesture: ScrollGesture| {
                        let rows = (gesture.delta.y / (FONT_SIZE * LINE_SPACING)).round() as isize;
                        if rows != 0 {
                            screen.scroll(-rows);
                        }
                    }}
                >
                    <Frame fill={ColorRole::custom(background)} padding=PADDING>
                        <List spacing=0.0>
                            <ForEach keys>|index: usize| {
                                let row = create_memo(move || rows.with(|rows| rows.get(index).cloned()).unwrap_or_default());
                                view! {
                                    <TerminalLine row />
                                }
                            }</ForEach>
                        </List>
                    </Frame>
                </Interactive>
            }
        }</LayoutBuilder>
    }
}

#[component]
fn TerminalLine(row: Memo<Vec<Span>>) -> NodeId {
    let keys = create_memo(move || (0..row.with(Vec::len)).collect::<Vec<_>>());
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <ForEach keys>|index: usize| {
                let span = create_memo(move || row.with(|row| row.get(index).cloned()));
                view! {
                    <TerminalSpan span />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn TerminalSpan(span: Memo<Option<Span>>) -> NodeId {
    let field = |read: fn(&Span) -> bool| {
        create_memo(move || span.with(|span| span.as_ref().is_some_and(read)))
    };
    let bold = field(|span| span.bold);
    let italic = field(|span| span.italic);
    let weight = create_memo(move || match bold.get() {
        true => Weight::Bold,
        false => Weight::Normal,
    });
    let underline = field(|span| span.underline);
    let text = create_memo(move || {
        span.with(|span| {
            span.as_ref()
                .map(|span| span.text.clone())
                .unwrap_or_default()
        })
    });
    let color = create_memo(move || {
        TextColor::Custom(span.with(|span| span.as_ref().map_or(Color32::WHITE, |span| span.color)))
    });
    let background = create_memo(move || {
        span.with(|span| span.as_ref().and_then(|span| span.background))
            .unwrap_or(Color32::TRANSPARENT)
    });
    view! {
        <Frame fill={ColorRole::custom(background)}>
            <Text
                size=TextSize::Custom(FONT_SIZE)
                family=Family::Mono
                weight
                italic
                color
                underline
            >
                {text}
            </Text>
        </Frame>
    }
}
