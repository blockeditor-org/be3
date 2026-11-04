use beui_macros::{component, view};
use beui_view::types::icons::Glyph;

use crate::button::{ButtonFace, ButtonSize, ButtonVariant};
use crate::context_menu::menu_style;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{MenuItem, PopupTriggerHandle};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Image;
use beui_view::types::Rect;
use beui_view::types::ScrollGesture;
use beui_view::types::icons::ICON_CHEVRON_DOWN;

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
    #[prop(default = false)] placeholder: Prop<bool>,
    #[prop(default = true)] click_opens: Prop<bool>,
    #[prop(default = false)] secondary_opens: Prop<bool>,
    #[prop(default = false)] open: Binding<bool>,
    #[prop(default = None)] bounds: Prop<Option<Rect>>,
    leading: Option<Child>,
    items: Children<MenuItem>,
    on_click: ClickCallback,
    on_secondary_click: ClickCallback,
    on_middle_click: ClickCallback,
    on_scroll: Callback<ScrollGesture>,
    #[prop(default = Callback::default())] on_select: Callback<Vec<usize>>,
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
            bounds
            menu={menu_style()}
            trigger={move |handle: PopupTriggerHandle| view! {
                <PopupButtonFace
                    handle
                    variant
                    size
                    image={image}
                    arrow={arrow}
                    placeholder
                    leading
                />
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
    #[prop(default = false)] placeholder: Prop<bool>,
    leading: Option<Child>,
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
            placeholder
            leading
        />
    }
}
