use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate as unstyled;
use crate::context_menu::MenuStyle;
use crate::menu::MenuItem;
use crate::menu_button::{MenuButtonHandle, MenuSheet};
use crate::ButtonHandle;
use beui_core::base::Direction;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, Children, ClickCallback, List, Prop, Render, clone, component_accessibility,
    create_memo,
};

#[component]
pub fn SplitButton(
    label: Prop<String>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = String::from("More options"))] menu_label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    items: Children<MenuItem>,
    #[prop(default = MenuStyle::default())] menu: MenuStyle,
    sheet: Option<MenuSheet>,
    main: Render<ButtonHandle>,
    arrow: Render<MenuButtonHandle>,
    on_click: ClickCallback,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let label = create_memo(move || label.get());
    let disabled = create_memo(move || disabled.get());
    component_accessibility(create_memo(clone!(label -> move || {
        let mut node = Node::new(Role::Group);
        node.set_label(label.get());
        node
    })));
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <unstyled::Button
                label
                glyph
                disabled={disabled.clone()}
                on_click={move || on_click.call()}
                content={main}
            />
            <unstyled::MenuButton
                items
                label={menu_label}
                disabled
                menu
                sheet
                trigger={arrow}
                on_select={move |path: Vec<usize>| on_select.call(path)}
            />
        </List>
    }
}
