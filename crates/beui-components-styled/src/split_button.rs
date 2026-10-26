use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_view::types::Overflow;
use beui_view::types::icons::Glyph;
use beui_macros::{component, view};

use crate::button::ButtonVariant;
use crate::context_menu::menu_style;
use crate::text::{Icon, Text};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ButtonHandle, MenuItem, PopupTriggerHandle};
use beui_view::types::TextAlign;
use beui_view::types::icons::ICON_CHEVRON_DOWN;
use beui_view::reactive::NodeId;
use beui_view::primitives::Fade;

use crate::list::List;
use crate::tokens::{CornerSet, Padding, Size, Space, Stroke};
use beui_view::reactive::*;

const INSET: Space = Space::Xs2;
const GLYPH_SPACING: Space = Space::Sm;
const LABEL_PADDING_HORIZONTAL: Space = Space::Lg;
const ARROW_PADDING_HORIZONTAL: Space = Space::Xs;
const PADDING_VERTICAL: Space = Space::Sm;
const DIVIDER_INSET: Space = Space::Sm;
const DIVIDER_ON_ACCENT_OPACITY: f32 = 0.43;

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
    let fill = create_memo(move || variant.fill(disabled.get(), false, false));
    view! {
        <Frame
            fill={fill}
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
                arrow={move |handle: PopupTriggerHandle| view! {
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
    let fill = create_memo(move || half_fill(variant, disabled.get(), hovered.get(), active.get()));
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
fn ArrowFace(handle: PopupTriggerHandle, variant: ButtonVariant) -> NodeId {
    let PopupTriggerHandle {
        open,
        button:
            ButtonHandle {
                hovered,
                active,
                focused,
                disabled,
                tooltip,
                ..
            },
    } = handle;
    let fill = create_memo(move || {
        half_fill(
            variant,
            disabled.get(),
            hovered.get(),
            active.get() || open.get(),
        )
    });
    let ink = create_memo(move || variant.label(disabled.get()));
    let divider = create_memo(move || divider(variant, disabled.get()));
    let divider_fill = create_memo(move || divider.get().0);
    let divider_opacity = create_memo(move || divider.get().1);
    view! {
        <List direction=Direction::Horizontal spacing=Space::None>
            <Frame padding={Padding::vertical(DIVIDER_INSET)}>
                <Fade opacity={divider_opacity}>
                    <Frame width={Size::Custom(Stroke::Hairline.px())} fill={divider_fill} />
                </Fade>
            </Frame>
            <Tooltip label={tooltip}>
                <HalfFace fill focused side=Side::Trailing padding=ARROW_PADDING_HORIZONTAL>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::None>
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
    fn corners(self) -> CornerSet {
        match self {
            Side::Leading => CornerSet::LEFT,
            Side::Trailing => CornerSet::RIGHT,
        }
    }
}

#[component]
fn HalfFace(
    fill: Memo<ColorRole>,
    focused: ReadSignal<bool>,
    side: Side,
    padding: Space,
    children: beui_view::reactive::Child,
) -> NodeId {
    view! {
        <Frame
            border={focus_border(focused, Border::FOCUS.offset(Stroke::Focus))}
            radius=Radius::Md
            corners={side.corners()}
        >
            <Frame
                fill={fill}
                radius=Radius::Sm
                corners={side.corners()}
                padding={Padding::symmetric(padding, PADDING_VERTICAL)}
            >
                {children}
            </Frame>
        </Frame>
    }
}

fn half_fill(variant: ButtonVariant, disabled: bool, hovered: bool, active: bool) -> ColorRole {
    if disabled || !(hovered || active) {
        return ColorRole::None;
    }
    variant.fill(false, hovered, active)
}

fn divider(variant: ButtonVariant, disabled: bool) -> (ColorRole, f32) {
    match (variant, disabled) {
        (ButtonVariant::Primary, false) => (ColorRole::OnAccent, DIVIDER_ON_ACCENT_OPACITY),
        _ => (ColorRole::Border, 1.0),
    }
}
