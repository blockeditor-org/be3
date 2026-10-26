use accesskit::{Node, Role};
use beui_macros::{component, view};
use beui_view::types::icons::Glyph;

use crate as unstyled;
use crate::ButtonHandle;
use crate::menu::MenuItem;
use crate::menu_popup::MenuStyle;
use crate::popup_trigger::PopupTriggerHandle;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Direction;

#[component]
pub fn SplitButton(
    label: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = String::from("More options"))] menu_label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    items: Children<MenuItem>,
    menu: MenuStyle,
    main: Render<ButtonHandle>,
    arrow: Render<PopupTriggerHandle>,
    on_click: ClickCallback,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let label = create_memo(move || label.get());
    let disabled = create_memo(move || disabled.get());
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Group);
        node.set_label(label.get());
        node
    }));
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <unstyled::Button label glyph disabled={disabled} on_click={on_click} content={main} />
            <unstyled::MenuButton
                items
                label={menu_label}
                icon_only=true
                disabled
                menu
                trigger={arrow}
                on_select={on_select}
            />
        </List>
    }
}
