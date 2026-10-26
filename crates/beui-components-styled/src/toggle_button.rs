use crate::frame::{Border, ColorRole, Frame, Radius};
use accesskit::Role;
use beui_core::icons::Glyph;
use beui_macros::{component, view};

use crate::focus_ring::FocusRing;
use crate::text::{Icon, Text, TextColor};
use crate::tooltip::Tooltip;
use beui_components_unstyled::{Toggle, ToggleHandle};
use beui_core::font::{Overflow, TextAlign};
use beui_core::node::NodeId;

use crate::list::List;
use crate::tokens::{Padding, Space, Stroke};
use beui_view::reactive::*;

#[component]
pub fn ToggleButton(
    #[prop(default = ComponentRef::new())] api: ComponentRef<crate::ToggleApi>,

    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] pressed: Binding<bool>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    action: Option<Action>,
) -> NodeId {
    view! {
        <Toggle
            api
            checked={pressed}
            label
            glyph
            icon_only
            role=Role::Button
            action
            disabled
        >|handle: ToggleHandle|
            <ToggleButtonFace handle />
        </Toggle>
    }
}

#[component]
fn ToggleButtonFace(handle: ToggleHandle) -> NodeId {
    let ToggleHandle {
        checked,
        hovered,
        focused,
        disabled,
        label,
        glyph,
        icon_only,
        tooltip,
        ..
    } = handle;
    let has_glyph = create_memo(move || glyph.get().is_some());
    let named = create_memo(move || !icon_only.get());
    let padding = create_memo(move || match icon_only.get() {
        true => Padding::all(Space::Md),
        false => Padding::symmetric(Space::Lg, Space::Md),
    });
    let text_color = create_memo(move || match disabled.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    let fill_color = create_memo(move || fill_for(checked.get(), hovered.get()));
    let border = create_memo(move || {
        Some(Border::new(match checked.get() {
            true => ColorRole::Accent,
            false => ColorRole::Border,
        }))
    });

    view! {
        <Tooltip label={tooltip}>
            <FocusRing focused offset=Stroke::Focus>
                <Frame fill={fill_color} border={border} radius=Radius::Md padding={padding}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                        <Show condition={has_glyph}>
                            <Text color={text_color} align=TextAlign::Center>
                                <Icon glyph={glyph} />
                            </Text>
                        </Show>
                        <Show condition={named}>
                            <Text overflow=Overflow::Ellipsis color={text_color}>{label}</Text>
                        </Show>
                    </List>
                </Frame>
            </FocusRing>
        </Tooltip>
    }
}

fn fill_for(pressed: bool, hovered: bool) -> ColorRole {
    if pressed {
        ColorRole::AccentSoft
    } else if hovered {
        ColorRole::Hover
    } else {
        ColorRole::Surface
    }
}
