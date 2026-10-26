use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::font::Overflow;
use beui_macros::{component, view};

use crate::focus_ring::FocusRing;
use beui_core::base::TextAlign;

use crate::text::{Text, TextColor, TextSize};
use crate::list::List;
use crate::tokens::{Size, Space, Stroke};
use beui_components_unstyled::{Toggle, ToggleHandle};
use beui_core::node::NodeId;
use beui_view::primitives::Layers;
use beui_view::reactive::*;

const BOX_SIZE: f32 = 18.0;
const MARK_SIZE: f32 = 10.0;
const SPACING: Space = Space::Md;

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
    let scale = create_memo(move || size.get().px() / TextSize::Base.px());
    let side = create_memo(move || Size::Custom(BOX_SIZE * scale.get()));
    let mark = create_memo(move || Size::Custom(MARK_SIZE * scale.get()));
    let fill_color = create_memo(move || box_fill(disabled.get(), checked.get(), hovered.get()));
    let border = create_memo(move || (!checked.get()).then_some(Border::DEFAULT));
    let labeled = create_memo(move || !label.get().is_empty());
    let label_color = create_memo(move || match disabled.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });

    view! {
        <FocusRing focused offset=Stroke::Focus>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Frame
                    width=side
                    height=side
                    fill={fill_color}
                    border={border}
                    radius=Radius::Sm
                    align=Align::Center
                >
                    <Layers>
                        <Show condition={checked}>
                            <Frame
                                width=mark
                                height=mark
                                fill=ColorRole::OnAccent
                                radius=Radius::Xs
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

fn box_fill(disabled: bool, checked: bool, hovered: bool) -> ColorRole {
    if disabled {
        return match checked {
            true => ColorRole::AccentSoft,
            false => ColorRole::Surface,
        };
    }
    match (checked, hovered) {
        (true, false) => ColorRole::Accent,
        (true, true) => ColorRole::AccentHover,
        (false, false) => ColorRole::SurfaceRaised,
        (false, true) => ColorRole::Pressed,
    }
}
