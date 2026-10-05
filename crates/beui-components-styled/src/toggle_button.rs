use accesskit::Role;
use beui_macros::{component, view};

use crate::focus_ring::FocusRing;
use crate::text::Icon;
use crate::theme::{FONT_BODY, RADIUS, ThemeStore, use_theme};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{Toggle, ToggleHandle};
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Action, Align, Callback, Direction, Frame, List, Memo, Prop, Show, Text, clone, create_memo,
};

#[component]
pub fn ToggleButton(
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] pressed: Prop<bool>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    action: Option<Action>,
    on_change: Callback<bool>,
) -> NodeId {
    let icon_only = create_memo(move || icon_only.get());
    let named = create_memo(clone!(icon_only -> move || !icon_only.get()));
    view! {
        <Toggle
            checked={pressed}
            label
            glyph
            role=Role::Button
            action
            disabled
            on_change={move |pressed| on_change.call(pressed)}
        >
            {move |handle: ToggleHandle| view! {
                <Tooltip label={handle.tooltip.clone()} disabled={named}>
                    <ToggleButtonFace handle icon_only />
                </Tooltip>
            }}
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
    let has_glyph = create_memo(clone!(glyph -> move || !glyph.get().is_empty()));
    let named = create_memo(move || !icon_only.get());
    let text_color = create_memo(clone!(theme disabled -> move || match disabled.get() {
        true => theme.text_muted.get(),
        false => theme.text.get(),
    }));
    let icon_color = text_color.clone();
    let fill_color = create_memo(
        clone!(checked theme -> move || fill_for(&theme, checked.get(), hovered.get())),
    );
    let border_color = create_memo(clone!(theme -> move || {
        let theme = theme.get();
        if checked.get() {
            theme.accent
        } else {
            theme.border
        }
    }));

    view! {
        <FocusRing focused offset=3.0>
            <Frame
                color={fill_color}
                outline={border_color}
                outline_width=1.0
                radius=RADIUS
                outline_visible=true
                padding_horizontal=14.0
                padding_vertical=8.0
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                    <Show condition={has_glyph}>
                        <Icon glyph={glyph.clone()} color={icon_color.clone()} />
                    </Show>
                    <Show condition={named}>
                        <Text string={label.clone()} font_size=FONT_BODY color={text_color.clone()} />
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

pub fn toggle_button_pressed(document: &Document, button: NodeId) -> bool {
    unstyled::toggle_checked(document, button).get()
}
