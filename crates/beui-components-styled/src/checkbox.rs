use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::focus_ring::FocusRing;
use beui_core::base::TextAlign;
use beui_core::color::Color32;

use crate::text::{Text, TextColor};
use crate::theme::{ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
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
    #[prop(default = String::new())] label: Prop<String>,
    checked: Binding<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = true)] tab_stop: Prop<bool>,
    #[prop(default = true)] press_focus: Prop<bool>,
) -> NodeId {
    view! {
        <Toggle checked label disabled capture_presses tab_stop press_focus>|handle: ToggleHandle|
            <CheckboxFace handle />
        </Toggle>
    }
}

#[component]
fn CheckboxFace(handle: ToggleHandle) -> NodeId {
    let ToggleHandle {
        checked,
        hovered,
        focused,
        disabled,
        label,
        ..
    } = handle;
    let theme = use_theme();
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
                    width=BOX_SIZE
                    height=BOX_SIZE
                    fill={ColorRole::custom(fill_color)}
                    border={create_memo(move || border_visible.get().then(|| Border::new(ColorRole::Custom(box_border.get()))))}
                    radius=Radius::Sm
                    align=Align::Center
                >
                    <Layers>
                        <Show condition={checked}>
                            <Frame
                                width=MARK_SIZE
                                height=MARK_SIZE
                                fill={ColorRole::custom(mark_color)}
                                radius={Radius::custom(MARK_RADIUS)}
                            />
                        </Show>
                    </Layers>
                </Frame>
                <Show condition={labeled}>
                    <Text
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

pub fn checkbox_checked(tree: &impl Inspect, checkbox: NodeId) -> bool {
    unstyled::toggle_checked(tree, checkbox).get()
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
