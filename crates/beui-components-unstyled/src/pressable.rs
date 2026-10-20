use accesskit::{Node, Role};
use beui_macros::{component, view};

use beui_core::input::CursorIcon;

use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

#[component]
pub fn Pressable(
    children: Option<Child>,
    #[prop(default = true)] enabled: Prop<bool>,
    on_click: ClickCallback,
    on_focus_change: Callback<bool>,
    on_hover_change: Callback<bool>,
    on_active_change: Callback<bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let enabled = create_memo(move || enabled.get());
    let key_active = create_signal(false);
    component_accessibility(accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::Button))));
    let click = move || {
        if untrack(|| enabled.get()) {
            on_click.call();
        }
    };
    let key_click = click;

    view! {
        <Interactive
            focusable=true
            tab_stop={enabled}
            on_focus_change={on_focus_change}
            on_activate_change={move |pressed| key_active.set(pressed)}
            on_activate={key_click}
            cursor=CursorIcon::PointingHand
            key_active
            on_click={click}
            on_hover_change={on_hover_change}
            on_active_change={on_active_change}
            children={children}
        />
    }
}
