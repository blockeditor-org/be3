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
    on_state_change: Callback<InteractionState>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let enabled = create_memo(move || enabled.get());
    component_accessibility(accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::Button))));
    let click = move || {
        if enabled.get_untracked() {
            on_click.call();
        }
    };

    view! {
        <Interactive
            focusable=true
            tab_stop={enabled}
            on_activate={click}
            cursor=CursorIcon::PointingHand
            on_click={click}
            on_state_change={on_state_change}
            children={children}
        />
    }
}
