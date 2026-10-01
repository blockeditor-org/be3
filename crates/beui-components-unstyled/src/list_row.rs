use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::button::{Button, ButtonHandle};
use beui_core::input::{Key, KeyPress, PointerPress};
use beui_core::node::NodeId;
use beui_view::reactive::{ClickCallback, Prop, Render, clone, create_memo};

#[component]
pub fn ListRow(
    #[prop(default = false)] selected: Prop<bool>,
    #[prop(default = true)] activates: bool,
    #[prop(children)] content: Render<ButtonHandle>,
    on_click: ClickCallback,
    on_activate: ClickCallback,
) -> NodeId {
    let double_click = on_activate.clone();
    let selected = create_memo(move || selected.get());
    let accessibility = create_memo(clone!(selected -> move || {
        let mut node = Node::new(Role::Button);
        node.set_selected(selected.get());
        node
    }));
    view! {
        <Button
            accessibility
            on_click={move || on_click.call()}
            on_click_at={move |press: PointerPress| {
                if press.clicks >= 2 {
                    double_click.call();
                }
            }}
            on_key={move |press: KeyPress| {
                if !activates || press.key != Key::Enter {
                    return false;
                }
                if press.pressed {
                    on_activate.call();
                }
                true
            }}
            content
        />
    }
}
