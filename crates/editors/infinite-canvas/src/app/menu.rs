use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::MenuItem;

use super::actions::CanvasActions;

#[component]
pub(crate) fn CanvasMenu(disabled: bool, children: Child) -> NodeId {
    let actions = use_context::<CanvasActions>().expect("the canvas provides its actions");
    let grid = actions.grid.clone();
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
                <MenuItem
                    action={actions.delete}
                    row_test_id={"infinite-canvas.menu.delete".to_owned()}
                />
                <MenuItem action={actions.group} />
                <MenuItem action={actions.ungroup} />
                <MenuItem action={actions.lock} />
                <MenuItem action={actions.unlock} />
                <MenuItem action={actions.front} />
                <MenuItem action={actions.forward} />
                <MenuItem action={actions.backward} />
                <MenuItem action={actions.back} />
                <MenuItem label="Grid">
                    <MenuItem action={grid.shown.clone()} checked={ticked(&grid.shown)} />
                    <MenuItem
                        label="Small"
                        action={grid.small.clone()}
                        checked={ticked(&grid.small)}
                    />
                    <MenuItem
                        label="Medium"
                        action={grid.medium.clone()}
                        checked={ticked(&grid.medium)}
                    />
                    <MenuItem
                        label="Large"
                        action={grid.large.clone()}
                        checked={ticked(&grid.large)}
                    />
                </MenuItem>
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

fn ticked(action: &Action) -> Memo<Option<bool>> {
    let checked = action.checked();
    create_memo(move || checked.map(|checked| checked.get()))
}
