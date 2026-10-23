use beui_macros::{component, view};

use beui_core::base::{Align, Direction};
use beui_core::color::Color32;
use beui_core::font::TextAlign;
use beui_core::geometry::Vec2;
use beui_core::icons::ICON_CHECK;
use beui_core::node::NodeId;
use beui_core::rich::{SpanBackground, SpanStyle};
use beui_view::components::text::Span;
use beui_view::primitives::*;
use beui_view::reactive::*;
use text_editor_core::{LineLook, ListMarker, MarkSet, SynHlStyle};

use crate::{Toggle, ToggleHandle};

use super::colors::TextAreaColors;
use super::state::TextAreaState;

pub const MARKER_COLUMN_EM: f32 = 1.6;
pub const MARKER_GAP_EM: f32 = 0.3;
pub const LIST_INDENT_EM: f32 = MARKER_COLUMN_EM + MARKER_GAP_EM;
pub const QUOTE_INDENT_EM: f32 = 0.875;
pub const QUOTE_BAR_WIDTH: f32 = 3.0;
pub const BULLET_EM: f32 = 0.375;
const CHECKBOX_SIZE: f32 = 18.0;
const CHECKBOX_RADIUS: f32 = 3.0;
const CHECKBOX_OUTLINE: f32 = 1.5;
pub const CODE_OUTSET: f32 = 3.0;
const CODE_RADIUS: f32 = 3.0;

#[derive(Clone)]
pub struct TextLeaf {
    pub text: String,
    pub style: SpanStyle,
    pub syntax: SynHlStyle,
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

    pub fn bullet(&self) -> Memo<f32> {
        let style = self.style;
        create_memo(move || style.get().font.size * BULLET_EM)
    }
}

#[component]
pub fn MarkerColumn(marker: TextMarker, content: NodeId) -> NodeId {
    let style = marker.style;
    let width = create_memo(move || {
        Extent::at_least(style.get().font.size * (MARKER_COLUMN_EM + MARKER_GAP_EM))
    });
    let gap = marker.gap();
    view! {
        <Frame width padding=gap align={Align2::new(Align::End, Align::Start)}>{content}</Frame>
    }
}

#[derive(Clone, Copy)]
pub struct TextLine {
    pub look: Memo<LineLook>,
    pub padding: Memo<f32>,
    pub size: Memo<f32>,
    pub text: NodeId,
}

impl TextLine {
    pub fn quotes(&self) -> Memo<Vec<usize>> {
        let look = self.look;
        create_memo(move || (0..look.get().quote).collect())
    }

    pub fn quote_indent(&self) -> Memo<f32> {
        let size = self.size;
        create_memo(move || size.get() * QUOTE_INDENT_EM)
    }

    pub fn indent(&self) -> Memo<f32> {
        let (look, size) = (self.look, self.size);
        create_memo(move || {
            let look = look.get();
            let nested = match (look.list, look.continues) {
                (Some(_), _) => look.depth as f32 * LIST_INDENT_EM,
                (None, true) => (look.depth + 1) as f32 * LIST_INDENT_EM,
                (None, false) => 0.0,
            };
            (look.quote as f32 * QUOTE_INDENT_EM + nested) * size.get()
        })
    }
}

#[component]
pub fn LeafSpan(
    leaf: TextLeaf,
    #[prop(default = None)] background: Prop<Option<SpanBackground>>,
) -> Span {
    let TextLeaf { text, style, .. } = leaf;
    view! {
        <Span
            font={Some(style.font)}
            color={Some(style.color)}
            underline={Some(style.underline)}
            strikethrough={Some(style.strikethrough)}
            background
        >
            {text}
        </Span>
    }
}

pub fn default_leaf(colors: Memo<TextAreaColors>) -> RenderFn<TextLeaf, Span> {
    RenderFn::new(move |leaf: TextLeaf| {
        let background = match leaf.syntax.code {
            true => Prop::dynamic(move || {
                Some(SpanBackground {
                    color: colors.get().code_background,
                    radius: CODE_RADIUS,
                    outset: Vec2::new(CODE_OUTSET, -1.0),
                })
            }),
            false => Prop::Static(None),
        };
        view! {
            <LeafSpan leaf background />
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
    let content = match marker.kind.get_untracked() {
        MarkerKind::Bullet => {
            let color = create_memo(move || marker.style.get().color);
            let size = marker.bullet();
            let side = create_memo(move || Extent::px(size.get()));
            let radius = create_memo(move || size.get() / 2.0);
            view! {
                <Frame width=side height=side fill=color radius />
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
        MarkerKind::Void(_) => {
            let gap = marker.gap();
            return view! {
                <Frame padding=gap />
            };
        }
    };
    view! {
        <MarkerColumn marker content />
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
    let indent = line.indent();
    let inner = create_memo(move || {
        let inset = line.padding.get().min(CODE_OUTSET);
        Sides::new(inset + indent.get(), 0.0, inset, 0.0)
    });
    let code = create_memo(move || match look.get().code_block {
        true => colors.get().code_background,
        false => Color32::TRANSPARENT,
    });
    let bar = create_memo(move || colors.get().syntax.markdown_symbol);
    let quotes = line.quotes();
    let quote = line.quote_indent();
    let width = create_memo(move || Extent::px(quote.get()));
    let room = create_memo(move || Sides::new(0.0, 0.0, quote.get() - QUOTE_BAR_WIDTH, 0.0));
    let text = line.text;
    view! {
        <Frame padding=outer>
            <Layers>
                <Frame fill=code padding=inner>{text}</Frame>
                <List direction=Direction::Horizontal spacing=0.0>
                    <ForEach keys={quotes}>|_: usize|
                        <Frame width padding=room>
                            <Frame fill=bar />
                        </Frame>
                    </ForEach>
                </List>
            </Layers>
        </Frame>
    }
}
