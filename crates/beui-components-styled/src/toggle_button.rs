use crate::frame::{Border, ColorRole, Frame, Radius};
use accesskit::Role;
use beui_core::font::Overflow;
use beui_core::icons::Glyph;
use beui_macros::{component, view};

use crate::focus_ring::FocusRing;
use crate::text::{Icon, Text, TextColor};
use crate::theme::{ThemeStore, use_theme};
use crate::tooltip::Tooltip;
use beui_components_unstyled::{Toggle, ToggleHandle};
use beui_core::color::Color32;
use beui_core::font::TextAlign;
use beui_core::node::NodeId;
use beui_view::primitives::*;
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
    let icon_only = create_memo(move || icon_only.get());
    let named = create_memo(move || !icon_only.get());
    view! {
        <Toggle
            api
            checked={pressed}
            label
            glyph
            role=Role::Button
            action
            disabled
        >|handle: ToggleHandle|
            <Tooltip label={handle.tooltip} disabled={named}>
                <ToggleButtonFace handle icon_only />
            </Tooltip>
        </Toggle>
    }
}

#[component]
fn ToggleButtonFace(handle: ToggleHandle, icon_only: Memo<bool>) -> NodeId {
    let ToggleHandle {
        checked,
        hovered,
        focused,
        disabled,
        label,
        glyph,
        ..
    } = handle;
    let theme = use_theme();
    let has_glyph = create_memo(move || glyph.get().is_some());
    let named = create_memo(move || !icon_only.get());
    let padding = create_memo(move || match icon_only.get() {
        true => Sides::symmetric(8.0, 8.0),
        false => Sides::symmetric(14.0, 8.0),
    });
    let text_color = create_memo(move || match disabled.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    let fill_color = create_memo(move || fill_for(&theme, checked.get(), hovered.get()));
    let border_color = create_memo(move || {
        let theme = theme.get();
        if checked.get() {
            theme.accent
        } else {
            theme.border
        }
    });

    view! {
        <FocusRing focused offset=3.0>
            <Frame
                fill={ColorRole::custom(fill_color)}
                border={create_memo(move || Some(Border::new(ColorRole::Custom(border_color.get())).width(1.0)))}
                radius=Radius::Md
                padding={padding}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
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
    }
}

fn fill_for(theme: &ThemeStore, pressed: bool, hovered: bool) -> Color32 {
    if pressed {
        theme.accent_soft.get()
    } else if hovered {
        theme.hover.get()
    } else {
        theme.surface.get()
    }
}
