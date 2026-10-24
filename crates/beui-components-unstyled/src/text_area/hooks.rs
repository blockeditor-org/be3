use beui_macros::{component, view};

use beui_core::base::{Align, Direction};
use beui_core::color::Color32;
use beui_core::font::TextAlign;
use beui_core::geometry::Vec2;
use beui_core::icons::ICON_CHECK;
use beui_core::node::NodeId;
use beui_core::painter::Corners;
use beui_core::rich::{SpanBackground, SpanStyle};
use beui_view::components::text::Span;
use beui_view::primitives::*;
use beui_view::reactive::*;
use text_editor_core::{LineLook, ListMarker, MarkSet, SynHlStyle};

use crate::{Toggle, ToggleHandle};

use super::colors::TextAreaColors;
use super::state::TextAreaState;

pub const MARKER_COLUMN_EM: f32 = 1.15;
pub const MARKER_GAP_EM: f32 = 0.55;
pub const LIST_INDENT_EM: f32 = MARKER_COLUMN_EM + MARKER_GAP_EM;
pub const QUOTE_INDENT_EM: f32 = 1.4;
pub const MARGIN_EM: f32 = 3.1;
pub const CODE_PADDING_EM: f32 = 1.0;
pub const RULE_WIDTH: f32 = 1.0;
pub const QUOTE_BAR_WIDTH: f32 = 3.0;
pub const BULLET_EM: f32 = 0.375;
const CHECKBOX_SIZE: f32 = 18.0;
const CHECKBOX_RADIUS: f32 = 3.0;
const CHECKBOX_OUTLINE: f32 = 1.5;
pub const CODE_OUTSET: f32 = 3.0;
const CODE_RADIUS: f32 = 3.0;
const CODE_BLOCK_RADIUS: f32 = 6.0;

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
    pub label: Memo<String>,
    pub style: Memo<SpanStyle>,
    pub checked: Binding<bool>,
    pub disabled: Memo<bool>,
}

impl TextMarker {
    pub fn number(&self) -> Memo<String> {
        let (kind, label) = (self.kind, self.label);
        create_memo(move || match kind.get() {
            MarkerKind::Number(_) if !label.get().is_empty() => label.get(),
            MarkerKind::Number(number) => format!("{number}."),
            _ => String::new(),
        })
    }

    pub fn written(&self) -> Memo<bool> {
        let label = self.label;
        create_memo(move || !label.get().is_empty())
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
    pub margin: Memo<f32>,
    pub outdent: Memo<f32>,
    pub space: Memo<(f32, f32)>,
    pub text: NodeId,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct QuoteBar {
    pub level: usize,
    pub opens: bool,
    pub closes: bool,
}

impl TextLine {
    pub fn quotes(&self) -> Memo<Vec<QuoteBar>> {
        let look = self.look;
        create_memo(move || {
            let look = look.get();
            (0..look.quote)
                .map(|level| QuoteBar {
                    level,
                    opens: level >= look.quote_above,
                    closes: level >= look.quote_below,
                })
                .collect()
        })
    }

    pub fn quote_indent(&self) -> Memo<f32> {
        let size = self.size;
        create_memo(move || size.get() * QUOTE_INDENT_EM)
    }

    pub fn column(&self) -> Memo<f32> {
        let (padding, margin) = (self.padding, self.margin);
        create_memo(move || padding.get() + margin.get())
    }

    pub fn indent(&self) -> Memo<f32> {
        let (look, size, margin, outdent) = (self.look, self.size, self.margin, self.outdent);
        create_memo(move || {
            let look = look.get();
            let listed = look.list.is_some() || look.continues;
            let levels = match listed {
                true => look.depth as f32 + f32::from(u8::from(look.continues)),
                false => 0.0,
            };
            let hung = listed && look.quote == 0 && margin.get() > 0.0;
            let nested = (levels - f32::from(u8::from(hung))) * LIST_INDENT_EM;
            let indent = (look.quote as f32 * QUOTE_INDENT_EM + nested) * size.get();
            margin.get() + indent - outdent.get()
        })
    }

    pub fn left(&self) -> Memo<f32> {
        let (padding, indent) = (self.padding, self.indent());
        create_memo(move || (padding.get() + indent.get()).max(0.0))
    }

    pub fn code_padding(&self) -> Memo<f32> {
        let (look, size, margin) = (self.look, self.size, self.margin);
        create_memo(move || match look.get().code_block && margin.get() > 0.0 {
            true => size.get() * CODE_PADDING_EM,
            false => 0.0,
        })
    }

    pub fn code_corners(&self, radius: f32) -> Memo<Corners> {
        let look = self.look;
        create_memo(move || {
            let look = look.get();
            Corners::new(
                if look.code_above { 0.0 } else { radius },
                if look.code_above { 0.0 } else { radius },
                if look.code_below { 0.0 } else { radius },
                if look.code_below { 0.0 } else { radius },
            )
        })
    }
}

impl QuoteBar {
    pub fn corners(self, radius: f32) -> Corners {
        let top = if self.opens { radius } else { 0.0 };
        let bottom = if self.closes { radius } else { 0.0 };
        Corners::new(top, top, bottom, bottom)
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
        MarkerKind::Bullet if marker.written().get_untracked() => {
            let font = create_memo(move || marker.style.get().font);
            let color = create_memo(move || colors.get().syntax.markdown_symbol);
            view! {
                <Text font color>{marker.label}</Text>
            }
        }
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
    let left = line.left();
    let column = line.column();
    let code_padding = line.code_padding();
    let outset = create_memo(move || match code_padding.get() > 0.0 {
        true => 0.0,
        false => CODE_OUTSET.min(left.get()),
    });
    let outer = create_memo(move || {
        let outset = outset.get();
        Sides::new(left.get() - outset, 0.0, (line.padding.get() - outset).max(0.0), 0.0)
    });
    let inner = create_memo(move || {
        let (above, below) = line.space.get();
        let side = outset.get() + code_padding.get();
        Sides::new(side, above, side, below)
    });
    let code = create_memo(move || match look.get().code_block {
        true => colors.get().code_background,
        false => Color32::TRANSPARENT,
    });
    let radius = line.code_corners(CODE_BLOCK_RADIUS);
    let bar = create_memo(move || colors.get().syntax.markdown_symbol);
    let quotes = line.quotes();
    let quote = line.quote_indent();
    let width = create_memo(move || Extent::px(quote.get()));
    let room = create_memo(move || Sides::new(0.0, 0.0, quote.get() - QUOTE_BAR_WIDTH, 0.0));
    let bars = create_memo(move || Sides::new(column.get(), 0.0, 0.0, 0.0));
    let ruled = create_memo(move || Sides::new(column.get(), 0.0, line.padding.get(), 0.0));
    let rule = create_memo(move || match look.get().rule {
        true => colors.get().gutter_border,
        false => Color32::TRANSPARENT,
    });
    let text = line.text;
    view! {
        <Layers>
            <Frame padding=outer>
                <Frame fill=code radius padding=inner>{text}</Frame>
            </Frame>
            <Frame padding=bars>
                <List direction=Direction::Horizontal spacing=0.0>
                    <ForEach keys={quotes}>|quote_bar: QuoteBar|
                        <Frame width padding=room>
                            <Frame fill=bar radius={quote_bar.corners(QUOTE_BAR_WIDTH / 2.0)} />
                        </Frame>
                    </ForEach>
                </List>
            </Frame>
            <Frame padding=ruled align={Align2::new(Align::Stretch, Align::Center)}>
                <Frame height={Extent::px(RULE_WIDTH)} fill=rule />
            </Frame>
        </Layers>
    }
}
