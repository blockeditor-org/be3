use beui_macros::{component, view};

use crate::icons::ICON_ARROW_DROP_DOWN;
use crate::node::NodeId;
use crate::reactive::{Callback, Children, Prop, create_memo};
use crate::styled::button::{ButtonFace, ButtonVariant};
use crate::styled::context_menu::{menu_panel, menu_row};
use crate::unstyled;
use crate::unstyled::{MenuButtonHandle, MenuItem};

#[component]
pub fn MenuButton(
    label: Prop<String>,
    #[prop(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = true)] arrow: bool,
    items: Children<MenuItem>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let disabled = create_memo(move || disabled.get());
    let face = disabled.clone();
    view! {
        <unstyled::MenuButton
            items
            disabled
            row={menu_row()}
            panel={menu_panel()}
            trigger={move |handle: MenuButtonHandle| {
                let MenuButtonHandle {
                    hovered,
                    active,
                    focused,
                    ..
                } = handle;
                let button = unstyled::ButtonHandle {
                    hovered,
                    active,
                    focused,
                };
                let trailing = match arrow {
                    true => ICON_ARROW_DROP_DOWN.to_owned(),
                    false => String::new(),
                };
                view! {
                    <ButtonFace
                        handle={button}
                        variant
                        label
                        glyph
                        trailing_glyph={trailing}
                        disabled={face.clone()}
                    />
                }
            }}
            on_select={move |path: Vec<usize>| on_select.call(path)}
        />
    }
}
