use beui_macros::{component, view};

use crate::button::ButtonVariant;
use crate::context_menu::menu_style;
use crate::text::Icon;
use crate::theme::{BORDER_WIDTH, FOCUS_RING_WIDTH, FONT_BODY, RADIUS, ThemeStore, use_theme};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ButtonHandle, MenuButtonHandle, MenuItem};
use beui_core::base::TextAlign;
use beui_core::color::Color32;
use beui_core::icons::ICON_ARROW_DROP_DOWN;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, Callback, Children, ClickCallback, Direction, Frame, List, Memo, Prop, ReadSignal, Show,
    Text, clone, create_memo, focus_ring,
};

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
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = String::from("More options"))] menu_label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    items: Children<MenuItem>,
    on_click: ClickCallback,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let disabled = create_memo(move || disabled.get());
    let theme = use_theme();
    let fill = create_memo(clone!(theme disabled -> move || {
        variant.fill(&theme, disabled.get(), false, false)
    }));
    view! {
        <Frame
            color={fill}
            outline={theme.border.clone()}
            outline_width=BORDER_WIDTH
            outline_visible={variant == ButtonVariant::Secondary}
            radius=RADIUS
            padding_horizontal=INSET
            padding_vertical=INSET
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
                on_click={move || on_click.call()}
                on_select={move |path: Vec<usize>| on_select.call(path)}
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
    let fill = create_memo(clone!(theme disabled -> move || {
        half_fill(&theme, variant, disabled.get(), hovered.get(), active.get())
    }));
    let ink = create_memo(clone!(theme -> move || variant.label(&theme, disabled.get())));
    let has_glyph = create_memo(clone!(glyph -> move || !glyph.get().is_empty()));
    let icon_ink = ink.clone();
    view! {
        <HalfFace fill focused side=Side::Leading padding=LABEL_PADDING_HORIZONTAL>
            <List direction=Direction::Horizontal align=Align::Center spacing=GLYPH_SPACING>
                <Show condition={has_glyph}>
                    <Icon glyph={glyph.clone()} color={icon_ink.clone()} />
                </Show>
                <Text string={label} font_size=FONT_BODY color={ink} align=TextAlign::Center />
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
    let fill = create_memo(clone!(theme disabled open -> move || {
        half_fill(&theme, variant, disabled.get(), hovered.get(), active.get() || open.get())
    }));
    let ink = create_memo(clone!(theme disabled -> move || variant.label(&theme, disabled.get())));
    let divider = create_memo(clone!(theme -> move || {
        divider_color(&theme, variant, disabled.get())
    }));
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame padding_vertical=DIVIDER_INSET>
                <Frame width=DIVIDER_WIDTH color={divider} />
            </Frame>
            <Tooltip label disabled={open}>
                <HalfFace fill focused side=Side::Trailing padding=ARROW_PADDING_HORIZONTAL>
                    <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                        <Icon glyph={ICON_ARROW_DROP_DOWN.to_owned()} color={ink} />
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
    fn corners(self, radius: u8) -> (u8, u8) {
        match self {
            Side::Leading => (radius, 0),
            Side::Trailing => (0, radius),
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
    let theme = use_theme();
    let (ring_left, ring_right) = side.corners(RADIUS);
    let (fill_left, fill_right) = side.corners(RADIUS - INSET as u8);
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius_top_left=ring_left
            radius_bottom_left=ring_left
            radius_top_right=ring_right
            radius_bottom_right=ring_right
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focus_ring(focused)}
        >
            <Frame
                color={fill}
                radius_top_left=fill_left
                radius_bottom_left=fill_left
                radius_top_right=fill_right
                radius_bottom_right=fill_right
                padding_horizontal=padding
                padding_vertical=PADDING_VERTICAL
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
