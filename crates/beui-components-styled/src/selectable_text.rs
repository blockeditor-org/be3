use beui_macros::{component, view};

use crate::ContextMenu;
use crate::theme::use_theme;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{MenuItem, SelectableState, copy_selection, select_all};
use beui_core::base::ItemSize;
use beui_core::geometry::Pos2;
use beui_core::node::NodeId;
use beui_view::reactive::{Child, Prop, clone, create_memo, create_signal, set_component_state};

#[component]
pub fn SelectableText(
    children: Child,
    #[prop(default = ItemSize::Intrinsic)] child_size: Prop<ItemSize>,
) -> NodeId {
    let theme = use_theme();
    let state = SelectableState::default();
    set_component_state(state.clone());
    let color = create_memo(clone!(theme -> move || theme.accent_soft.get()));
    let handle_color = create_memo(clone!(theme -> move || theme.accent.get()));
    let (menu_at, set_menu_at) = create_signal(None::<Pos2>);
    let close_menu = set_menu_at.clone();
    let chosen = state.clone();
    view! {
        <ContextMenu
            child_size
            open_at={menu_at}
            open_at_focuses=false
            on_close={move || close_menu.set(None)}
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
            <unstyled::Selectable
                state
                color
                handle_color
                on_menu={move |at: Pos2| set_menu_at.set(Some(at))}
            >
                {children}
            </unstyled::Selectable>
        </ContextMenu>
    }
}
