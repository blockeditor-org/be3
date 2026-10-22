use beui_macros::{component, view};

use crate::ContextMenu;
use crate::theme::use_theme;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{MenuItem, SelectableState, copy_selection, select_all};
use beui_core::base::ItemSize;
use beui_core::geometry::Rect;
use beui_core::node::NodeId;
use beui_view::reactive::*;

#[component]
pub fn SelectableText(
    #[prop(default = ComponentRef::new())] api: ComponentRef<SelectableState>,
    children: Child,
    #[prop(default = ItemSize::Intrinsic)] child_size: Prop<ItemSize>,
) -> NodeId {
    let theme = use_theme();
    let state = SelectableState::default();
    api.fill(state);
    let color = create_memo(move || theme.accent_soft.get());
    let handle_color = create_memo(move || theme.accent.get());
    let toolbar_at = create_signal(None::<Rect>);
    view! {
        <ContextMenu
            child_size
            selection=true
            toolbar_at
            opens_on_hold=false
            on_toolbar_close={move || toolbar_at.set(None)}
            items={view! {
                <MenuItem label="Copy" />
                <MenuItem label="Select All" />
            }}
            on_select={move |path: Vec<usize>| match path.first() {
                Some(0) => copy_selection(state),
                Some(1) => select_all(state),
                _ => {}
            }}
        >
            <unstyled::Selectable state color handle_color on_toolbar={toolbar_at}>
                {children}
            </unstyled::Selectable>
        </ContextMenu>
    }
}
