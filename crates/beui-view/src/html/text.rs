use crate::reactive::Child;
use crate::reactive::ChildSegment;
use crate::reactive::ChildValue;
use crate::reactive::Children;
use crate::reactive::FromPlaced;
use crate::reactive::Get;
use crate::reactive::IntoChild;
use crate::reactive::IntoProp;
use crate::reactive::IntoSegment;
use crate::reactive::Memo;
use crate::reactive::NodeId;
use crate::reactive::Placed;
use crate::reactive::Prop;
use crate::reactive::ReadSignal;
use crate::reactive::Ready;
use crate::reactive::Signal;
use crate::reactive::SlotChild;
use crate::reactive::SlotHost;
use crate::reactive::SlotId;
use crate::reactive::create_binding;
use crate::types::Color;
use crate::types::FontFamily;
use crate::types::FontId;
use crate::types::Overflow;
use crate::types::SpanBackground;
use crate::types::TextAlign;
use crate::types::TextCaret;
use crate::types::TextGlyph;
use crate::types::TextMark;
use crate::types::WrapAt;
use crate::types::icons::Glyph;
use beui_core_html::beui_html::Node;
use beui_core_html::beui_html::reactive::IntoSlotHost;
use beui_core_html::css;
use beui_core_html::document::{append, create, element, set_attribute, set_style};
use beui_core_html::text::{Piece, Run, TextModel, TextSlot, line_height as font_line_height};
use beui_core_html::tree::TextKind;
use std::rc::Rc;

use super::bind;

fn text_align(align: TextAlign) -> &'static str {
    match align {
        TextAlign::Start => "start",
        TextAlign::Center => "center",
        TextAlign::End => "end",
    }
}

fn bind_text(target: Node, text: Prop<String>) {
    bind(text, move |text| target.set_text(text));
}

fn decorated<T: PartialEq + Default>(prop: &Prop<T>) -> bool {
    !matches!(prop, Prop::Static(value) if *value == T::default())
}

pub fn text(
    font: Prop<FontId>,
    line_height: Prop<Option<f32>>,
    color: Prop<Color>,
    align: Prop<Option<TextAlign>>,
    vertical_align: Prop<Option<TextAlign>>,
    overflow: Prop<Overflow>,
    underline: Prop<bool>,
    line_padding: Prop<(f32, f32)>,
    hang: Prop<usize>,
    marks: Prop<Vec<TextMark>>,
    glyphs: Prop<Vec<TextGlyph>>,
    carets: Prop<Vec<TextCaret>>,
    children: Children<TextChild>,
) -> NodeId {
    let node = create("Text", "div");
    let model = TextModel::new(node);
    let held = model.clone();
    let sized = font.clone();
    bind(font, move |font| {
        held.set_font(font);
        set_style(node, "font", Some(css::font(font)));
    });
    create_binding(move || {
        let height = match line_height.get() {
            Some(height) => height.round().max(1.0),
            None => font_line_height(sized.get().size),
        };
        set_style(node, "line-height", Some(css::px(height)));
    });
    bind(color, move |color| {
        set_style(node, "color", Some(css::color(color)))
    });
    create_binding(move || {
        let horizontal = align.get();
        let vertical = vertical_align.get().unwrap_or(match horizontal {
            Some(_) => TextAlign::Center,
            None => TextAlign::Start,
        });
        set_style(
            node,
            "text-align",
            horizontal.map(|align| text_align(align).to_owned()),
        );
        set_style(
            node,
            "align-content",
            (vertical != TextAlign::Start).then(|| text_align(vertical).to_owned()),
        );
    });
    bind(overflow, move |overflow| {
        let (white_space, wrap, clamp, ellipsis) = match overflow {
            Overflow::Wrap { at, max_lines } => (
                "pre-wrap",
                Some(match at {
                    WrapAt::Words => "break-word",
                    WrapAt::Anywhere => "anywhere",
                }),
                max_lines,
                false,
            ),
            Overflow::Ellipsis => ("pre", None, None, true),
            Overflow::Visible => ("pre", None, None, false),
        };
        set_style(node, "white-space", Some(white_space.to_owned()));
        set_style(node, "overflow-wrap", wrap.map(str::to_owned));
        set_style(node, "display", clamp.map(|_| "-webkit-box".to_owned()));
        set_style(
            node,
            "-webkit-box-orient",
            clamp.map(|_| "vertical".to_owned()),
        );
        set_style(
            node,
            "-webkit-line-clamp",
            clamp.map(|lines| lines.to_string()),
        );
        set_style(
            node,
            "overflow",
            (clamp.is_some() || ellipsis).then(|| "hidden".to_owned()),
        );
        set_style(
            node,
            "text-overflow",
            ellipsis.then(|| "ellipsis".to_owned()),
        );
    });
    bind(underline, move |underline| {
        set_style(
            node,
            "text-decoration-line",
            underline.then(|| "underline".to_owned()),
        );
    });
    bind(line_padding, move |(top, bottom)| {
        set_style(node, "padding-top", (top != 0.0).then(|| css::px(top)));
        set_style(
            node,
            "padding-bottom",
            (bottom != 0.0).then(|| css::px(bottom)),
        );
    });
    let glyphed = decorated(&glyphs);
    match children.into_only() {
        Ok(TextChild::Span(span)) if span.is_plain() => {
            if let Some(element) = element(node) {
                model.set_single(Run::new(&element, span.text, glyphed));
            }
        }
        Ok(child) => Children::from(ChildSegment::One(child)).mount(host(&model, glyphed)),
        Err(children) => children.mount(host(&model, glyphed)),
    }
    let rendered = model.clone();
    create_binding(move || rendered.render(&glyphs.get()));
    if decorated(&hang) || decorated(&marks) || decorated(&carets) {
        set_style(node, "position", Some("relative".to_owned()));
        set_style(node, "z-index", Some("0".to_owned()));
        model.decorate();
        let held = model.clone();
        bind(hang, move |hang| held.set_hang(hang));
        let held = model.clone();
        bind(marks, move |marks| held.set_marks(marks));
        let held = model.clone();
        bind(carets, move |carets| held.set_carets(carets));
    }
    node
}

pub enum TextChild {
    Span(Span),
    Gap(Gap),
    Break(Break),
    Item(TextItem),
}

impl ChildValue for TextChild {
    fn finish_component(
        &mut self,
        component: &crate::reactive::ComponentContext,
        scope: &crate::reactive::ScopeContext,
    ) {
        match self {
            Self::Span(value) => value.finish_component(component, scope),
            Self::Gap(value) => value.finish_component(component, scope),
            Self::Break(value) => value.finish_component(component, scope),
            Self::Item(value) => value.finish_component(component, scope),
        }
    }
}

crate::child_type!(TextChild);

fn inline(kind: &'static str, tag: &'static str, styles: &[(&'static str, &str)]) -> NodeId {
    let node = create(kind, tag);
    for (property, value) in styles {
        set_style(node, property, Some((*value).to_owned()));
    }
    node
}

impl TextChild {
    fn build(self, glyphed: bool) -> (NodeId, Piece) {
        match self {
            TextChild::Span(span) => span.build(glyphed),
            TextChild::Item(item) => (item.node, Piece::Item(item.data)),
            TextChild::Gap(gap) => {
                let node = inline(
                    "Gap",
                    "span",
                    &[("display", "inline-block"), ("user-select", "none")],
                );
                set_style(node, "width", Some(css::px(gap.width)));
                (node, Piece::Skip { newline: false })
            }
            TextChild::Break(_) => {
                let node = create("Break", "span");
                set_attribute(node, "class", Some("beui-break".to_owned()));
                (node, Piece::Skip { newline: true })
            }
        }
    }
}

impl SlotChild for TextChild {
    type Stored = NodeId;

    fn store(self) -> NodeId {
        self.build(false).0
    }
}

struct TextHost {
    model: Rc<TextModel>,
    glyphed: bool,
}

impl SlotHost<TextChild> for TextHost {
    fn store(&self, child: TextChild) -> NodeId {
        let (node, piece) = child.build(self.glyphed);
        self.model.insert(node, piece);
        node
    }

    fn append(&self, stored: NodeId) {
        self.model.append(stored);
    }

    fn open(&self) -> SlotId {
        self.model.open()
    }

    fn fill(&self, slot: SlotId, items: Vec<NodeId>) {
        self.model.fill(slot, items);
    }
}

fn host(model: &Rc<TextModel>, glyphed: bool) -> Rc<dyn SlotHost<TextChild>> {
    Rc::new(TextHost {
        model: model.clone(),
        glyphed,
    })
}

macro_rules! text_child_from {
    ($($ty:ty),*) => {
        $(
            impl IntoChild<TextChild> for $ty {
                fn into_child(self) -> TextChild {
                    TextChild::Span(Span::plain(self.into_prop()))
                }
            }

            impl IntoSegment<TextChild> for $ty {
                fn into_segment(self) -> ChildSegment<TextChild> {
                    ChildSegment::One(self.into_child())
                }
            }

            impl IntoSegment<SpanText> for $ty {
                fn into_segment(self) -> ChildSegment<SpanText> {
                    ChildSegment::One(SpanText(self.into_prop()))
                }
            }
        )*
    };
}

text_child_from!(
    &'static str,
    String,
    Prop<String>,
    Memo<String>,
    Memo<Ready<String>>,
    ReadSignal<String>,
    Signal<String>
);

impl IntoChild<TextChild> for Span {
    fn into_child(self) -> TextChild {
        TextChild::Span(self)
    }
}

impl IntoSegment<TextChild> for Span {
    fn into_segment(self) -> ChildSegment<TextChild> {
        ChildSegment::One(TextChild::Span(self))
    }
}

impl FromPlaced<TextKind> for TextChild {
    fn from_placed(child: TextItem) -> Self {
        TextChild::Item(child)
    }
}

pub struct SpanText(Prop<String>);

crate::value_child_type!(SpanText);

#[derive(Clone, Copy, PartialEq, Default, Debug)]
struct FontPatch {
    size: Option<f32>,
    family: Option<FontFamily>,
    bold: Option<bool>,
    italic: Option<bool>,
}

#[derive(Clone, Copy)]
struct IconSpan {
    scale: f32,
    spacing_em: f32,
}

pub struct Span {
    text: Prop<String>,
    font: Prop<FontPatch>,
    color: Prop<Option<Color>>,
    underline: Prop<Option<bool>>,
    strikethrough: Prop<Option<bool>>,
    background: Prop<Option<SpanBackground>>,
    break_after: Prop<bool>,
    icon: Option<IconSpan>,
}

impl ChildValue for Span {}

fn font_patch(
    font: Option<FontId>,
    size: Option<f32>,
    family: Option<FontFamily>,
    bold: Option<bool>,
    italic: Option<bool>,
) -> FontPatch {
    let whole = font.map_or_else(FontPatch::default, |font| FontPatch {
        size: Some(font.size),
        family: Some(font.family),
        bold: Some(font.bold),
        italic: Some(font.italic),
    });
    FontPatch {
        size: size.or(whole.size),
        family: family.or(whole.family),
        bold: bold.or(whole.bold),
        italic: italic.or(whole.italic),
    }
}

impl Span {
    fn plain(text: Prop<String>) -> Self {
        Self {
            text,
            font: Prop::Static(FontPatch::default()),
            color: Prop::Static(None),
            underline: Prop::Static(None),
            strikethrough: Prop::Static(None),
            background: Prop::Static(None),
            break_after: Prop::Static(false),
            icon: None,
        }
    }

    fn is_plain(&self) -> bool {
        matches!(self.font, Prop::Static(font) if font == FontPatch::default())
            && matches!(self.color, Prop::Static(None))
            && matches!(self.underline, Prop::Static(None))
            && matches!(self.strikethrough, Prop::Static(None))
            && matches!(self.background, Prop::Static(None))
            && matches!(self.break_after, Prop::Static(false))
            && self.icon.is_none()
    }

    fn build(self, glyphed: bool) -> (NodeId, Piece) {
        let span = create(if self.icon.is_some() { "Icon" } else { "Span" }, "span");
        let Some(element) = element(span) else {
            return (span, Piece::Skip { newline: false });
        };
        let piece = match self.icon {
            Some(_) => {
                let run = Node::text("");
                bind_text(run.clone(), self.text);
                IntoSlotHost::<Node>::into_slot_host(&element).append(run);
                Piece::Skip { newline: false }
            }
            None => Piece::Run(Run::new(&element, self.text, glyphed)),
        };
        let icon = self.icon;
        if let Some(icon) = icon {
            set_attribute(span, "aria-hidden", Some("true".to_owned()));
            set_style(span, "user-select", Some("none".to_owned()));
            set_style(
                span,
                "font-family",
                Some(css::family(FontFamily::Icons).to_owned()),
            );
            set_attribute(span, "class", Some("beui-icon".to_owned()));
            set_style(
                span,
                "--beui-icon-spacing",
                (icon.spacing_em != 0.0).then(|| format!("{}em", css::number(icon.spacing_em))),
            );
        }
        bind(self.font, move |font| {
            let size = match (font.size, icon) {
                (Some(size), _) => Some(css::px(size)),
                (None, Some(icon)) if icon.scale != 1.0 => {
                    Some(format!("{}em", css::number(icon.scale)))
                }
                (None, _) => None,
            };
            set_style(span, "font-size", size);
            let line = match icon {
                Some(icon) => font.size.map(|size| size / icon.scale),
                None => font.size,
            };
            set_style(span, "line-height", line.map(|size| css::px(font_line_height(size))));
            if icon.is_none() {
                set_style(
                    span,
                    "font-family",
                    font.family.map(|family| css::family(family).to_owned()),
                );
            }
            set_style(
                span,
                "font-weight",
                font.bold
                    .map(|bold| if bold { "bold" } else { "normal" }.to_owned()),
            );
            set_style(
                span,
                "font-style",
                font.italic
                    .map(|italic| if italic { "italic" } else { "normal" }.to_owned()),
            );
        });
        bind(self.color, move |color| {
            set_style(span, "color", color.map(css::color));
        });
        let (underline, strikethrough) = (self.underline, self.strikethrough);
        create_binding(move || {
            let lines: Vec<&str> = [
                (underline.get() == Some(true)).then_some("underline"),
                (strikethrough.get() == Some(true)).then_some("line-through"),
            ]
            .into_iter()
            .flatten()
            .collect();
            set_style(
                span,
                "text-decoration-line",
                (!lines.is_empty()).then(|| lines.join(" ")),
            );
        });
        bind(self.background, move |background| {
            set_style(
                span,
                "background-color",
                background.map(|background| css::color(background.color)),
            );
            set_style(
                span,
                "border-radius",
                background.map(|background| css::px(background.radius)),
            );
            let clone = background.map(|_| "clone".to_owned());
            set_style(span, "box-decoration-break", clone.clone());
            set_style(span, "-webkit-box-decoration-break", clone);
        });
        let host = IntoSlotHost::<Node>::into_slot_host(&element);
        match self.break_after {
            Prop::Static(false) => {}
            Prop::Static(true) => host.append(Node::element("wbr")),
            Prop::Dynamic(read) => {
                let slot = host.open();
                create_binding(move || {
                    let wbr = read.get().then(|| Node::element("wbr"));
                    host.fill(slot, wbr.into_iter().collect());
                });
            }
        }
        (span, piece)
    }
}

pub fn span(
    font: Prop<Option<FontId>>,
    size: Prop<Option<f32>>,
    family: Prop<Option<FontFamily>>,
    bold: Prop<Option<bool>>,
    italic: Prop<Option<bool>>,
    color: Prop<Option<Color>>,
    underline: Prop<Option<bool>>,
    strikethrough: Prop<Option<bool>>,
    background: Prop<Option<SpanBackground>>,
    break_after: Prop<bool>,
    children: Children<SpanText>,
) -> Span {
    let text = match children.into_only() {
        Ok(SpanText(text)) => text,
        Err(children) => {
            let run = children.into_run();
            Prop::dynamic(move || run.items().iter().map(|piece| piece.0.get()).collect())
        }
    };
    let font = match (font, size, family, bold, italic) {
        (
            Prop::Static(font),
            Prop::Static(size),
            Prop::Static(family),
            Prop::Static(bold),
            Prop::Static(italic),
        ) => Prop::Static(font_patch(font, size, family, bold, italic)),
        (font, size, family, bold, italic) => Prop::dynamic(move || {
            font_patch(
                font.get(),
                size.get(),
                family.get(),
                bold.get(),
                italic.get(),
            )
        }),
    };
    Span {
        text,
        font,
        color,
        underline,
        strikethrough,
        background,
        break_after,
        icon: None,
    }
}

pub fn icon(
    glyph: Prop<Option<Glyph>>,
    size: Prop<Option<f32>>,
    scale: f32,
    spacing_em: f32,
    color: Prop<Option<Color>>,
) -> Span {
    Span {
        font: size.map(|size| FontPatch {
            size,
            ..FontPatch::default()
        }),
        color,
        icon: Some(IconSpan { scale, spacing_em }),
        ..Span::plain(
            glyph.map(|glyph| glyph.map_or_else(String::new, |glyph| glyph.as_str().to_owned())),
        )
    }
}

pub struct Gap {
    width: f32,
}

impl ChildValue for Gap {}

pub fn gap(width: f32) -> Gap {
    Gap { width }
}

impl IntoChild<TextChild> for Gap {
    fn into_child(self) -> TextChild {
        TextChild::Gap(self)
    }
}

impl IntoSegment<TextChild> for Gap {
    fn into_segment(self) -> ChildSegment<TextChild> {
        ChildSegment::One(TextChild::Gap(self))
    }
}

pub type TextItem = Placed<TextKind>;

pub fn text_item(
    at: Option<Prop<Option<usize>>>,
    text: Option<Prop<Option<Rc<str>>>>,
    children: Option<Child>,
) -> TextItem {
    let item = inline("TextItem", "span", &[("display", "inline-block")]);
    if let Some(child) = children {
        append(item, child);
    }
    let slot = match (at, text) {
        (Some(_), _) => {
            set_attribute(item, "class", Some("beui-stand-in".to_owned()));
            set_attribute(item, "title", Some("TODO: TextItem at".to_owned()));
            Prop::Static(TextSlot::Anchored)
        }
        (None, Some(text)) => text.map(|text| TextSlot::inline(text.as_deref())),
        (None, None) => Prop::Static(TextSlot::inline(None)),
    };
    TextItem::new(item, slot)
}

pub struct Break {}

impl ChildValue for Break {}

pub fn line_break() -> Break {
    Break {}
}

impl IntoChild<TextChild> for Break {
    fn into_child(self) -> TextChild {
        TextChild::Break(self)
    }
}

impl IntoSegment<TextChild> for Break {
    fn into_segment(self) -> ChildSegment<TextChild> {
        ChildSegment::One(TextChild::Break(self))
    }
}
