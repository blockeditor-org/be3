use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::button::{Button, ButtonHandle};
use beui_core::input::{Key, KeyPress, PointerPress};
use beui_core::node::NodeId;
use beui_view::reactive::*;

#[component]
pub fn ListRow(
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] selected: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = true)] activates: bool,
    #[prop(children)] content: Render<ButtonHandle>,
    on_click: ClickCallback,
    on_activate: ClickCallback,
) -> NodeId {
    let selected = create_memo(move || selected.get());
    let accessibility = create_memo(move || {
        let mut node = Node::new(Role::Button);
        node.set_selected(selected.get());
        node
    });
    view! {
        <Button
            label
            accessibility
            disabled
            on_click={on_click}
            on_click_at={move |press: PointerPress| {
                if press.clicks >= 2 {
                    on_activate.call();
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
