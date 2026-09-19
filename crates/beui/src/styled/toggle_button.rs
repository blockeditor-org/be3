use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::color::Color32;
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    Align, Callback, Direction, Frame, List, Prop, Show, Text, clone, create_memo,
};
use crate::styled::text::IconSized;
use crate::styled::theme::{FONT_BODY, ICON_SIZE, RADIUS, ThemeStore, use_theme};
use crate::styled::tooltip::Tooltip;
use crate::unstyled;
use crate::unstyled::{Toggle, ToggleHandle};

#[component]
pub fn ToggleButton(
    label: Prop<String>,
    pressed: Prop<bool>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_change: Callback<bool>,
) -> NodeId {
    let label_text = create_memo(clone!(label -> move || label.get()));
    let icon_only = create_memo(move || icon_only.get());
    let named = create_memo(clone!(icon_only -> move || !icon_only.get()));
    let accessibility = create_memo({
        let label_text = label_text.clone();
        move || {
            let label = label_text.get();
            let mut node = Node::new(Role::Button);
            node.set_label(label);
            node
        }
    });

    view! {
        <Toggle
            checked={pressed}
            disabled={disabled}
            accessibility
            on_change={move |pressed| on_change.call(pressed)}
        >
            {move |handle| view! {
                <Tooltip label={label} disabled={named}>
                    <ToggleButtonFace
                        handle
                        label={label_text}
                        glyph={glyph}
                        icon_only={icon_only}
                    />
                </Tooltip>
            }}
        </Toggle>
    }
}

#[component]
fn ToggleButtonFace(
    handle: ToggleHandle,
    label: Prop<String>,
    glyph: Prop<String>,
    icon_only: Prop<bool>,
) -> NodeId {
    let ToggleHandle {
        checked,
        hovered,
        focused,
        disabled,
        ..
    } = handle;
    let theme = use_theme();
    let glyph_text = create_memo(move || glyph.get());
    let has_glyph = create_memo(clone!(glyph_text -> move || !glyph_text.get().is_empty()));
    let label_text = create_memo(move || label.get());
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
        <Frame
            outline={theme.accent.clone()}
            outline_width=2.0
            radius=RADIUS
            outline_offset=3.0
            outline_visible={focused}
        >
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
                        <IconSized glyph={glyph_text} font_size=ICON_SIZE color={icon_color} />
                    </Show>
                    <Show condition={named}>
                        <Text string={label_text} font_size=FONT_BODY color={text_color} />
                    </Show>
                </List>
            </Frame>
        </Frame>
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
