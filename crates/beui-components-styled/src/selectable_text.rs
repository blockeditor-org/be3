use beui_macros::{component, view};

use crate::ContextMenu;
use crate::text_area::text_area_geometry;
use crate::theme::use_theme;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::selectable::{COPY_KEY, SELECT_ALL_KEY};
use beui_components_unstyled::{MenuItem, SelectableState, copy_selection, select_all};
use beui_core::base::ItemSize;
use beui_core::geometry::Rect;
use beui_core::node::NodeId;
use beui_view::actions::Chord;
use beui_view::reactive::*;

#[component]
pub fn SelectableText(
    #[prop(default = ComponentRef::new())] api: ComponentRef<SelectableState>,
    children: Child,
    #[prop(default = ItemSize::Intrinsic)] child_size: Prop<ItemSize>,
    #[prop(default = Some(COPY_KEY))] copy_key: Prop<Option<Chord>>,
    #[prop(default = Some(SELECT_ALL_KEY))] select_all_key: Prop<Option<Chord>>,
) -> NodeId {
    let theme = use_theme();
    let state = SelectableState::default();
    api.fill(state);
    let color = create_memo(move || theme.accent_soft.get());
    let handle_color = create_memo(move || theme.accent.get());
    let toolbar_at = create_signal(None::<Rect>);
    let copy_label = create_memo(move || copy_key.get().map(Chord::label).unwrap_or_default());
    let select_all_label =
        create_memo(move || select_all_key.get().map(Chord::label).unwrap_or_default());
    view! {
        <ContextMenu
            child_size
            selection=true
            toolbar_at
            opens_on_hold=false
            on_toolbar_close={move || toolbar_at.set(None)}
            items={view! {
                <MenuItem label="Copy" shortcut={copy_label} />
                <MenuItem label="Select All" shortcut={select_all_label} />
            }}
            on_select={move |path: Vec<usize>| match path.first() {
                Some(0) => copy_selection(state),
                Some(1) => select_all(state),
                _ => {}
            }}
        >
            <unstyled::Selectable
                state
                color
                handle_color
                caret_width={text_area_geometry().caret_width}
                copy_key
                select_all_key
                on_toolbar={toolbar_at}
            >
                {children}
            </unstyled::Selectable>
        </ContextMenu>
    }
}
