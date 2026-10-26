use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::font::Overflow;
use beui_macros::{component, view};

use crate::focus_ring::FocusRing;
use beui_core::base::TextAlign;
use beui_core::color::Color32;

use crate::text::{Text, TextColor, TextSize};
use crate::theme::{ThemeStore, use_theme};
use beui_components_unstyled::{Toggle, ToggleHandle};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const BOX_SIZE: f32 = 18.0;
const MARK_SIZE: f32 = 10.0;
const MARK_RADIUS: f32 = 2.0;
const SPACING: f32 = 10.0;
const FOCUS_RING_OFFSET: f32 = 4.0;

#[component]
pub fn Checkbox(
    #[prop(default = ComponentRef::new())] api: ComponentRef<crate::ToggleApi>,

    #[prop(default = String::new())] label: Prop<String>,
    checked: Binding<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = true)] tab_stop: Prop<bool>,
    #[prop(default = true)] press_focus: Prop<bool>,
    #[prop(default = TextSize::Base)] size: Prop<TextSize>,
) -> NodeId {
    view! {
        <Toggle
            api
            checked
            label
            disabled
            capture_presses
            tab_stop
            press_focus
        >|handle: ToggleHandle|
            <CheckboxFace handle size />
        </Toggle>
    }
}

#[component]
fn CheckboxFace(handle: ToggleHandle, size: Prop<TextSize>) -> NodeId {
    let ToggleHandle {
        checked,
        hovered,
        focused,
        disabled,
        label,
        ..
    } = handle;
    let theme = use_theme();
    let scale = create_memo(move || size.get().px() / TextSize::Base.px());
    let side = create_memo(move || Extent::px(BOX_SIZE * scale.get()));
    let mark = create_memo(move || Extent::px(MARK_SIZE * scale.get()));
    let box_radius = create_memo(move || Radius::custom(Radius::Sm.px() * scale.get()));
    let mark_radius = create_memo(move || Radius::custom(MARK_RADIUS * scale.get()));
    let fill_color =
        create_memo(move || box_fill(&theme, disabled.get(), checked.get(), hovered.get()));
    let border_visible = create_memo(move || !checked.get());
    let box_border = theme.border;
    let mark_color = theme.on_accent;
    let labeled = create_memo(move || !label.get().is_empty());
    let label_color = create_memo(move || match disabled.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });

    view! {
        <FocusRing focused offset=FOCUS_RING_OFFSET>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Frame
                    width=side
                    height=side
                    fill={ColorRole::custom(fill_color)}
                    border={create_memo(move || border_visible.get().then(|| Border::new(ColorRole::Custom(box_border.get()))))}
                    radius=box_radius
                    align=Align::Center
                >
                    <Layers>
                        <Show condition={checked}>
                            <Frame
                                width=mark
                                height=mark
                                fill={ColorRole::custom(mark_color)}
                                radius=mark_radius
                            />
                        </Show>
                    </Layers>
                </Frame>
                <Show condition={labeled}>
                    <Text
                        overflow=Overflow::Ellipsis
                        @sizing=ItemSize::Percent(100.0)
                        color={label_color}
                        align=TextAlign::Start
                    >
                        {label}
                    </Text>
                </Show>
            </List>
        </FocusRing>
    }
}

fn box_fill(theme: &ThemeStore, disabled: bool, checked: bool, hovered: bool) -> Color32 {
    if disabled {
        return match checked {
            true => theme.accent_soft.get(),
            false => theme.surface.get(),
        };
    }
    match (checked, hovered) {
        (true, false) => theme.accent.get(),
        (true, true) => theme.accent_hover.get(),
        (false, false) => theme.surface_raised.get(),
        (false, true) => theme.pressed.get(),
    }
}
