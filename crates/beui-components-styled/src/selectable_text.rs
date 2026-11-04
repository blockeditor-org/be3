use beui_macros::{component, view};

use crate::ContextMenu;
use crate::text_area::text_area_geometry;
use crate::theme::use_theme;
use beui_components_unstyled::MenuItem;
use beui_view::actions::Chord;
use beui_view::components::selectable::{
    COPY_KEY, SELECT_ALL_KEY, Selectable, SelectableState, copy_selection, select_all,
};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Rect;

/// Makes the plain text nodes under it selectable together: a drag selects from
/// one text to another in tree order (and scrolls the scroll around it past its
/// edge), a double click selects a word and a triple click a whole text,
/// Shift+click extends, and a click clears. `copy_key` (Ctrl+C) copies the
/// selection, joining texts on one line with a space and lines with a line
/// break; `select_all_key` (Ctrl+A) selects everything under it. `None` turns
/// either off, as the terminal does to leave them to the shell; the secondary
/// click menu shows whichever keys are set. It is not a Tab stop: the keys also
/// reach it from a focused control inside it.
#[component]
pub fn SelectableText(
    #[prop(default = ComponentRef::new())] api: ComponentRef<SelectableState>,
    children: Child,
    #[prop(default = 0.0)] child_grow: Prop<f32>,
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
            child_grow
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
            <Selectable
                state
                color
                handle_color
                caret_width={text_area_geometry().caret_width}
                copy_key
                select_all_key
                on_toolbar={toolbar_at}
            >
                {children}
            </Selectable>
        </ContextMenu>
    }
}
