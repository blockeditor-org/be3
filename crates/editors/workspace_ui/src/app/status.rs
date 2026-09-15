use block::{BlockParent, BlockReference};
use block_client::ReferenceList;
use block_editor_plugin::{
    BlockSource,
    block_ui::{BlockLabel, BlockTypes},
    egui,
};
use uuid::Uuid;

use super::{Frame, State, TabItem, menu};

pub(crate) struct Pending {
    pub(crate) reference: BlockReference,
    pub(crate) source: BlockSource,
    pub(crate) is_reference: bool,
    pub(crate) action: menu::Action,
}

pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut State,
    frame: &Frame<'_>,
    id: Uuid,
) -> Option<TabItem> {
    let block_type = state.block_types.get(&id).copied().unwrap_or_default();
    let type_name = frame
        .types
        .display_name(block_type)
        .map_or_else(|| block_type.to_string(), str::to_owned);
    let parents = state.parents.get(&id).map(ReferenceList::is_loaded);
    let references = state
        .references
        .get(&id)
        .map(|list| (list.is_loaded(), list.read()));
    let backrefs = state
        .backrefs
        .get(&id)
        .map(|list| (list.is_loaded(), list.read()));
    let mut navigate = None;
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("Type: {type_name}"));
        ui.separator();
        if parents != Some(true) {
            ui.label("Relationships loading\u{2026}");
            return;
        }
        ui.menu_button(
            format!(
                "Backrefs: {}",
                backrefs.as_ref().map_or_else(
                    || "\u{2026}".to_owned(),
                    |(loaded, backrefs)| match loaded {
                        true => backrefs.len().to_string(),
                        false => "\u{2026}".to_owned(),
                    }
                )
            ),
            |ui| {
                let Some((loaded, backrefs)) = &backrefs else {
                    ui.weak("Loading\u{2026}");
                    return;
                };
                reference_list(
                    ui,
                    state,
                    frame,
                    backrefs,
                    *loaded,
                    "No backrefs",
                    None,
                    &mut navigate,
                    &mut action,
                );
            },
        );
        ui.separator();
        ui.menu_button(
            format!(
                "References: {}",
                references.as_ref().map_or_else(
                    || "\u{2026}".to_owned(),
                    |(loaded, references)| match loaded {
                        true => references.len().to_string(),
                        false => "\u{2026}".to_owned(),
                    }
                )
            ),
            |ui| {
                let Some((loaded, references)) = &references else {
                    ui.weak("Loading\u{2026}");
                    return;
                };
                reference_list(
                    ui,
                    state,
                    frame,
                    references,
                    *loaded,
                    "No references",
                    Some(id),
                    &mut navigate,
                    &mut action,
                );
            },
        );
    });
    if let Some(action) = action {
        apply(state, frame, action);
    }
    navigate
}

pub(crate) fn unlink_permission(
    state: &State,
    frame: &Frame<'_>,
    container: Option<Uuid>,
) -> Result<(), &'static str> {
    let container_type = container.and_then(|container| {
        state.block_types.get(&container).copied().or_else(|| {
            frame
                .client
                .cached_block(container)
                .map(|cached| cached.block_type)
        })
    });
    match (container, container_type) {
        (Some(_), Some(block_type)) if !frame.types.child_edits(block_type).replace => {
            Err("This container doesn't support replacing a reference")
        }
        (Some(container), Some(_)) if !frame.can_edit(container) => {
            Err("You don't have permission to edit this container")
        }
        (Some(_), Some(_)) => Ok(()),
        _ => Err("Loading\u{2026}"),
    }
}

fn source_of(parent: BlockParent) -> BlockSource {
    match parent {
        BlockParent::Root => BlockSource::Root,
        BlockParent::Orphaned => BlockSource::Orphaned,
        BlockParent::Uuid(id) => BlockSource::Block(id),
    }
}

pub(crate) fn reference_list(
    ui: &mut egui::Ui,
    state: &mut State,
    frame: &Frame<'_>,
    references: &[BlockReference],
    loaded: bool,
    empty: &str,
    containing: Option<Uuid>,
    navigate: &mut Option<TabItem>,
    pending: &mut Option<Pending>,
) {
    if references.is_empty() {
        ui.weak(match loaded {
            true => empty,
            false => "Loading\u{2026}",
        });
    }
    for reference in references {
        state.record_reference_types(reference);
        let source = containing.map_or_else(|| source_of(reference.parent), BlockSource::Block);
        let is_reference = containing.is_some_and(|id| reference.parent != BlockParent::Uuid(id));
        let label = BlockLabel::for_reference(frame.types, reference).widget_text(ui.style());
        let response = ui.button(label).on_hover_text(reference.id.to_string());
        if response.clicked() {
            *navigate = Some(TabItem {
                id: reference.id,
                block_type: reference.block_type,
            });
            ui.close();
        }
        let can_edit = frame.can_edit(reference.id);
        let permissions = menu::Permissions {
            add: frame.types.child_edits(reference.block_type).add && can_edit,
            edit: can_edit,
            delete: source != BlockSource::Orphaned
                && can_delete_from(state, frame, source)
                && (is_reference || can_edit),
            unlink: unlink_permission(state, frame, containing),
        };
        let mut chosen = None;
        response.context_menu(|ui| {
            chosen = menu::show(
                ui,
                frame,
                &mut state.parent_candidates,
                reference.id,
                reference.parent,
                permissions,
                is_reference,
            );
        });
        if let Some(action) = chosen {
            *pending = Some(Pending {
                reference: reference.clone(),
                source,
                is_reference,
                action,
            });
        }
    }
}

fn can_delete_from(state: &State, frame: &Frame<'_>, source: BlockSource) -> bool {
    match source {
        BlockSource::Root | BlockSource::Orphaned => true,
        BlockSource::Block(id) => {
            let block_type = state.block_types.get(&id).copied().or_else(|| {
                frame
                    .client
                    .cached_block(id)
                    .map(|cached| cached.block_type)
            });
            block_type.is_some_and(|block_type| frame.types.child_edits(block_type).delete)
                && frame.can_edit(id)
        }
    }
}

pub(crate) fn apply(state: &mut State, frame: &Frame<'_>, pending: Pending) {
    let Pending {
        reference,
        source,
        is_reference,
        action,
    } = pending;
    match action {
        menu::Action::Picker => state.open_picker(reference.id),
        menu::Action::SetParent(parent) => frame.client.set_block_parent(reference.id, parent),
        menu::Action::Rename => frame.host.rename_block(reference.id),
        menu::Action::Share => frame.host.share_block(reference.id),
        menu::Action::Unlink => {
            if let BlockSource::Block(container) = source {
                frame.host.unlink_block(reference.id, container);
            }
        }
        menu::Action::Delete => {
            frame
                .host
                .delete_block(reference.id, reference.block_type, source, is_reference)
        }
    }
}
