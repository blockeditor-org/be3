use beui_core::color::Color32;
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::primitives;
use beui_view::reactive::{
    Align2, Child, Corners, Extent, Get, IntoProp, Memo, Outline, Prop, Shadow, Sides, create_memo,
    focus_ring,
};

use crate::theme::{
    BORDER_WIDTH, FLOATING_SHADOW_DROP, FLOATING_SHADOW_SPREAD, FOCUS_RING_WIDTH, POINTER_REACH,
    ThemeStore, use_theme,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Elevation {
    #[default]
    Flat,
    Floating,
}

impl Elevation {
    fn shadow(self, theme: &ThemeStore) -> Option<Shadow> {
        match self {
            Elevation::Flat => None,
            Elevation::Floating => Some(
                Shadow::new(theme.shadow.get(), FLOATING_SHADOW_SPREAD).drop(FLOATING_SHADOW_DROP),
            ),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ColorRole {
    None,
    Background,
    Surface,
    SurfaceRaised,
    Hover,
    Pressed,
    Border,
    Text,
    TextMuted,
    Accent,
    AccentHover,
    AccentActive,
    AccentSoft,
    OnAccent,
    Danger,
    Warning,
    Success,
    ScrollThumb,
    ScrollThumbHover,
    ScrollThumbActive,
    Track,
    Knob,
    Custom(Color32),
}

impl ColorRole {
    pub const ROLES: [ColorRole; 21] = [
        ColorRole::Background,
        ColorRole::Surface,
        ColorRole::SurfaceRaised,
        ColorRole::Hover,
        ColorRole::Pressed,
        ColorRole::Border,
        ColorRole::Text,
        ColorRole::TextMuted,
        ColorRole::Accent,
        ColorRole::AccentHover,
        ColorRole::AccentActive,
        ColorRole::AccentSoft,
        ColorRole::OnAccent,
        ColorRole::Danger,
        ColorRole::Warning,
        ColorRole::Success,
        ColorRole::ScrollThumb,
        ColorRole::ScrollThumbHover,
        ColorRole::ScrollThumbActive,
        ColorRole::Track,
        ColorRole::Knob,
    ];

    pub fn resolve(self, theme: &ThemeStore) -> Color32 {
        match self {
            ColorRole::None => Color32::TRANSPARENT,
            ColorRole::Background => theme.background.get(),
            ColorRole::Surface => theme.surface.get(),
            ColorRole::SurfaceRaised => theme.surface_raised.get(),
            ColorRole::Hover => theme.hover.get(),
            ColorRole::Pressed => theme.pressed.get(),
            ColorRole::Border => theme.border.get(),
            ColorRole::Text => theme.text.get(),
            ColorRole::TextMuted => theme.text_muted.get(),
            ColorRole::Accent => theme.accent.get(),
            ColorRole::AccentHover => theme.accent_hover.get(),
            ColorRole::AccentActive => theme.accent_active.get(),
            ColorRole::AccentSoft => theme.accent_soft.get(),
            ColorRole::OnAccent => theme.on_accent.get(),
            ColorRole::Danger => theme.danger.get(),
            ColorRole::Warning => theme.warning.get(),
            ColorRole::Success => theme.success.get(),
            ColorRole::ScrollThumb => theme.scroll_thumb.get(),
            ColorRole::ScrollThumbHover => theme.scroll_thumb_hover.get(),
            ColorRole::ScrollThumbActive => theme.scroll_thumb_active.get(),
            ColorRole::Track => theme.track.get(),
            ColorRole::Knob => theme.knob.get(),
            ColorRole::Custom(color) => color,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            ColorRole::None => "none",
            ColorRole::Background => "background",
            ColorRole::Surface => "surface",
            ColorRole::SurfaceRaised => "surface raised",
            ColorRole::Hover => "hover",
            ColorRole::Pressed => "pressed",
            ColorRole::Border => "border",
            ColorRole::Text => "text",
            ColorRole::TextMuted => "text muted",
            ColorRole::Accent => "accent",
            ColorRole::AccentHover => "accent hover",
            ColorRole::AccentActive => "accent active",
            ColorRole::AccentSoft => "accent soft",
            ColorRole::OnAccent => "on accent",
            ColorRole::Danger => "danger",
            ColorRole::Warning => "warning",
            ColorRole::Success => "success",
            ColorRole::ScrollThumb => "scroll thumb",
            ColorRole::ScrollThumbHover => "scroll thumb hover",
            ColorRole::ScrollThumbActive => "scroll thumb active",
            ColorRole::Track => "track",
            ColorRole::Knob => "knob",
            ColorRole::Custom(_) => "custom",
        }
    }

    pub fn custom(color: impl IntoProp<Color32>) -> Prop<ColorRole> {
        color.into_prop().map(ColorRole::Custom)
    }

    const fn themed(self) -> bool {
        !matches!(self, ColorRole::None | ColorRole::Custom(_))
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Radius {
    None,
    Xs,
    Sm,
    Md,
    Lg,
    Full,
    Custom(Corners),
}

impl Radius {
    pub const ALL: [Radius; 6] = [
        Radius::None,
        Radius::Xs,
        Radius::Sm,
        Radius::Md,
        Radius::Lg,
        Radius::Full,
    ];

    pub const fn px(self) -> f32 {
        match self {
            Radius::None => 0.0,
            Radius::Xs => 2.0,
            Radius::Sm => 4.0,
            Radius::Md => 6.0,
            Radius::Lg => 10.0,
            Radius::Full => 10_000.0,
            Radius::Custom(corners) => corners.top_left,
        }
    }

    pub const fn custom(radius: f32) -> Self {
        Radius::Custom(Corners::all(radius))
    }

    pub const fn corners(self) -> Corners {
        match self {
            Radius::Custom(corners) => corners,
            radius => Corners::all(radius.px()),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Radius::None => "none",
            Radius::Xs => "xs",
            Radius::Sm => "sm",
            Radius::Md => "md",
            Radius::Lg => "lg",
            Radius::Full => "full",
            Radius::Custom(_) => "custom",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Border {
    pub color: ColorRole,
    pub width: f32,
    pub offset: f32,
}

impl Border {
    pub const DEFAULT: Border = Border::new(ColorRole::Border);
    pub const FOCUS: Border = Border::new(ColorRole::Accent).width(FOCUS_RING_WIDTH);

    pub const fn new(color: ColorRole) -> Self {
        Self {
            color,
            width: BORDER_WIDTH,
            offset: 0.0,
        }
    }

    pub const fn width(self, width: f32) -> Self {
        Self { width, ..self }
    }

    pub const fn offset(self, offset: f32) -> Self {
        Self { offset, ..self }
    }

    fn outline(self, theme: &ThemeStore) -> Outline {
        Outline::new(self.color.resolve(theme), self.width).offset(self.offset)
    }
}

pub fn focus_border(focused: impl IntoProp<bool>, border: Border) -> Memo<Option<Border>> {
    let ring = focus_ring(focused);
    create_memo(move || ring.get().then_some(border))
}

fn fill_color(fill: Prop<ColorRole>, theme: ThemeStore) -> Prop<Color32> {
    match fill {
        Prop::Static(role) if !role.themed() => Prop::Static(role.resolve(&theme)),
        fill => Prop::dynamic(move || fill.get().resolve(&theme)),
    }
}

fn border_outline(border: Prop<Option<Border>>, theme: ThemeStore) -> Prop<Option<Outline>> {
    match border {
        Prop::Static(None) => Prop::Static(None),
        Prop::Static(Some(border)) if !border.color.themed() => {
            Prop::Static(Some(border.outline(&theme)))
        }
        border => Prop::dynamic(move || border.get().map(|border| border.outline(&theme))),
    }
}

#[component]
pub fn Frame(
    #[prop(default = Extent::INTRINSIC)] width: Prop<Extent>,
    #[prop(default = Extent::INTRINSIC)] height: Prop<Extent>,
    #[prop(default = None)] aspect_ratio: Prop<Option<f32>>,
    #[prop(default = Sides::ZERO)] padding: Prop<Sides>,
    #[prop(default = Align2::STRETCH)] align: Prop<Align2>,
    #[prop(default = ColorRole::None)] fill: Prop<ColorRole>,
    #[prop(default = Radius::None)] radius: Prop<Radius>,
    #[prop(default = None)] border: Prop<Option<Border>>,
    #[prop(default = Elevation::Flat)] elevation: Prop<Elevation>,
    #[prop(default = false)] pointer: Prop<bool>,
    #[prop(default = true)] visible: Prop<bool>,
    children: Option<Child>,
) -> NodeId {
    let theme = use_theme();
    let fill = fill_color(fill, theme);
    let outline = border_outline(border, theme);
    let radius = radius.map(Radius::corners);
    let shadow = match elevation {
        Prop::Static(Elevation::Flat) => Prop::Static(None),
        elevation => Prop::dynamic(move || elevation.get().shadow(&theme)),
    };
    let pointer = pointer.map(|pointer| if pointer { POINTER_REACH } else { 0.0 });
    match children {
        Some(child) => view! {
            <primitives::Frame
                width
                height
                aspect_ratio
                padding
                align
                fill
                radius
                outline
                shadow
                pointer
                visible
            >
                {child}
            </primitives::Frame>
        },
        None => view! {
            <primitives::Frame
                width
                height
                aspect_ratio
                padding
                align
                fill
                radius
                outline
                shadow
                pointer
                visible
            />
        },
    }
}
