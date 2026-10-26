use beui_core::icons::Glyph;
use beui_macros::{component, view};

use crate::button::{ButtonFace, ButtonSize, ButtonVariant};
use crate::context_menu::menu_style;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{MenuItem, PopupTriggerHandle};
use beui_core::icons::ICON_CHEVRON_DOWN;
use beui_core::image::Image;
use beui_core::input::ScrollGesture;
use beui_core::node::NodeId;
use beui_view::reactive::*;

#[component]
pub fn MenuButton(
    label: Prop<String>,
    #[prop(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[prop(default = ButtonSize::Regular)] size: ButtonSize,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = None)] image: Prop<Option<Image>>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = true)] arrow: Prop<bool>,
    #[prop(default = true)] click_opens: Prop<bool>,
    #[prop(default = false)] secondary_opens: Prop<bool>,
    #[prop(default = false)] open: Binding<bool>,
    items: Children<MenuItem>,
    on_click: ClickCallback,
    on_secondary_click: ClickCallback,
    on_middle_click: ClickCallback,
    on_scroll: Callback<ScrollGesture>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let image = create_memo(move || image.get());
    let arrow = create_memo(move || arrow.get());
    view! {
        <unstyled::MenuButton
            items
            open
            label
            glyph
            icon_only
            disabled
            click_opens
            secondary_opens
            menu={menu_style()}
            trigger={move |handle: PopupTriggerHandle| view! {
                <PopupButtonFace handle variant size image={image} arrow={arrow} />
            }}
            on_click={on_click}
            on_secondary_click={on_secondary_click}
            on_middle_click={on_middle_click}
            on_scroll={on_scroll}
            on_select={on_select}
        />
    }
}

#[component]
pub(crate) fn PopupButtonFace(
    handle: PopupTriggerHandle,
    variant: ButtonVariant,
    size: ButtonSize,
    #[prop(default = None)] image: Prop<Option<Image>>,
    #[prop(default = false)] arrow: Prop<bool>,
) -> NodeId {
    let PopupTriggerHandle { open, button } = handle;
    let icon_only = button.icon_only;
    let trailing = create_memo(move || match arrow.get() && !icon_only.get() {
        true => Some(ICON_CHEVRON_DOWN),
        false => None,
    });
    view! {
        <ButtonFace
            handle={button}
            variant
            size
            image
            trailing_glyph={trailing}
            pressed={create_memo(move || open.get())}
        />
    }
}
