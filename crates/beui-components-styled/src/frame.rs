use beui_view::types::Color32;
use beui_view::reactive::NodeId;
use beui_macros::{component, view};
use beui_view::primitives;
use beui_view::reactive::{
    Align2, Child, Corners, Get, IntoProp, Memo, Outline, Prop, create_memo, focus_ring,
};

use crate::theme::{POINTER_REACH, ThemeStore, use_theme};
use crate::tokens::{CornerSet, Length, Padding, Stroke, resolved};

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
    AccentSoftHover,
    OnAccent,
    Danger,
    DangerFill,
    DangerFillHover,
    DangerFillActive,
    DangerSoft,
    OnDanger,
    Warning,
    Success,
    ScrollThumb,
    ScrollThumbHover,
    ScrollThumbActive,
    Track,
    Knob,
    ControlOutline,
    KnobOutline,
    Dim,
    Scrim,
    ScrimSoft,
    ScrimStrong,
    Custom(Color32),
}

impl ColorRole {
    pub const ROLES: [ColorRole; 33] = [
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
        ColorRole::AccentSoftHover,
        ColorRole::OnAccent,
        ColorRole::Danger,
        ColorRole::DangerFill,
        ColorRole::DangerFillHover,
        ColorRole::DangerFillActive,
        ColorRole::DangerSoft,
        ColorRole::OnDanger,
        ColorRole::Warning,
        ColorRole::Success,
        ColorRole::ScrollThumb,
        ColorRole::ScrollThumbHover,
        ColorRole::ScrollThumbActive,
        ColorRole::Track,
        ColorRole::Knob,
        ColorRole::ControlOutline,
        ColorRole::KnobOutline,
        ColorRole::Dim,
        ColorRole::Scrim,
        ColorRole::ScrimSoft,
        ColorRole::ScrimStrong,
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
            ColorRole::AccentSoftHover => theme.accent_soft_hover.get(),
            ColorRole::OnAccent => theme.on_accent.get(),
            ColorRole::Danger => theme.danger.get(),
            ColorRole::DangerFill => theme.danger_fill.get(),
            ColorRole::DangerFillHover => theme.danger_fill_hover.get(),
            ColorRole::DangerFillActive => theme.danger_fill_active.get(),
            ColorRole::DangerSoft => theme.danger_soft.get(),
            ColorRole::OnDanger => theme.on_danger.get(),
            ColorRole::Warning => theme.warning.get(),
            ColorRole::Success => theme.success.get(),
            ColorRole::ScrollThumb => theme.scroll_thumb.get(),
            ColorRole::ScrollThumbHover => theme.scroll_thumb_hover.get(),
            ColorRole::ScrollThumbActive => theme.scroll_thumb_active.get(),
            ColorRole::Track => theme.track.get(),
            ColorRole::Knob => theme.knob.get(),
            ColorRole::ControlOutline => {
                theme.control_outline.get().unwrap_or(Color32::TRANSPARENT)
            }
            ColorRole::KnobOutline => theme.knob_outline.get().unwrap_or(Color32::TRANSPARENT),
            ColorRole::Dim => theme.dim.get(),
            ColorRole::Scrim => theme.scrim.get(),
            ColorRole::ScrimSoft => theme.scrim_soft.get(),
            ColorRole::ScrimStrong => theme.scrim_strong.get(),
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
            ColorRole::AccentSoftHover => "accent soft hover",
            ColorRole::OnAccent => "on accent",
            ColorRole::Danger => "danger",
            ColorRole::DangerFill => "danger fill",
            ColorRole::DangerFillHover => "danger fill hover",
            ColorRole::DangerFillActive => "danger fill active",
            ColorRole::DangerSoft => "danger soft",
            ColorRole::OnDanger => "on danger",
            ColorRole::Warning => "warning",
            ColorRole::Success => "success",
            ColorRole::ScrollThumb => "scroll thumb",
            ColorRole::ScrollThumbHover => "scroll thumb hover",
            ColorRole::ScrollThumbActive => "scroll thumb active",
            ColorRole::Track => "track",
            ColorRole::Knob => "knob",
            ColorRole::ControlOutline => "control outline",
            ColorRole::KnobOutline => "knob outline",
            ColorRole::Dim => "dim",
            ColorRole::Scrim => "scrim",
            ColorRole::ScrimSoft => "scrim soft",
            ColorRole::ScrimStrong => "scrim strong",
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
    Tab,
    Lg,
    Full,
    Custom(f32),
}

impl Radius {
    pub const ALL: [Radius; 7] = [
        Radius::None,
        Radius::Xs,
        Radius::Sm,
        Radius::Md,
        Radius::Tab,
        Radius::Lg,
        Radius::Full,
    ];

    pub const fn px(self) -> f32 {
        match self {
            Radius::None => 0.0,
            Radius::Xs => 2.0,
            Radius::Sm => 4.0,
            Radius::Md => 6.0,
            Radius::Tab => 7.0,
            Radius::Lg => 10.0,
            Radius::Full => 10_000.0,
            Radius::Custom(radius) => radius,
        }
    }

    pub const fn corners(self) -> Corners {
        Corners::all(self.px())
    }

    pub const fn around(self, offset: Stroke) -> Corners {
        let corners = self.corners();
        let grow = offset.px();
        Corners::new(
            corners.top_left + grow,
            corners.top_right + grow,
            corners.bottom_right + grow,
            corners.bottom_left + grow,
        )
    }

    pub const fn name(self) -> &'static str {
        match self {
            Radius::None => "none",
            Radius::Xs => "xs",
            Radius::Sm => "sm",
            Radius::Md => "md",
            Radius::Tab => "tab",
            Radius::Lg => "lg",
            Radius::Full => "full",
            Radius::Custom(_) => "custom",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Border {
    pub color: ColorRole,
    pub width: Stroke,
    pub offset: Stroke,
}

impl Border {
    pub const DEFAULT: Border = Border::new(ColorRole::Border);
    pub const FOCUS: Border = Border {
        color: ColorRole::Accent,
        width: Stroke::Focus,
        offset: Stroke::None,
    };

    pub const fn new(color: ColorRole) -> Self {
        Self {
            color,
            width: Stroke::Hairline,
            offset: Stroke::None,
        }
    }

    pub const fn width(self, width: Stroke) -> Self {
        Self { width, ..self }
    }

    pub const fn offset(self, offset: Stroke) -> Self {
        Self { offset, ..self }
    }

    fn outline(self, theme: &ThemeStore) -> Outline {
        Outline::new(self.color.resolve(theme), self.width.px()).offset(self.offset.px())
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
    #[prop(default = Length::INTRINSIC)] width: Prop<Length>,
    #[prop(default = Length::INTRINSIC)] height: Prop<Length>,
    #[prop(default = None)] aspect_ratio: Prop<Option<f32>>,
    #[prop(default = Padding::ZERO)] padding: Prop<Padding>,
    #[prop(default = Align2::STRETCH)] align: Prop<Align2>,
    #[prop(default = ColorRole::None)] fill: Prop<ColorRole>,
    #[prop(default = Radius::None)] radius: Prop<Radius>,
    #[prop(default = CornerSet::ALL)] corners: Prop<CornerSet>,
    #[prop(default = None)] border: Prop<Option<Border>>,
    #[prop(default = false)] pointer: Prop<bool>,
    children: Option<Child>,
) -> NodeId {
    let theme = use_theme();
    let fill = fill_color(fill, theme);
    let outline = border_outline(border, theme);
    let radius = match (radius, corners) {
        (radius, Prop::Static(CornerSet::ALL)) => radius.map(Radius::corners),
        (radius, corners) => Prop::dynamic(move || corners.get().of(radius.get().corners())),
    };
    let width = resolved(width, Length::resolve);
    let height = resolved(height, Length::resolve);
    let padding = resolved(padding, Padding::resolve);
    let pointer = pointer.map(|pointer| if pointer { POINTER_REACH } else { 0.0 });
    match children {
        Some(child) => view! {
            <primitives::Frame width height aspect_ratio padding align fill radius outline pointer>
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
                pointer
            />
        },
    }
}
