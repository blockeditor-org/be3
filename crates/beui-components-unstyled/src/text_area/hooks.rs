use beui_macros::{component, view};

use beui_core::base::{Direction, Justify};
use beui_core::color::Color32;
use beui_core::font::TextAlign;
use beui_core::icons::ICON_CHECK;
use beui_core::node::NodeId;
use beui_core::rich::SpanStyle;
use beui_view::components::text::Span;
use beui_view::primitives::*;
use beui_view::reactive::*;
use text_editor_core::{LineLook, ListMarker, MarkSet, SynHlStyle};

use crate::{Toggle, ToggleHandle};

use super::colors::TextAreaColors;
use super::state::TextAreaState;

const BULLET_SIZE: f32 = 6.0;
const BULLET_ROOM: f32 = 10.0;
const MARKER_GAP_EM: f32 = 0.3;
const CHECKBOX_SIZE: f32 = 18.0;
const CHECKBOX_RADIUS: f32 = 3.0;
const CHECKBOX_OUTLINE: f32 = 1.5;
const QUOTE_INDENT: f32 = 14.0;
const QUOTE_BAR_WIDTH: f32 = 3.0;
const LIST_INDENT: f32 = 24.0;
pub const CODE_OUTSET: f32 = 3.0;

#[derive(Clone, PartialEq, Debug)]
pub struct TextLeaf {
    pub text: String,
    pub style: SpanStyle,
    pub syntax: SynHlStyle,
    pub break_after: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum MarkerKind {
    Bullet,
    Number(usize),
    Check(bool),
    Void(MarkSet),
}

impl From<ListMarker> for MarkerKind {
    fn from(list: ListMarker) -> Self {
        match list {
            ListMarker::Bullet => MarkerKind::Bullet,
            ListMarker::Number(number) => MarkerKind::Number(number),
            ListMarker::Check(checked) => MarkerKind::Check(checked),
        }
    }
}

#[derive(Clone, Copy)]
pub struct TextMarker {
    pub state: TextAreaState,
    pub line: usize,
    pub kind: Memo<MarkerKind>,
    pub style: Memo<SpanStyle>,
    pub checked: Binding<bool>,
    pub disabled: Memo<bool>,
}

impl TextMarker {
    pub fn number(&self) -> Memo<String> {
        let kind = self.kind;
        create_memo(move || match kind.get() {
            MarkerKind::Number(number) => format!("{number}."),
            _ => String::new(),
        })
    }

    pub fn gap(&self) -> Memo<Sides> {
        let style = self.style;
        create_memo(move || Sides::new(0.0, 0.0, style.get().font.size * MARKER_GAP_EM, 0.0))
    }
}

#[derive(Clone, Copy)]
pub struct TextLine {
    pub look: Memo<LineLook>,
    pub padding: Memo<f32>,
    pub text: NodeId,
}

impl TextLine {
    pub fn quotes(&self) -> Memo<Vec<usize>> {
        let look = self.look;
        create_memo(move || (0..look.get().quote).collect())
    }

    pub fn nested(&self) -> Memo<usize> {
        let look = self.look;
        create_memo(move || {
            let look = look.get();
            match look.list.is_some() && look.marker.is_empty() {
                true => look.depth,
                false => 0,
            }
        })
    }
}

#[component]
pub fn LeafSpan(leaf: TextLeaf) -> Span {
    let TextLeaf {
        text,
        style,
        break_after,
        ..
    } = leaf;
    view! {
        <Span
            font={style.font}
            color={style.color}
            underline={style.underline}
            strikethrough={style.strikethrough}
            break_after
        >
            {text}
        </Span>
    }
}

pub fn default_leaf() -> RenderFn<TextLeaf, Span> {
    RenderFn::new(|leaf: TextLeaf| {
        view! {
            <LeafSpan leaf />
        }
    })
}

pub fn default_marker(colors: Memo<TextAreaColors>) -> RenderFn<TextMarker> {
    RenderFn::new(move |marker: TextMarker| {
        view! {
            <DefaultMarker marker colors />
        }
    })
}

pub fn default_line(colors: Memo<TextAreaColors>) -> RenderFn<TextLine> {
    RenderFn::new(move |line: TextLine| {
        view! {
            <DefaultLine line colors />
        }
    })
}

#[component]
fn DefaultMarker(marker: TextMarker, colors: Memo<TextAreaColors>) -> NodeId {
    let gap = marker.gap();
    let content = match marker.kind.get_untracked() {
        MarkerKind::Bullet => {
            let color = create_memo(move || marker.style.get().color);
            view! {
                <Frame width=BULLET_ROOM height=BULLET_SIZE>
                    <List direction=Direction::Horizontal justify=Justify::Center spacing=0.0>
                        <Frame width=BULLET_SIZE fill=color radius={BULLET_SIZE / 2.0} />
                    </List>
                </Frame>
            }
        }
        MarkerKind::Number(_) => {
            let font = create_memo(move || marker.style.get().font);
            let color = create_memo(move || colors.get().syntax.markdown_symbol);
            view! {
                <Text font color>{marker.number()}</Text>
            }
        }
        MarkerKind::Check(_) => {
            let checked = marker.checked;
            let disabled = marker.disabled;
            view! {
                <Toggle
                    checked
                    disabled
                    capture_presses=true
                    tab_stop=false
                    press_focus=false
                >|handle: ToggleHandle|
                    <CheckboxBox colors handle />
                </Toggle>
            }
        }
        MarkerKind::Void(_) => view! {
            <Frame />
        },
    };
    view! {
        <Frame padding=gap>{content}</Frame>
    }
}

#[component]
fn CheckboxBox(colors: Memo<TextAreaColors>, handle: ToggleHandle) -> NodeId {
    let checked = handle.checked;
    let fill = create_memo(move || match checked.get() {
        true => colors.get().widget,
        false => Color32::TRANSPARENT,
    });
    let outline =
        create_memo(move || Some(Outline::new(colors.get().gutter_arrow, CHECKBOX_OUTLINE)));
    view! {
        <Frame
            width=CHECKBOX_SIZE
            height=CHECKBOX_SIZE
            fill=fill
            outline=outline
            radius=CHECKBOX_RADIUS
        >
            <List spacing=0.0>
                <Show condition={checked}>
                    <Text
                        @sizing=beui_core::base::ItemSize::Percent(100.0)
                        color={Color32::WHITE}
                        align=TextAlign::Center
                    >
                        <Icon glyph={ICON_CHECK.to_owned()} />
                    </Text>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn DefaultLine(line: TextLine, colors: Memo<TextAreaColors>) -> NodeId {
    let look = line.look;
    let outer = create_memo(move || Sides::horizontal((line.padding.get() - CODE_OUTSET).max(0.0)));
    let nested = line.nested();
    let inner = create_memo(move || {
        let inset = line.padding.get().min(CODE_OUTSET);
        let quote = look.get().quote as f32 * QUOTE_INDENT;
        Sides::new(
            inset + quote + nested.get() as f32 * LIST_INDENT,
            0.0,
            inset,
            0.0,
        )
    });
    let code = create_memo(move || match look.get().code_block {
        true => colors.get().code_background,
        false => Color32::TRANSPARENT,
    });
    let bar = create_memo(move || colors.get().syntax.markdown_symbol);
    let quotes = line.quotes();
    let text = line.text;
    view! {
        <Frame padding=outer>
            <Layers>
                <Frame fill=code padding=inner>{text}</Frame>
                <List direction=Direction::Horizontal spacing=0.0>
                    <ForEach keys={quotes}>|_: usize|
                        <Frame
                            width=QUOTE_INDENT
                            padding={Sides::new(0.0, 0.0, QUOTE_INDENT - QUOTE_BAR_WIDTH, 0.0)}
                        >
                            <Frame fill=bar />
                        </Frame>
                    </ForEach>
                </List>
            </Layers>
        </Frame>
    }
}
