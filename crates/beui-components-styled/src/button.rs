use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};
use beui_view::types::Overflow;
use beui_view::types::icons::Glyph;

use crate::focus_ring::FocusRing;
use crate::list::List;
use crate::progress::{INLINE_PROGRESS_WIDTH, Progress};
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::tokens::{Length, Padding, Size, Space, Stroke};
use crate::tooltip::Tooltip;
use beui_components_unstyled::{self as unstyled, Picture};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Align2;
use beui_view::types::Image;
use beui_view::types::{ItemSize, TextAlign};

const GLYPH_SPACING: Space = Space::Sm;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum ButtonSize {
    Small,
    Compact,
    #[default]
    Regular,
    Toolbar,
}

impl ButtonSize {
    fn padding(self, icon_only: bool) -> Padding {
        let (horizontal, vertical) = match (self, icon_only) {
            (ButtonSize::Small, true) => (Space::Xs2, Space::Xs2),
            (ButtonSize::Small, false) => (Space::Sm, Space::Xs2),
            (ButtonSize::Compact, true) => (Space::Sm, Space::Sm),
            (ButtonSize::Compact, false) => (Space::Lg, Space::Sm),
            (ButtonSize::Regular, true) => (Space::Md, Space::Md),
            (ButtonSize::Regular, false) => (Space::Xl, Space::Md),
            (ButtonSize::Toolbar, true) => (Space::None, Space::None),
            (ButtonSize::Toolbar, false) => (Space::Md, Space::None),
        };
        Padding::symmetric(horizontal, vertical)
    }

    fn width(self, icon_only: bool) -> Length {
        match (self, icon_only) {
            (ButtonSize::Toolbar, true) => Length::Size(Size::Toolbar),
            _ => Length::INTRINSIC,
        }
    }

    fn height(self) -> Length {
        match self {
            ButtonSize::Toolbar => Length::Size(Size::Toolbar),
            _ => Length::INTRINSIC,
        }
    }

    fn align(self) -> Align2 {
        match self {
            ButtonSize::Toolbar => Align2::CENTER,
            _ => Align2::STRETCH,
        }
    }

    fn radius(self) -> Radius {
        match self {
            ButtonSize::Small | ButtonSize::Regular => Radius::Md,
            ButtonSize::Compact | ButtonSize::Toolbar => Radius::Tab,
        }
    }

    fn text(self) -> TextSize {
        match self {
            ButtonSize::Small => TextSize::Xs,
            _ => TextSize::Base,
        }
    }

    fn focus_ring_offset(self) -> Stroke {
        match self {
            ButtonSize::Small => Stroke::Hairline,
            _ => Stroke::Focus,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Danger,
    Ghost,
}

impl ButtonVariant {
    pub fn fill(self, disabled: bool, hovered: bool, active: bool) -> ColorRole {
        if disabled {
            return match self {
                ButtonVariant::Primary => ColorRole::AccentSoft,
                ButtonVariant::Danger => ColorRole::DangerSoft,
                ButtonVariant::Secondary => ColorRole::Surface,
                ButtonVariant::Ghost => ColorRole::None,
            };
        }
        match (self, hovered, active) {
            (ButtonVariant::Primary, _, true) => ColorRole::AccentActive,
            (ButtonVariant::Primary, true, false) => ColorRole::AccentHover,
            (ButtonVariant::Primary, false, false) => ColorRole::Accent,
            (ButtonVariant::Danger, _, true) => ColorRole::DangerFillActive,
            (ButtonVariant::Danger, true, false) => ColorRole::DangerFillHover,
            (ButtonVariant::Danger, false, false) => ColorRole::DangerFill,
            (ButtonVariant::Secondary, _, true) => ColorRole::Pressed,
            (ButtonVariant::Secondary, true, false) => ColorRole::Hover,
            (ButtonVariant::Secondary, false, false) => ColorRole::Surface,
            (ButtonVariant::Ghost, _, true) => ColorRole::Pressed,
            (ButtonVariant::Ghost, true, false) => ColorRole::Hover,
            (ButtonVariant::Ghost, false, false) => ColorRole::None,
        }
    }

    pub fn label(self, disabled: bool) -> TextColor {
        if disabled {
            return TextColor::Muted;
        }
        match self {
            ButtonVariant::Primary => TextColor::OnAccent,
            ButtonVariant::Danger => TextColor::OnDanger,
            ButtonVariant::Secondary | ButtonVariant::Ghost => TextColor::Default,
        }
    }
}

#[component]
pub fn Button(
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = ButtonVariant::Secondary)] variant: Prop<ButtonVariant>,
    #[prop(default = ButtonSize::Regular)] size: ButtonSize,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] busy: Prop<bool>,
    #[prop(default = false)] wide: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = true)] press_focus: Prop<bool>,
    #[prop(default = true)] tab_stop: Prop<bool>,
    #[prop(default = true)] revealed: Prop<bool>,
    action: Option<Action>,
    on_click: ClickCallback,
    on_focus_change: Callback<bool>,
) -> NodeId {
    let busy = create_memo(move || busy.get());
    let disabled = create_memo(move || disabled.get() || busy.get());
    view! {
        <unstyled::Button
            label
            glyph
            icon_only
            action
            disabled
            focused
            capture_presses
            press_focus
            tab_stop
            on_click={on_click}
            on_focus_change={on_focus_change}
        >|handle|
            <ButtonFace handle variant size busy wide revealed />
        </unstyled::Button>
    }
}

#[component]
pub fn ButtonFace(
    handle: unstyled::ButtonHandle,
    #[prop(default = ButtonVariant::Secondary)] variant: Prop<ButtonVariant>,
    #[prop(default = ButtonSize::Regular)] size: ButtonSize,
    #[prop(default = None)] trailing_glyph: Prop<Option<Glyph>>,
    #[prop(default = None)] image: Prop<Option<Image>>,
    #[prop(default = false)] pressed: Prop<bool>,
    #[prop(default = false)] busy: Prop<bool>,
    #[prop(default = false)] wide: Prop<bool>,
    #[prop(default = true)] revealed: Prop<bool>,
    #[prop(default = false)] placeholder: Prop<bool>,
    leading: Option<Child>,
) -> NodeId {
    let unstyled::ButtonHandle {
        hovered,
        active,
        focused,
        disabled,
        label,
        glyph,
        icon_only,
        tooltip,
    } = handle;
    let revealed = create_memo(move || revealed.get());
    let fill_color = create_memo(move || {
        let held = active.get() || pressed.get();
        match revealed.get() {
            true => variant.get().fill(disabled.get(), hovered.get(), held),
            false => ColorRole::None,
        }
    });
    let label_color = create_memo(move || match (revealed.get(), placeholder.get()) {
        (true, true) => TextColor::Muted,
        (true, false) => variant.get().label(disabled.get()),
        (false, _) => TextColor::None,
    });
    let border = create_memo(move || {
        (revealed.get() && variant.get() == ButtonVariant::Secondary).then_some(Border::DEFAULT)
    });
    let image = create_memo(move || image.get());
    let pictured = create_memo(move || image.with(Option::is_some));
    let has_leading = leading.is_some();
    let leading = leading.unwrap_or_else(|| {
        view! {
            <Frame />
        }
    });
    let has_glyph = create_memo(move || !has_leading && !pictured.get() && glyph.get().is_some());
    let trailing = create_memo(move || trailing_glyph.get());
    let has_trailing = create_memo(move || trailing.get().is_some());
    let labelled = create_memo(move || !icon_only.get());
    let busy = create_memo(move || busy.get());
    let justify = create_memo(move || match wide.get() {
        true => Justify::Center,
        false => Justify::Start,
    });
    let padding = create_memo(move || size.padding(icon_only.get()));
    let width = create_memo(move || size.width(icon_only.get()));
    let text = size.text();
    let picture = Size::Custom(text.icon_px());
    view! {
        <Tooltip label={tooltip}>
            <FocusRing focused radius={size.radius()} offset={size.focus_ring_offset()}>
                <Frame
                    fill={fill_color}
                    border={border}
                    radius={size.radius()}
                    width={width}
                    height={size.height()}
                    align={size.align()}
                    padding={padding}
                >
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        justify={justify}
                        spacing=GLYPH_SPACING
                    >
                        <Show condition={busy}>
                            <Progress width=INLINE_PROGRESS_WIDTH />
                        </Show>
                        <Show condition=has_leading>{leading}</Show>
                        <Show condition={pictured}>
                            <Frame width=picture height=picture>
                                <Picture image />
                            </Frame>
                        </Show>
                        <Show condition={has_glyph}>
                            <Text size=text color={label_color} align=TextAlign::Center>
                                <Icon glyph={glyph} />
                            </Text>
                        </Show>
                        <Show condition={labelled}>
                            <Text
                                @sizing={ItemSize::Intrinsic.shrink(1.0)}
                                size=text
                                color={label_color}
                                align=TextAlign::Center
                                overflow=Overflow::Ellipsis
                            >
                                {label}
                            </Text>
                        </Show>
                        <Show condition={has_trailing}>
                            <Text size=text color={label_color} align=TextAlign::Center>
                                <Icon glyph={trailing} />
                            </Text>
                        </Show>
                    </List>
                </Frame>
            </FocusRing>
        </Tooltip>
    }
}
