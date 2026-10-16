use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::unstyled::MenuItem;
use block_editor_beui::block_ui::BlockTypes;
use block_editor_beui::{BlockInfo, BlockParent};
use uuid::Uuid;

use super::dialogs::OpenDialog;
use super::tab::TabItem;
use super::workspace::Workspace;

#[derive(Clone, Copy)]
pub(crate) enum Action {
    Open,
    Picker,
    MoveToMyFiles,
    SetParent(BlockParent),
    Rename,
    Share,
    Unlink,
    Delete,
}

pub(crate) struct Permissions {
    pub(crate) add: bool,
    pub(crate) edit: bool,
    pub(crate) delete: bool,
    pub(crate) unlink: Result<(), &'static str>,
    pub(crate) is_reference: bool,
    pub(crate) source: Option<BlockParent>,
}

pub(crate) fn action_for(path: &[usize]) -> Option<Action> {
    match path {
        [0] => Some(Action::Open),
        [1] => Some(Action::Picker),
        [2, 0] => Some(Action::MoveToMyFiles),
        [2, 1] => Some(Action::SetParent(BlockParent::Detached)),
        [3] => Some(Action::Rename),
        [4] => Some(Action::Share),
        [5] => Some(Action::Unlink),
        [6] => Some(Action::Delete),
        _ => None,
    }
}

pub(crate) fn unlink_permission(
    workspace: &Workspace,
    container: Option<Uuid>,
) -> Result<(), &'static str> {
    let container_type = container.and_then(|container| workspace.known_type(container));
    let types = workspace.types();
    match (container, container_type) {
        (Some(_), Some(block_type)) if !types.child_edits(block_type).replace => {
            Err("This container doesn't support replacing a reference")
        }
        (Some(container), Some(_)) if !workspace.can_edit(container) => {
            Err("You don't have permission to edit this container")
        }
        (Some(_), Some(_)) => Ok(()),
        _ => Err("Loading…"),
    }
}

fn can_delete_from(workspace: &Workspace, source: BlockParent) -> bool {
    match source {
        BlockParent::Root | BlockParent::Detached => true,
        BlockParent::Block(id) => {
            let types = workspace.types();
            workspace
                .known_type(id)
                .is_some_and(|block_type| types.child_edits(block_type).delete)
                && workspace.can_edit(id)
        }
    }
}

pub(crate) fn permissions(
    workspace: &Workspace,
    reference: &BlockInfo,
    containing: Option<Uuid>,
) -> Permissions {
    let can_edit = workspace.can_edit(reference.id);
    let source = containing.map(BlockParent::Block).or(reference.parent);
    let is_reference =
        containing.is_some_and(|id| reference.parent != Some(BlockParent::Block(id)));
    let types = workspace.types();
    Permissions {
        add: types.child_edits(reference.block_type).add && can_edit,
        edit: can_edit,
        delete: source.is_some_and(|source| {
            source != BlockParent::Detached && can_delete_from(workspace, source)
        }) && (is_reference || can_edit),
        unlink: unlink_permission(workspace, containing),
        is_reference,
        source,
    }
}

pub(crate) fn apply(
    workspace: &Workspace,
    reference: &BlockInfo,
    containing: Option<Uuid>,
    action: Action,
) {
    let permissions = permissions(workspace, reference, containing);
    match action {
        Action::Open => workspace.open(TabItem::new(reference.id, reference.block_type), None),
        Action::Picker => workspace.open_picker(reference.id),
        Action::MoveToMyFiles => {
            if let Some(files) = workspace.my_files()
                && let Some(source) = permissions.source
            {
                workspace.host().move_block(
                    reference.id,
                    reference.block_type,
                    source,
                    files,
                    permissions.is_reference,
                );
            }
        }
        Action::SetParent(parent) => workspace.blocks().set_parent(reference.id, parent),
        Action::Rename => workspace.open_dialog(OpenDialog::Rename(reference.id)),
        Action::Share => workspace.open_dialog(OpenDialog::Share(reference.id)),
        Action::Unlink => {
            if let Some(BlockParent::Block(container)) = permissions.source {
                workspace.host().unlink_block(reference.id, container);
            }
        }
        Action::Delete => {
            if let Some(source) = permissions.source {
                workspace.host().delete_block(
                    reference.id,
                    reference.block_type,
                    source,
                    permissions.is_reference,
                );
            }
        }
    }
}

#[component]
pub(crate) fn ReferenceMenuItem(
    workspace: Workspace,
    reference: Memo<Option<BlockInfo>>,
    containing: Memo<Option<Uuid>>,
) -> MenuItem {
    let label = create_memo(move || {
        let types = workspace.types();
        reference.with(|reference| {
            reference.as_ref().map_or_else(
                || "Untitled".to_owned(),
                |reference| reference.label(types.as_ref()).name,
            )
        })
    });
    let rights = create_memo(move || {
        let containing = containing.get();
        reference.with(|reference| {
            reference.as_ref().map(|reference| {
                let permissions = permissions(&workspace, reference, containing);
                (
                    !permissions.add,
                    !permissions.edit,
                    !permissions.delete,
                    !(permissions.is_reference && permissions.unlink.is_ok()),
                    reference.parent,
                    permissions.is_reference,
                )
            })
        })
    });
    let add = create_memo(move || rights.get().is_none_or(|rights| rights.0));
    let edit = create_memo(move || rights.get().is_none_or(|rights| rights.1));
    let delete = create_memo(move || rights.get().is_none_or(|rights| rights.2));
    let unlink = create_memo(move || rights.get().is_none_or(|rights| rights.3));
    let files = create_memo(move || workspace.my_files());
    let rooted = create_memo(move || {
        let files = files.get();
        delete.get()
            || files.is_none()
            || rights.get().is_none_or(|rights| {
                rights.4.is_none_or(|parent| {
                    parent == BlockParent::Root
                        || files.is_none_or(|files| parent == BlockParent::Block(files))
                })
            })
    });
    let orphaned = create_memo(move || {
        edit.get()
            || rights.get().is_none_or(|rights| {
                rights
                    .4
                    .is_none_or(|parent| parent == BlockParent::Detached)
            })
    });
    let delete_label = create_memo(move || match rights.get().is_some_and(|rights| rights.5) {
        true => "Remove link".to_owned(),
        false => "Delete".to_owned(),
    });
    view! {
        <MenuItem label={label}>
            <MenuItem label="Open" />
            <MenuItem label="Add" disabled={add} />
            <MenuItem label="Set parent" disabled={edit}>
                <MenuItem label="My files" disabled={rooted} />
                <MenuItem label="Orphaned" disabled={orphaned} />
            </MenuItem>
            <MenuItem label="Rename" disabled={edit} />
            <MenuItem label="Share" disabled={edit} />
            <MenuItem label="Unlink" disabled={unlink} />
            <MenuItem label={delete_label} disabled={delete} />
        </MenuItem>
    }
}
