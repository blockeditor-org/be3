use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_core::font::Overflow;
use beui_core::icons::Glyph;
use beui_macros::{component, view};

use crate::button::ButtonVariant;
use crate::context_menu::menu_style;
use crate::text::{Icon, Text};
use crate::theme::{ThemeStore, use_theme};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ButtonHandle, MenuButtonHandle, MenuItem};
use beui_core::base::TextAlign;
use beui_core::color::Color32;
use beui_core::icons::ICON_CHEVRON_DOWN;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const INSET: f32 = 2.0;
const GLYPH_SPACING: f32 = 6.0;
const LABEL_PADDING_HORIZONTAL: f32 = 14.0;
const ARROW_PADDING_HORIZONTAL: f32 = 4.0;
const PADDING_VERTICAL: f32 = 7.0;
const DIVIDER_WIDTH: f32 = 1.0;
const DIVIDER_INSET: f32 = 6.0;
const FOCUS_RING_OFFSET: f32 = 2.0;

#[component]
pub fn SplitButton(
    label: Prop<String>,
    #[prop(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = String::from("More options"))] menu_label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    items: Children<MenuItem>,
    on_click: ClickCallback,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let disabled = create_memo(move || disabled.get());
    let theme = use_theme();
    let fill = create_memo(move || variant.fill(&theme, disabled.get(), false, false));
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            border={(variant == ButtonVariant::Secondary).then_some(Border::DEFAULT)}
            radius=Radius::Md
            padding=INSET
        >
            <unstyled::SplitButton
                label
                glyph
                menu_label
                disabled
                items
                menu={menu_style()}
                main={move |handle: ButtonHandle| view! {
                    <MainFace handle variant />
                }}
                arrow={move |handle: MenuButtonHandle| view! {
                    <ArrowFace handle variant />
                }}
                on_click={on_click}
                on_select={on_select}
            />
        </Frame>
    }
}

#[component]
fn MainFace(handle: ButtonHandle, variant: ButtonVariant) -> NodeId {
    let ButtonHandle {
        hovered,
        active,
        focused,
        disabled,
        label,
        glyph,
        ..
    } = handle;
    let theme = use_theme();
    let fill = create_memo(move || {
        half_fill(&theme, variant, disabled.get(), hovered.get(), active.get())
    });
    let ink = create_memo(move || variant.label(disabled.get()));
    let has_glyph = create_memo(move || glyph.get().is_some());
    view! {
        <HalfFace fill focused side=Side::Leading padding=LABEL_PADDING_HORIZONTAL>
            <List direction=Direction::Horizontal align=Align::Center spacing=GLYPH_SPACING>
                <Show condition={has_glyph}>
                    <Text color={ink} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Show>
                <Text overflow=Overflow::Ellipsis color={ink} align=TextAlign::Center>{label}</Text>
            </List>
        </HalfFace>
    }
}

#[component]
fn ArrowFace(handle: MenuButtonHandle, variant: ButtonVariant) -> NodeId {
    let MenuButtonHandle {
        open,
        button:
            ButtonHandle {
                hovered,
                active,
                focused,
                disabled,
                label,
                ..
            },
    } = handle;
    let theme = use_theme();
    let fill = create_memo(move || {
        half_fill(
            &theme,
            variant,
            disabled.get(),
            hovered.get(),
            active.get() || open.get(),
        )
    });
    let ink = create_memo(move || variant.label(disabled.get()));
    let divider = create_memo(move || divider_color(&theme, variant, disabled.get()));
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame padding={Sides::vertical(DIVIDER_INSET)}>
                <Frame width=DIVIDER_WIDTH fill={ColorRole::custom(divider)} />
            </Frame>
            <Tooltip label disabled={open}>
                <HalfFace fill focused side=Side::Trailing padding=ARROW_PADDING_HORIZONTAL>
                    <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                        <Text color={ink} align=TextAlign::Center>
                            <Icon glyph=ICON_CHEVRON_DOWN />
                        </Text>
                    </List>
                </HalfFace>
            </Tooltip>
        </List>
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Leading,
    Trailing,
}

impl Side {
    fn corners(self, radius: f32) -> (f32, f32) {
        match self {
            Side::Leading => (radius, 0.0),
            Side::Trailing => (0.0, radius),
        }
    }
}

#[component]
fn HalfFace(
    fill: Memo<Color32>,
    focused: ReadSignal<bool>,
    side: Side,
    padding: f32,
    children: beui_view::reactive::Child,
) -> NodeId {
    let (ring_left, ring_right) = side.corners(Radius::Md.px());
    let (fill_left, fill_right) = side.corners(Radius::Md.px() - INSET);
    view! {
        <Frame
            border={focus_border(focused, Border::FOCUS.offset(FOCUS_RING_OFFSET))}
            radius={Radius::Custom(Corners::new(ring_left, ring_right, ring_right, ring_left))}
        >
            <Frame
                fill={ColorRole::custom(fill)}
                radius={Radius::Custom(Corners::new(fill_left, fill_right, fill_right, fill_left))}
                padding={Sides::symmetric(padding, PADDING_VERTICAL)}
            >
                {children}
            </Frame>
        </Frame>
    }
}

fn half_fill(
    theme: &ThemeStore,
    variant: ButtonVariant,
    disabled: bool,
    hovered: bool,
    active: bool,
) -> Color32 {
    if disabled || !(hovered || active) {
        return Color32::TRANSPARENT;
    }
    variant.fill(theme, false, hovered, active)
}

fn divider_color(theme: &ThemeStore, variant: ButtonVariant, disabled: bool) -> Color32 {
    match (variant, disabled) {
        (ButtonVariant::Primary, false) => {
            let [red, green, blue, _] = theme.on_accent.get().to_array();
            Color32::from_rgba_unmultiplied(red, green, blue, 110)
        }
        _ => theme.border.get(),
    }
}
