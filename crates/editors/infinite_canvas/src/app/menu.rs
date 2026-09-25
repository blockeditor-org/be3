use std::rc::Rc;

use block_editor_plugin::be_block::canvas::CanvasLayerMove;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::{Child, ItemSize, clone, component, create_memo, view};
use block_editor_plugin::beui::styled::ContextMenu;
use block_editor_plugin::beui::unstyled::MenuItem;

use super::state::{CanvasCommand, CanvasState, Tool};

const LAYERS: [CanvasLayerMove; 4] = [
    CanvasLayerMove::BringToFront,
    CanvasLayerMove::ForwardOne,
    CanvasLayerMove::BackOne,
    CanvasLayerMove::SendToBack,
];

#[component]
pub(crate) fn CanvasMenu(state: Rc<CanvasState>, disabled: bool, children: Child) -> NodeId {
    let empty = create_memo(clone!(state -> move || state.selection.get().is_empty()));
    let single = create_memo(clone!(state -> move || state.selection.get().len() != 1));
    let blocked = create_memo(clone!(state -> move || {
        let selected = state.selected_entities();
        !matches!(
            selected.as_slice(),
            [entity]
                if matches!(
                    entity.kind,
                    block_editor_plugin::be_block::canvas::CanvasEntityKind::Block { .. }
                        | block_editor_plugin::be_block::canvas::CanvasEntityKind::DirectEditor { .. }
                )
        )
    }));
    let ungrouped = create_memo(clone!(state -> move || {
        !state
            .selected_entities()
            .iter()
            .any(|entity| entity.group_id.is_some())
    }));
    let grouped = create_memo(clone!(state -> move || !state.selection_can_group()));
    let lockable = create_memo(clone!(state -> move || {
        !state.selected_entities().iter().any(|entity| !entity.locked)
    }));
    let unlockable = create_memo(clone!(state -> move || {
        !state.selected_entities().iter().any(|entity| entity.locked)
    }));
    let front = create_memo(clone!(state -> move || !state.can_reorder(LAYERS[0])));
    let forward = create_memo(clone!(state -> move || !state.can_reorder(LAYERS[1])));
    let backward = create_memo(clone!(state -> move || !state.can_reorder(LAYERS[2])));
    let back = create_memo(clone!(state -> move || !state.can_reorder(LAYERS[3])));
    let chosen = clone!(state -> move |path: Vec<usize>| chose(&state, &path));
    let items = view! {
        <MenuItem label="Add">
            <MenuItem label="Rectangle" />
            <MenuItem label="Line" />
            <MenuItem label="Text" />
            <MenuItem label="Freehand" />
            <MenuItem label="Image…" />
            <MenuItem label="Block…" />
        </MenuItem>
        <MenuItem label="Open / edit" disabled={single.clone()} />
        <MenuItem label="Toggle preview / direct editor" disabled={blocked} />
        <MenuItem label="Fit selection" disabled={empty.clone()} />
        <MenuItem label="Cut" disabled={empty.clone()} />
        <MenuItem label="Copy" disabled={empty.clone()} />
        <MenuItem label="Paste" />
        <MenuItem label="Duplicate" disabled={empty.clone()} />
        <MenuItem label="Delete" disabled={empty} />
        <MenuItem label="Group" disabled={grouped} />
        <MenuItem label="Ungroup" disabled={ungrouped} />
        <MenuItem label="Lock" disabled={lockable} />
        <MenuItem label="Unlock" disabled={unlockable} />
        <MenuItem label="Bring to front" disabled={front} />
        <MenuItem label="Forward" disabled={forward} />
        <MenuItem label="Backward" disabled={backward} />
        <MenuItem label="Send to back" disabled={back} />
        <MenuItem label="Select all" />
        <MenuItem label="Invert selection" />
    };
    view! {
        <ContextMenu
            items={items}
            child_size=ItemSize::Percent(100.0)
            disabled={disabled}
            on_select={chosen}
        >
            {children}
        </ContextMenu>
    }
}

fn chose(state: &Rc<CanvasState>, path: &[usize]) {
    let at = state
        .context_position()
        .unwrap_or_else(|| state.viewport_center());
    match path {
        [0, entry] => match entry {
            0 => state.add_at(Tool::Rectangle, at),
            1 => state.add_at(Tool::Line, at),
            2 => state.add_at(Tool::Text, at),
            3 => state.set_tool(Tool::Pen),
            4 => state.open_image_picker(Some(at)),
            _ => state.open_block_picker(Some(at)),
        },
        [1] => state.edit_selected(),
        [2] => state.toggle_selected_block_mode(),
        [3] => state.request_fit_selection(),
        [4] => state.run(CanvasCommand::Cut),
        [5] => state.run(CanvasCommand::Copy),
        [6] => state.run(CanvasCommand::Paste),
        [7] => state.run(CanvasCommand::Duplicate),
        [8] => state.run(CanvasCommand::Delete),
        [9] => state.run(CanvasCommand::Group),
        [10] => state.run(CanvasCommand::Ungroup),
        [11] => state.run(CanvasCommand::Lock),
        [12] => state.run(CanvasCommand::Unlock),
        [13..=16] => state.reorder(LAYERS[path[0] - 13]),
        [17] => state.run(CanvasCommand::SelectAll),
        [18] => state.run(CanvasCommand::InvertSelection),
        _ => {}
    }
}
