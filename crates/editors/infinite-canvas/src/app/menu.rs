use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{Child, ItemSize, component, use_context, view};
use block_editor_beui::beui::styled::ContextMenu;
use block_editor_beui::beui::unstyled::MenuItem;

use super::actions::CanvasActions;

#[component]
pub(crate) fn CanvasMenu(disabled: bool, children: Child) -> NodeId {
    let actions = use_context::<CanvasActions>().expect("the canvas provides its actions");
    view! {
        <ContextMenu
            items={view! {
                <MenuItem label="Add">
                    <MenuItem label="Rectangle" action={actions.add_rectangle} />
                    <MenuItem label="Line" action={actions.add_line} />
                    <MenuItem label="Text" action={actions.add_text} />
                    <MenuItem label="Freehand" action={actions.pen} />
                    <MenuItem label="Image…" action={actions.add_image} />
                    <MenuItem label="Block…" action={actions.add_block} />
                </MenuItem>
                <MenuItem action={actions.edit} />
                <MenuItem action={actions.toggle_mode} />
                <MenuItem action={actions.fit_selection} />
                <MenuItem action={actions.cut} />
                <MenuItem action={actions.copy} />
                <MenuItem action={actions.paste} />
                <MenuItem action={actions.duplicate} />
                <MenuItem action={actions.delete} />
                <MenuItem action={actions.group} />
                <MenuItem action={actions.ungroup} />
                <MenuItem action={actions.lock} />
                <MenuItem action={actions.unlock} />
                <MenuItem action={actions.front} />
                <MenuItem action={actions.forward} />
                <MenuItem action={actions.backward} />
                <MenuItem action={actions.back} />
                <MenuItem action={actions.select_all} />
                <MenuItem action={actions.invert} />
            }}
            child_size=ItemSize::Percent(100.0)
            disabled={disabled}
        >
            {children}
        </ContextMenu>
    }
}
