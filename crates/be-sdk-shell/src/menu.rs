use be_sdk_beui::be_block::{self, SettingsContent};
use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::be_sdk_ui::BlockTypes;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::MenuItem;
use be_sdk_beui::{BlockInfo, BlockParent, Rect, node_anchor};
use uuid::Uuid;

use super::dialogs::OpenDialog;
use super::tab::TabItem;
use super::workspace::Workspace;

#[derive(Clone, Copy)]
pub enum BlockAction {
    Open,
    Picker,
    MoveToMyFiles,
    Rename,
    Share,
    Unlink,
    Delete,
    Restore,
    Settings,
}

pub struct Permissions {
    pub add: bool,
    pub edit: bool,
    pub share: bool,
    pub settings: bool,
    pub delete: bool,
    pub offers_delete: bool,
    pub unlink: Result<(), &'static str>,
    pub is_reference: bool,
    pub source: Option<BlockParent>,
}

pub(crate) fn unlink_permission(
    workspace: &Workspace,
    container: Option<Uuid>,
) -> Result<(), &'static str> {
    let container_type = container.and_then(|container| workspace.known_type(container));
    let types = workspace.types();
    match (container, container_type) {
        (Some(_), Some(block_type)) if !types.child_operations(block_type).replace => {
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
        BlockParent::Root | BlockParent::Detached | BlockParent::Unfiled => true,
        BlockParent::Block(id) => {
            let types = workspace.types();
            workspace
                .known_type(id)
                .is_some_and(|block_type| types.child_operations(block_type).delete)
                && workspace.can_edit(id)
        }
    }
}

pub fn permissions(
    workspace: &Workspace,
    reference: &BlockInfo,
    containing: Option<Uuid>,
) -> Permissions {
    let can_edit = workspace.can_edit(reference.id);
    let source = containing.map(BlockParent::Block).or(reference.parent);
    let is_reference =
        containing.is_some_and(|id| reference.parent != Some(BlockParent::Block(id)));
    let types = workspace.types();
    let host = workspace.host();
    let offers_delete = is_reference
        || reference.id != be_block::my_files(host.organization_id(), host.account_id());
    Permissions {
        add: types.child_operations(reference.block_type).add && can_edit,
        edit: can_edit,
        share: workspace.can_manage(reference.id),
        settings: workspace.can_manage(reference.id)
            || workspace.editor().settings_block(reference.id).is_some(),
        delete: offers_delete
            && source.is_some_and(|source| {
                source != BlockParent::Detached && can_delete_from(workspace, source)
            })
            && (is_reference || can_edit),
        offers_delete,
        unlink: unlink_permission(workspace, containing),
        is_reference,
        source,
    }
}

pub fn apply(
    workspace: &Workspace,
    reference: &BlockInfo,
    containing: Option<Uuid>,
    action: BlockAction,
    anchor: Option<Rect>,
) {
    let permissions = permissions(workspace, reference, containing);
    match action {
        BlockAction::Open => workspace.open(TabItem::new(reference.id, reference.block_type), None),
        BlockAction::Picker => workspace.open_picker(reference.id, anchor),
        BlockAction::MoveToMyFiles => {
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
        BlockAction::Rename => workspace.open_rename(reference.id, anchor),
        BlockAction::Share => workspace.open_dialog(OpenDialog::Share(reference.id)),
        BlockAction::Unlink => {
            if let Some(BlockParent::Block(container)) = permissions.source {
                workspace.host().unlink_block(reference.id, container);
            }
        }
        BlockAction::Restore => workspace.host().restore_block(reference.id),
        BlockAction::Settings => {
            if let Some(settings) = workspace.editor().ensure_settings_block(reference.id) {
                workspace.open(
                    TabItem::new(settings, SettingsContent::CONTENT_TYPE),
                    Some(reference.id),
                );
            }
        }
        BlockAction::Delete => {
            if let Some(source) = permissions.source {
                workspace
                    .host()
                    .delete_block(reference.id, reference.block_type, source);
            }
        }
    }
}

#[component]
pub(crate) fn ReferenceMenuItem(
    workspace: Workspace,
    reference: Memo<Option<BlockInfo>>,
    containing: Memo<Option<Uuid>>,
    anchor: NodeRef,
) -> MenuItem {
    let act = move |action: BlockAction| {
        if let Some(reference) = reference.get_untracked() {
            apply(
                &workspace,
                &reference,
                containing.get_untracked(),
                action,
                node_anchor(&anchor),
            );
        }
    };
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
                    !permissions.share,
                    !permissions.settings,
                    permissions.offers_delete,
                )
            })
        })
    });
    let add = create_memo(move || rights.get().is_none_or(|rights| rights.0));
    let edit = create_memo(move || rights.get().is_none_or(|rights| rights.1));
    let delete = create_memo(move || rights.get().is_none_or(|rights| rights.2));
    let unlink = create_memo(move || rights.get().is_none_or(|rights| rights.3));
    let share = create_memo(move || rights.get().is_none_or(|rights| rights.6));
    let settings = create_memo(move || rights.get().is_none_or(|rights| rights.7));
    let offers_delete = create_memo(move || rights.get().is_none_or(|rights| rights.8));
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
    let detail = create_memo(move || {
        match reference.with(|reference| {
            reference
                .as_ref()
                .is_some_and(|reference| reference.deleted)
        }) {
            true => "In Recently Deleted".to_owned(),
            false => String::new(),
        }
    });
    let restore = create_memo(move || {
        edit.get()
            || reference.with(|reference| {
                reference
                    .as_ref()
                    .is_none_or(|reference| !reference.deleted)
            })
    });
    let delete_label = create_memo(move || match rights.get().is_some_and(|rights| rights.5) {
        true => "Remove link".to_owned(),
        false => "Delete".to_owned(),
    });
    view! {
        <MenuItem label={label} detail={detail}>
            <MenuItem label="Open" on_click={move || act(BlockAction::Open)} />
            <MenuItem label="Add" disabled={add} on_click={move || act(BlockAction::Picker)} />
            <MenuItem label="Set parent" disabled={edit}>
                <MenuItem
                    label="My files"
                    disabled={rooted}
                    on_click={move || act(BlockAction::MoveToMyFiles)}
                />
            </MenuItem>
            <MenuItem label="Rename" disabled={edit} on_click={move || act(BlockAction::Rename)} />
            <MenuItem label="Share" disabled={share} on_click={move || act(BlockAction::Share)} />
            <MenuItem
                label="Unlink"
                disabled={unlink}
                on_click={move || act(BlockAction::Unlink)}
            />
            <MenuItem
                label="Restore"
                disabled={restore}
                on_click={move || act(BlockAction::Restore)}
            />
            <MenuItem
                label="Settings"
                disabled={settings}
                on_click={move || act(BlockAction::Settings)}
            />
            <Show condition={offers_delete}>
                <MenuItem
                    label={delete_label}
                    disabled={delete}
                    on_click={move || act(BlockAction::Delete)}
                />
            </Show>
        </MenuItem>
    }
}
