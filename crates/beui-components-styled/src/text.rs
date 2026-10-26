use beui_core::color::Color32;
use beui_core::font::{FontFamily, FontId, Overflow, TextAlign};
use beui_core::node::NodeId;
use beui_core::rich::{TextCaret, TextMark};
use beui_macros::{component, view};
use beui_view::primitives;
use beui_view::primitives::*;
use beui_view::reactive::*;

use crate::theme::{ThemeStore, use_theme};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TextSize {
    Xs,
    Sm,
    Base,
    Lg,
    Xl,
    Xl2,
    Xl3,
    Xl4,
    Xl5,
    Custom(f32),
}

impl TextSize {
    pub const ALL: [TextSize; 9] = [
        TextSize::Xs,
        TextSize::Sm,
        TextSize::Base,
        TextSize::Lg,
        TextSize::Xl,
        TextSize::Xl2,
        TextSize::Xl3,
        TextSize::Xl4,
        TextSize::Xl5,
    ];

    pub const fn px(self) -> f32 {
        match self {
            TextSize::Xs => 10.0,
            TextSize::Sm => 12.0,
            TextSize::Base => 14.0,
            TextSize::Lg => 16.0,
            TextSize::Xl => 20.0,
            TextSize::Xl2 => 24.0,
            TextSize::Xl3 => 30.0,
            TextSize::Xl4 => 36.0,
            TextSize::Xl5 => 48.0,
            TextSize::Custom(px) => px,
        }
    }

    pub const fn icon_px(self) -> f32 {
        self.px() * ICON_SCALE
    }

    pub const fn name(self) -> &'static str {
        match self {
            TextSize::Xs => "xs",
            TextSize::Sm => "sm",
            TextSize::Base => "base",
            TextSize::Lg => "lg",
            TextSize::Xl => "xl",
            TextSize::Xl2 => "2xl",
            TextSize::Xl3 => "3xl",
            TextSize::Xl4 => "4xl",
            TextSize::Xl5 => "5xl",
            TextSize::Custom(_) => "custom",
        }
    }
}

impl Eq for TextSize {}

impl std::hash::Hash for TextSize {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.px().to_bits().hash(state);
    }
}

pub const ICON_SCALE: f32 = 16.0 / 14.0;
pub const ICON_SPACING_EM: f32 = 2.0 / 9.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Weight {
    Normal,
    Bold,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Family {
    Sans,
    Mono,
}

impl Family {
    pub const fn font(self) -> FontFamily {
        match self {
            Family::Sans => FontFamily::Proportional,
            Family::Mono => FontFamily::Monospace,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextColor {
    Default,
    Muted,
    Faint,
    Accent,
    OnAccent,
    OnDanger,
    Danger,
    Warning,
    Success,
    Custom(Color32),
}

impl TextColor {
    pub const ROLES: [TextColor; 9] = [
        TextColor::Default,
        TextColor::Muted,
        TextColor::Faint,
        TextColor::Accent,
        TextColor::OnAccent,
        TextColor::OnDanger,
        TextColor::Danger,
        TextColor::Warning,
        TextColor::Success,
    ];

    pub fn resolve(self, theme: &ThemeStore) -> Color32 {
        match self {
            TextColor::Default => theme.text.get(),
            TextColor::Muted => theme.text_muted.get(),
            TextColor::Faint => theme.border.get(),
            TextColor::Accent => theme.accent.get(),
            TextColor::OnAccent => theme.on_accent.get(),
            TextColor::OnDanger => theme.on_danger.get(),
            TextColor::Danger => theme.danger.get(),
            TextColor::Warning => theme.warning.get(),
            TextColor::Success => theme.success.get(),
            TextColor::Custom(color) => color,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            TextColor::Default => "default",
            TextColor::Muted => "muted",
            TextColor::Faint => "faint",
            TextColor::Accent => "accent",
            TextColor::OnAccent => "on accent",
            TextColor::OnDanger => "on danger",
            TextColor::Danger => "danger",
            TextColor::Warning => "warning",
            TextColor::Success => "success",
            TextColor::Custom(_) => "custom",
        }
    }
}

impl From<Color32> for TextColor {
    fn from(color: Color32) -> Self {
        TextColor::Custom(color)
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct TextStyle {
    pub size: Option<TextSize>,
    pub weight: Option<Weight>,
    pub family: Option<Family>,
    pub italic: Option<bool>,
    pub color: Option<TextColor>,
}

impl TextStyle {
    pub const BODY: TextStyle = TextStyle::new();
    pub const CAPTION: TextStyle = TextStyle::new().size(TextSize::Sm).color(TextColor::Muted);
    pub const HEADING: TextStyle = TextStyle::new().size(TextSize::Lg);
    pub const TITLE: TextStyle = TextStyle::new().size(TextSize::Xl);
    pub const DISPLAY: TextStyle = TextStyle::new().size(TextSize::Xl5);
    pub const CODE: TextStyle = TextStyle::new().size(TextSize::Sm).family(Family::Mono);
    pub const PARAGRAPH: TextStyle = TextStyle::new().color(TextColor::Muted);

    pub const fn new() -> Self {
        Self {
            size: None,
            weight: None,
            family: None,
            italic: None,
            color: None,
        }
    }

    pub const fn size(self, size: TextSize) -> Self {
        Self {
            size: Some(size),
            ..self
        }
    }

    pub const fn weight(self, weight: Weight) -> Self {
        Self {
            weight: Some(weight),
            ..self
        }
    }

    pub const fn bold(self) -> Self {
        self.weight(Weight::Bold)
    }

    pub const fn family(self, family: Family) -> Self {
        Self {
            family: Some(family),
            ..self
        }
    }

    pub const fn italic(self, italic: bool) -> Self {
        Self {
            italic: Some(italic),
            ..self
        }
    }

    pub const fn color(self, color: TextColor) -> Self {
        Self {
            color: Some(color),
            ..self
        }
    }

    pub fn over(self, base: TextStyle) -> TextStyle {
        TextStyle {
            size: self.size.or(base.size),
            weight: self.weight.or(base.weight),
            family: self.family.or(base.family),
            italic: self.italic.or(base.italic),
            color: self.color.or(base.color),
        }
    }

    pub fn font(self) -> FontId {
        FontId::new(
            self.size.unwrap_or(TextSize::Base).px(),
            self.family.unwrap_or(Family::Sans).font(),
        )
        .bold(self.weight == Some(Weight::Bold))
        .italic(self.italic.unwrap_or(false))
    }
}

#[derive(Clone, Copy)]
struct Inherited(Memo<TextStyle>);

fn inherited() -> Option<Memo<TextStyle>> {
    use_context::<Inherited>().map(|inherited| inherited.0)
}

#[component]
pub fn TextStyleProvider(style: Prop<TextStyle>, #[prop(children)] content: Render) -> NodeId {
    let outer = inherited();
    let merged = create_memo(move || {
        let style = style.get();
        outer.map_or(style, |outer| style.over(outer.get()))
    });
    provide_context(Inherited(merged));
    content.call(())
}

fn resolved(
    style: Option<Prop<TextStyle>>,
    size: Option<Prop<TextSize>>,
    weight: Option<Prop<Weight>>,
    family: Option<Prop<Family>>,
    italic: Option<Prop<bool>>,
    color: Option<Prop<TextColor>>,
) -> Memo<TextStyle> {
    let outer = inherited();
    create_memo(move || {
        let base = outer.map(|outer| outer.get()).unwrap_or_default();
        let own = style.as_ref().map(Prop::get).unwrap_or_default();
        TextStyle {
            size: size.as_ref().map(Prop::get),
            weight: weight.as_ref().map(Prop::get),
            family: family.as_ref().map(Prop::get),
            italic: italic.as_ref().map(Prop::get),
            color: color.as_ref().map(Prop::get),
        }
        .over(own)
        .over(base)
    })
}

#[component]
pub fn Text(
    style: Option<Prop<TextStyle>>,
    size: Option<Prop<TextSize>>,
    weight: Option<Prop<Weight>>,
    family: Option<Prop<Family>>,
    italic: Option<Prop<bool>>,
    color: Option<Prop<TextColor>>,
    #[prop(default = false)] underline: Prop<bool>,
    #[prop(default = None)] align: Prop<Option<TextAlign>>,
    #[prop(default = None)] vertical_align: Prop<Option<TextAlign>>,
    #[prop(default = Overflow::default())] overflow: Prop<Overflow>,
    #[prop(default = Vec::new())] marks: Prop<Vec<TextMark>>,
    #[prop(default = Vec::new())] carets: Prop<Vec<TextCaret>>,
    children: Children<TextChild>,
) -> NodeId {
    let theme = use_theme();
    let style = resolved(style, size, weight, family, italic, color);
    let font = create_memo(move || style.get().font());
    let color = create_memo(move || {
        style
            .get()
            .color
            .unwrap_or(TextColor::Default)
            .resolve(&theme)
    });
    view! {
        <primitives::Text font color underline align vertical_align overflow marks carets>
            {children}
        </primitives::Text>
    }
}

fn color_part(color: Option<Prop<TextColor>>, theme: ThemeStore) -> Prop<Option<Color32>> {
    match color {
        None => Prop::Static(None),
        Some(color) => Prop::dynamic(move || Some(color.get().resolve(&theme))),
    }
}

fn part<T: Copy + PartialEq + 'static, U: Clone + 'static>(
    prop: Option<Prop<T>>,
    map: impl Fn(T) -> U + 'static,
) -> Prop<Option<U>> {
    match prop {
        None => Prop::Static(None),
        Some(prop) => prop.map(move |value| Some(map(value))),
    }
}

#[component]
pub fn Span(
    size: Option<Prop<TextSize>>,
    weight: Option<Prop<Weight>>,
    family: Option<Prop<Family>>,
    italic: Option<Prop<bool>>,
    color: Option<Prop<TextColor>>,
    #[prop(default = None)] underline: Prop<Option<bool>>,
    #[prop(default = None)] strikethrough: Prop<Option<bool>>,
    #[prop(default = false)] break_after: Prop<bool>,
    children: Children<SpanText>,
) -> primitives::Span {
    let theme = use_theme();
    let size = part(size, TextSize::px);
    let bold = part(weight, |weight| weight == Weight::Bold);
    let family = part(family, Family::font);
    let italic = part(italic, |italic| italic);
    let color = color_part(color, theme);
    view! {
        <primitives::Span size family bold italic color underline strikethrough break_after>
            {children}
        </primitives::Span>
    }
}

#[component]
pub fn Icon(
    glyph: Prop<Option<beui_core::icons::Glyph>>,
    size: Option<Prop<TextSize>>,
    color: Option<Prop<TextColor>>,
) -> primitives::Span {
    let theme = use_theme();
    let size = part(size, TextSize::icon_px);
    let color = color_part(color, theme);
    view! {
        <primitives::Icon glyph size color scale=ICON_SCALE spacing_em=ICON_SPACING_EM />
    }
}
