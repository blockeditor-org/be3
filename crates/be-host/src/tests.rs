use super::*;
mod a_crash_report_does_not_leave_a_dev_run_busy;
mod a_creation_that_failed_tries_again_once_committed_again;
mod a_leave_asked_while_another_is_pending_does_not_change_what_confirm_leaves;
mod a_move_into_a_container_whose_type_refuses_it_puts_the_block_back;
mod a_reorder_its_container_refuses_is_reported;
mod an_editor_closes_only_itself_and_the_children_it_placed;
mod an_unlink_the_containers_type_makes_places_the_copy_in_it;
mod an_unlink_the_containers_type_refuses_makes_no_copy;
mod restore_puts_a_block_back_in_unfiled_and_falls_back_to_it;
mod unlinking_into_a_container_that_cannot_replace_a_child_ends_with_a_notice;

/// The manifests of text, which takes a replaced child and nothing else, and folder, which
/// takes every child change, in place of the apps a unit test does not stage. Their block types
/// and `children` are copied from crates/be-app-text/manifest.json and
/// crates/be-app-workspace/manifest.json, without the editors and modules those name, which a
/// unit test has none of: keep them as those say.
const MANIFESTS: [(&str, &str); 2] = [
    (
        "text.json",
        r#"{"id": "be3.text", "name": "Text", "version": "0.1.0", "entry_point": "text_block.wasm",
            "block_types": {"text": {"id": "6f4d8f85-7991-4cdf-ae41-b52630df014b",
                "display_name": "Text", "icon": "\ue0cc", "color": [0, 0, 0],
                "children": {"replace": true},
                "module": "text_block_types.wasm"}}}"#,
    ),
    (
        "workspace.json",
        r#"{"id": "be3.workspace", "name": "Workspace", "version": "0.1.0",
            "entry_point": "workspace.wasm",
            "block_types": {"folder": {"id": "d4e367ab-db73-4419-bffc-b9131288c2e7",
                "display_name": "Folder", "icon": "\ue0cc", "color": [0, 0, 0],
                "children": {"add": true, "delete": true, "replace": true, "reorder": true},
                "module": "workspace_block_types.wasm"}}}"#,
    ),
];

/// Waits until the running stack knows each of `types`, as a core needs before it changes a
/// block of one, with the text and folder manifests (`MANIFESTS`) found while the answer is held.
#[track_caller]
fn known(types: &[Uuid]) -> be_host_stack::discovery::InstalledForTesting {
    let installed = be_host_stack::discovery::install_for_testing(&MANIFESTS);
    be_host_stack::wait_until("the block types are known", |_| {
        types
            .iter()
            .all(|block_type| be_host_stack::is_known(*block_type))
            .then_some(())
    });
    installed
}

/// Waits until the running stack has sent all it made to the server: a text made here and not
/// on the server yet takes no replaced child.
#[track_caller]
fn synced() {
    be_host_stack::wait_until("everything reached the server", |shared| {
        (shared.unsynced().is_empty() && shared.busy(true).is_empty()).then_some(())
    });
}

/// Opens a core on the running stack (`be_host_testing::Harness`).
fn core() -> HostCore {
    let state = HostStateStore::open(":memory:").expect("the host state opens");
    HostCore::with_state(state).expect("the app starts")
}

/// Makes a block on the running stack, named `name`, with `content`.
fn make(block: Uuid, content_type: Uuid, parent: BlockParent, name: &str, content: Vec<u8>) {
    be_host_stack::create(
        block,
        content_type,
        parent,
        be_block_content::BlockMetadata::named(name),
        Some(content),
        false,
    );
}

/// What the core sent to containers' types and waits to hear back.
fn awaited(core: &HostCore) -> Vec<be_host_stack::ChildChanged> {
    let copies = core
        .pending_copies
        .iter()
        .filter_map(|copy| match &copy.stage {
            CopyStage::Replacing { answer, .. } => Some(answer.clone()),
            _ => None,
        });
    let transfers =
        core.pending_transfers
            .iter()
            .filter_map(|transfer| match &transfer.stage {
                TransferStage::DeletingSource(answer)
                | TransferStage::AddingDestination(answer) => Some(answer.clone()),
                _ => None,
            });
    let reorders = core
        .pending_reorders
        .iter()
        .filter_map(|reorder| reorder.answer.clone());
    copies.chain(transfers).chain(reorders).collect()
}

/// Runs the core's pending copies, transfers and reorders as its frames do until none is left,
/// waiting between runs for the types to answer what they were sent. Fails, naming what is left
/// (`pending`), after a few passes that answer nothing.
#[track_caller]
fn settle(core: &mut HostCore) {
    let mut idle = false;
    for _ in 0..16 {
        // Taken before the pass, so what readies between it and a wait is not missed.
        let seen = progressed();
        core.process_pending_copies();
        core.process_pending_transfers();
        core.process_pending_reorders();
        if core.pending_copies.is_empty()
            && core.pending_transfers.is_empty()
            && core.pending_reorders.is_empty()
        {
            return;
        }
        // What a pass queued for the next (a block put back after a refusal) runs then; a pass
        // after that with nothing sent waits for the stack to move (a type or an editor that
        // is not ready yet).
        let awaited = awaited(core);
        if awaited.is_empty() {
            if std::mem::replace(&mut idle, true) {
                be_host_stack::wait_until("the stack moved on", |shared| {
                    (shared.progressed != seen).then_some(())
                });
            }
            continue;
        }
        idle = false;
        be_host_stack::wait_until("the containers' types answered", |_| {
            awaited
                .iter()
                .all(|answer| answer.answer().is_some())
                .then_some(())
        });
    }
    panic!(
        "the core never finished its pending changes: {}",
        pending(core)
    );
}

/// The stages the core's pending changes are at, for a test that fails waiting on them.
fn pending(core: &HostCore) -> String {
    let copies = core.pending_copies.iter().map(|copy| {
        match copy.stage {
            CopyStage::Duplicate => "a copy to duplicate",
            CopyStage::Replacing { .. } => "a copy replacing",
        }
        .to_owned()
    });
    let transfers = core.pending_transfers.iter().map(|transfer| {
        let stage = match transfer.stage {
            TransferStage::DeleteSource => "to delete from its source",
            TransferStage::DeletingSource(_) => "deleting from its source",
            TransferStage::AddDestination => "to add to its destination",
            TransferStage::AddingDestination(_) => "adding to its destination",
        };
        let what = match transfer.putting_back {
            true => "a transfer putting a block back",
            false => "a transfer",
        };
        format!("{what} {stage} {:?}", transfer.destination)
    });
    let reorders = core.pending_reorders.iter().map(|reorder| {
        match reorder.answer {
            Some(_) => "a reorder sent",
            None => "a reorder to send",
        }
        .to_owned()
    });
    copies
        .chain(transfers)
        .chain(reorders)
        .collect::<Vec<_>>()
        .join(", ")
}

/// How far the running stack has got (`Shared::progressed`).
fn progressed() -> u64 {
    be_host_stack::wait_until("read the stack", |shared| Some(shared.progressed))
}

/// Whether a refusal containing `text` was shown. Notices are the process's, and a target's tests
/// share a process: `text` names what only its own test does.
fn refused(text: &str) -> bool {
    notices::shown()
        .iter()
        .any(|notice| notice.refusal && notice.message.contains(text))
}
