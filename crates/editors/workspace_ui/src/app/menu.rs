use std::rc::Rc;

use block_editor_beui::BlockSource;
use block_editor_beui::beui::reactive::{Memo, clone, component, create_memo, view};
use block_editor_beui::beui::unstyled::MenuItem;
use block_editor_beui::block_ui::BlockTypes;
use block_editor_beui::{BlockInfo, BlockParent};
use uuid::Uuid;

use super::tab::TabItem;
use super::workspace::Workspace;

#[derive(Clone, Copy)]
pub(crate) enum Action {
    Open,
    Picker,
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
    pub(crate) source: BlockSource,
}

pub(crate) fn action_for(path: &[usize]) -> Option<Action> {
    match path {
        [0] => Some(Action::Open),
        [1] => Some(Action::Picker),
        [2, 0] => Some(Action::SetParent(BlockParent::Root)),
        [2, 1] => Some(Action::SetParent(BlockParent::Detached)),
        [3] => Some(Action::Rename),
        [4] => Some(Action::Share),
        [5] => Some(Action::Unlink),
        [6] => Some(Action::Delete),
        _ => None,
    }
}

pub(crate) fn source_of(parent: BlockParent) -> BlockSource {
    match parent {
        BlockParent::Root => BlockSource::Root,
        BlockParent::Detached => BlockSource::Orphaned,
        BlockParent::Block(id) => BlockSource::Block(id),
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

fn can_delete_from(workspace: &Workspace, source: BlockSource) -> bool {
    match source {
        BlockSource::Root | BlockSource::Orphaned => true,
        BlockSource::Block(id) => {
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
    let source = containing.map_or_else(|| source_of(reference.parent), BlockSource::Block);
    let is_reference = containing.is_some_and(|id| reference.parent != BlockParent::Block(id));
    let types = workspace.types();
    Permissions {
        add: types.child_edits(reference.block_type).add && can_edit,
        edit: can_edit,
        delete: source != BlockSource::Orphaned
            && can_delete_from(workspace, source)
            && (is_reference || can_edit),
        unlink: unlink_permission(workspace, containing),
        is_reference,
        source,
    }
}

pub(crate) fn apply(
    workspace: &Rc<Workspace>,
    reference: &BlockInfo,
    containing: Option<Uuid>,
    action: Action,
) {
    let permissions = permissions(workspace, reference, containing);
    match action {
        Action::Open => workspace.open(
            TabItem {
                id: reference.id,
                block_type: reference.block_type,
            },
            None,
        ),
        Action::Picker => workspace.open_picker(reference.id),
        Action::SetParent(parent) => workspace.blocks().set_parent(reference.id, parent),
        Action::Rename => workspace.host().rename_block(reference.id),
        Action::Share => workspace.host().share_block(reference.id),
        Action::Unlink => {
            if let BlockSource::Block(container) = permissions.source {
                workspace.host().unlink_block(reference.id, container);
            }
        }
        Action::Delete => workspace.host().delete_block(
            reference.id,
            reference.block_type,
            permissions.source,
            permissions.is_reference,
        ),
    }
}

#[component]
pub(crate) fn ReferenceMenuItem(
    workspace: Rc<Workspace>,
    reference: Memo<Option<BlockInfo>>,
    containing: Memo<Option<Uuid>>,
) -> MenuItem {
    let naming = Rc::clone(&workspace);
    let label = create_memo(clone!(reference -> move || {
        let types = naming.types();
        reference.with(|reference| {
            reference.as_ref().map_or_else(
                || "Untitled".to_owned(),
                |reference| reference.label(types.as_ref()).name,
            )
        })
    }));
    let allowed = Rc::clone(&workspace);
    let rights = create_memo(clone!(reference containing -> move || {
        let containing = containing.get();
        reference.with(|reference| {
            reference.as_ref().map(|reference| {
                let permissions = permissions(&allowed, reference, containing);
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
    }));
    let add = create_memo(clone!(rights -> move || rights.get().is_none_or(|rights| rights.0)));
    let edit = create_memo(clone!(rights -> move || rights.get().is_none_or(|rights| rights.1)));
    let delete = create_memo(clone!(rights -> move || rights.get().is_none_or(|rights| rights.2)));
    let unlink = create_memo(clone!(rights -> move || rights.get().is_none_or(|rights| rights.3)));
    let rooted = create_memo(clone!(rights edit -> move || {
        edit.get() || rights.get().is_none_or(|rights| rights.4 == BlockParent::Root)
    }));
    let orphaned = create_memo(clone!(rights edit -> move || {
        edit.get() || rights.get().is_none_or(|rights| rights.4 == BlockParent::Detached)
    }));
    let delete_label = create_memo(clone!(rights -> move || {
        match rights.get().is_some_and(|rights| rights.5) {
            true => "Remove link".to_owned(),
            false => "Delete".to_owned(),
        }
    }));
    let parent_disabled = edit.clone();
    let share_disabled = edit.clone();
    view! {
        <MenuItem label={label}>
            <MenuItem label="Open" />
            <MenuItem label="Add" disabled={add} />
            <MenuItem label="Set parent" disabled={parent_disabled}>
                <MenuItem label="Root" disabled={rooted} />
                <MenuItem label="Orphaned" disabled={orphaned} />
            </MenuItem>
            <MenuItem label="Rename" disabled={edit} />
            <MenuItem label="Share" disabled={share_disabled} />
            <MenuItem label="Unlink" disabled={unlink} />
            <MenuItem label={delete_label} disabled={delete} />
        </MenuItem>
    }
}
