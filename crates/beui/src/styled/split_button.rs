use accesskit::{HasPopup, Node, Role};
use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;
use crate::icons::ICON_ARROW_DROP_DOWN;
use crate::node::NodeId;
use crate::reactive::{
    Align, Callback, Children, ClickCallback, Direction, Frame, List, Memo, Prop, ReadSignal, Show,
    Text, clone, component_accessibility, create_memo, focus_ring,
};
use crate::styled::button::ButtonVariant;
use crate::styled::context_menu::{menu_panel, menu_row};
use crate::styled::text::Icon;
use crate::styled::theme::{BORDER_WIDTH, FONT_BODY, RADIUS, ThemeStore, use_theme};
use crate::styled::tooltip::Tooltip;
use crate::unstyled;
use crate::unstyled::{ButtonHandle, MenuButtonHandle, MenuItem};

const INSET: f32 = 2.0;
const GLYPH_SPACING: f32 = 6.0;
const LABEL_PADDING_HORIZONTAL: f32 = 14.0;
const ARROW_PADDING_HORIZONTAL: f32 = 4.0;
const PADDING_VERTICAL: f32 = 7.0;
const DIVIDER_WIDTH: f32 = 1.0;
const DIVIDER_INSET: f32 = 6.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
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
    let label = create_memo(move || label.get());
    let menu_label = create_memo(move || menu_label.get());
    let disabled = create_memo(move || disabled.get());
    component_accessibility(create_memo(clone!(label -> move || {
        let mut node = Node::new(Role::Group);
        node.set_label(label.get());
        node
    })));
    let main_accessibility = create_memo(clone!(label -> move || {
        let mut node = Node::new(Role::Button);
        node.set_label(label.get());
        node
    }));
    let menu_accessibility = create_memo(clone!(menu_label -> move || {
        let mut node = Node::new(Role::Button);
        node.set_label(menu_label.get());
        node.set_has_popup(HasPopup::Menu);
        node
    }));
    let theme = use_theme();
    let fill = create_memo(clone!(theme disabled -> move || {
        variant.fill(&theme, disabled.get(), false, false)
    }));
    let divider = create_memo(clone!(theme disabled -> move || {
        divider_color(&theme, variant, disabled.get())
    }));
    let (main_disabled, arrow_disabled) = (disabled.clone(), disabled.clone());
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
            <List direction=Direction::Horizontal spacing=0.0>
                <unstyled::Button
                    disabled={disabled.clone()}
                    accessibility={main_accessibility}
                    on_click={move || on_click.call()}
                    content={move |handle: ButtonHandle| view! {
                        <MainFace handle variant label glyph disabled={main_disabled} />
                    }}
                />
                <Frame padding_vertical=DIVIDER_INSET>
                    <Frame width=DIVIDER_WIDTH color={divider} />
                </Frame>
                <unstyled::MenuButton
                    items
                    disabled
                    accessibility={menu_accessibility}
                    row={menu_row()}
                    panel={menu_panel()}
                    trigger={move |handle: MenuButtonHandle| view! {
                        <ArrowFace handle variant label={menu_label} disabled={arrow_disabled} />
                    }}
                    on_select={move |path: Vec<usize>| on_select.call(path)}
                />
            </List>
        </Frame>
    }
}

#[component]
fn MainFace(
    handle: ButtonHandle,
    variant: ButtonVariant,
    label: Memo<String>,
    glyph: Prop<String>,
    disabled: Memo<bool>,
) -> NodeId {
    let ButtonHandle {
        hovered,
        active,
        focused,
    } = handle;
    let theme = use_theme();
    let fill = create_memo(clone!(theme disabled -> move || {
        half_fill(&theme, variant, disabled.get(), hovered.get(), active.get())
    }));
    let ink = create_memo(clone!(theme disabled -> move || variant.label(&theme, disabled.get())));
    let glyph = create_memo(move || glyph.get());
    let has_glyph = create_memo(clone!(glyph -> move || !glyph.get().is_empty()));
    let icon_ink = ink.clone();
    view! {
        <HalfFace fill focused padding=LABEL_PADDING_HORIZONTAL>
            <List direction=Direction::Horizontal align=Align::Center spacing=GLYPH_SPACING>
                <Show condition={has_glyph}>
                    <Icon glyph color={icon_ink} />
                </Show>
                <Text string={label} font_size=FONT_BODY color={ink} align=TextAlign::Center />
            </List>
        </HalfFace>
    }
}

#[component]
fn ArrowFace(
    handle: MenuButtonHandle,
    variant: ButtonVariant,
    label: Memo<String>,
    disabled: Memo<bool>,
) -> NodeId {
    let MenuButtonHandle {
        open,
        hovered,
        active,
        focused,
    } = handle;
    let theme = use_theme();
    let fill = create_memo(clone!(theme disabled open -> move || {
        half_fill(&theme, variant, disabled.get(), hovered.get(), active.get() || open.get())
    }));
    let ink = create_memo(clone!(theme disabled -> move || variant.label(&theme, disabled.get())));
    view! {
        <Tooltip label disabled={open}>
            <HalfFace fill focused padding=ARROW_PADDING_HORIZONTAL>
                <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                    <Icon glyph={ICON_ARROW_DROP_DOWN.to_owned()} color={ink} />
                </List>
            </HalfFace>
        </Tooltip>
    }
}

#[component]
fn HalfFace(
    fill: Memo<Color32>,
    focused: ReadSignal<bool>,
    padding: f32,
    children: crate::reactive::Child,
) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius={RADIUS}
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focus_ring(focused)}
        >
            <Frame
                color={fill}
                radius={RADIUS - INSET as u8}
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
