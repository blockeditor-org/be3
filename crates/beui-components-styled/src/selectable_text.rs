use beui_macros::{component, view};

use crate::ContextMenu;
use crate::theme::use_theme;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{MenuItem, SelectableState, copy_selection, select_all};
use beui_core::base::ItemSize;
use beui_core::node::NodeId;
use beui_view::reactive::{Child, Prop, clone, create_memo, set_component_state};

#[component]
pub fn SelectableText(
    children: Child,
    #[prop(default = ItemSize::Intrinsic)] child_size: Prop<ItemSize>,
) -> NodeId {
    let theme = use_theme();
    let state = SelectableState::default();
    set_component_state(state.clone());
    let color = create_memo(clone!(theme -> move || theme.accent_soft.get()));
    let chosen = state.clone();
    view! {
        <ContextMenu
            child_size
            items={view! {
                <MenuItem label="Copy" />
                <MenuItem label="Select All" />
            }}
            on_select={move |path: Vec<usize>| match path.first() {
                Some(0) => copy_selection(&chosen),
                Some(1) => select_all(&chosen),
                _ => {}
            }}
        >
            <unstyled::Selectable state color>{children}</unstyled::Selectable>
        </ContextMenu>
    }
}
