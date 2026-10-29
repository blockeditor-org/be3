use std::sync::{Arc, PoisonError};

use wasi_threads::Mutex;

use be_client::ClientError;
use be_commit::CommitId;
use be_types_host::Conflict;
use block_type_api::{Description, EditorId, Status};
use block_type_guest::editor::ToEditor;
use uuid::Uuid;

use super::{
    Content,
    worker::{PeerMessage, Presence, Shared, Store},
};

pub(crate) use be_types_host::HOST_EDITOR;

pub(super) type Replicas = be_types_host::Replicas<Shared>;

impl be_types_host::Host for Shared {
    type Store = Store;

    fn spawn(task: impl Future<Output = ()> + 'static) {
        super::platform::spawn_local(task);
    }

    fn wake() {
        block_app_base::host::wake();
    }

    fn record(shared: &Arc<Mutex<Self>>, error: ClientError) {
        super::worker::record(shared, error);
    }

    fn report(shared: &Arc<Mutex<Self>>, block: Option<Uuid>, message: String) {
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.module_error = block.map(|block| (block, message.clone()));
        held.error = Some(message);
    }

    fn set_locked(shared: &Arc<Mutex<Self>>, block: Uuid) {
        super::worker::set_locked(shared, block, true);
    }

    fn owned(shared: &Arc<Mutex<Self>>, block: Uuid) -> bool {
        shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .owned
            .contains(&block)
    }

    fn reshaped(shared: &Arc<Mutex<Self>>, block: Uuid, schema: &str) -> bool {
        shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .graph
            .get(block)
            .and_then(|node| node.metadata.get(&be_block::SCHEMA))
            .is_some_and(|stamped| stamped != schema)
    }

    fn reset_notice() {
        block_app_base::notices::report(
            "Some of this device's settings were saved by another build of Block, so they were reset."
                .to_owned(),
        );
    }

    fn known_head(shared: &Arc<Mutex<Self>>, block: Uuid) -> Option<CommitId> {
        let held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.graph.get(block).and_then(|node| node.head)
    }

    fn to_editor(shared: &Arc<Mutex<Self>>, block: Uuid, editor: EditorId, bytes: Vec<u8>) -> bool {
        to_editor(shared, block, editor, bytes);
        true
    }

    fn content(shared: &Arc<Mutex<Self>>, block: Uuid, content_type: Uuid, bytes: Vec<u8>) {
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        let revision = held
            .blocks
            .get(&block)
            .map_or(0, |content| content.revision);
        let mut content = Content::new(content_type, bytes);
        content.revision = revision + 1;
        held.blocks.insert(block, content);
        held.previewed.remove(&block);
        held.touch(block);
    }

    fn released(shared: &Arc<Mutex<Self>>, block: Uuid) {
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.blocks.remove(&block);
        held.described.remove(&block);
        held.graph.forget_derived(block);
        held.graph.forget_references(block);
        held.previewed.remove(&block);
        held.histories.remove(&block);
        held.presence.remove(&block);
        held.peer_messages.remove(&block);
        held.touch(block);
    }

    fn status(shared: &Arc<Mutex<Self>>, block: Uuid, status: &Status) {
        let locked = {
            let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
            let before = held.graph.get(block).and_then(|node| node.head);
            if before != status.head {
                held.saves += 1;
                held.touch(block);
                if let Some((from, message)) = held.module_error.take() {
                    match from == block && held.error.as_ref() == Some(&message) {
                        true => held.error = None,
                        false => held.module_error = Some((from, message)),
                    }
                }
            }
            held.graph.set_head(block, status.head);
            held.locked.contains(&block)
        };
        if !locked || status.unreadable.is_none() {
            super::worker::set_unreadable(shared, block, status.unreadable.clone());
        }
    }

    fn described(
        shared: &Arc<Mutex<Self>>,
        block: Uuid,
        references: Vec<Uuid>,
        description: Description,
    ) {
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.graph.refer(block, references);
        held.described.insert(block, description);
        held.touch(block);
    }

    fn history(shared: &Arc<Mutex<Self>>, block: Uuid, can_undo: bool, can_redo: bool) {
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.histories
            .insert(block, super::History { can_undo, can_redo });
        held.touch(block);
    }

    fn conflict(shared: &Arc<Mutex<Self>>, block: Uuid, conflict: Conflict) {
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.conflicts.entry(block).or_default().push(conflict);
        held.stale_revision += 1;
        held.touch(block);
    }

    fn counted(shared: &Arc<Mutex<Self>>, unsealed: usize, owing: usize) {
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.unsealed = unsealed;
        held.owing = owing;
    }
}

fn to_editor(shared: &Arc<Mutex<Shared>>, block: Uuid, editor: EditorId, bytes: Vec<u8>) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    if editor == HOST_EDITOR {
        match ToEditor::decode(&bytes) {
            Some(ToEditor::Presence(peers)) => {
                held.presence_revision += 1;
                let revision = held.presence_revision;
                held.presence.insert(
                    block,
                    (
                        revision,
                        peers
                            .into_iter()
                            .map(|peer| Presence {
                                client: peer.client,
                                kind: peer.kind,
                                value: peer.value,
                            })
                            .collect(),
                    ),
                );
                held.touch(block);
            }
            Some(ToEditor::PeerMessages(messages)) => {
                for message in messages {
                    held.peer_message_count += 1;
                    let sequence = held.peer_message_count;
                    let queue = held.peer_messages.entry(block).or_default();
                    queue.push_back((
                        sequence,
                        PeerMessage {
                            from: message.from,
                            channel: message.channel,
                            payload: message.payload,
                        },
                    ));
                    if queue.len() > super::worker::PEER_MESSAGE_LIMIT {
                        queue.pop_front();
                    }
                }
                held.touch(block);
            }
            _ => {}
        }
        return;
    }
    held.editor_frames
        .entry(editor)
        .or_default()
        .push((block, frame(bytes)));
    drop(held);
    block_app_base::host::wake();
}

fn frame(mut bytes: Vec<u8>) -> Vec<u8> {
    let mut framed = (bytes.len() as u32).to_be_bytes().to_vec();
    framed.append(&mut bytes);
    framed
}

pub(crate) fn unframe(frame: &[u8]) -> Option<Vec<u8>> {
    let (length, payload) = frame.split_first_chunk::<4>()?;
    (u32::from_be_bytes(*length) as usize == payload.len()).then(|| payload.to_vec())
}

pub(crate) fn snapshot_frame(content_type: Uuid, bytes: Vec<u8>) -> Vec<u8> {
    frame(
        ToEditor::Snapshot(block_type_guest::editor::Snapshot {
            content_type,
            bytes,
            session: Vec::new(),
            unsettled: Vec::new(),
            applied: 0,
        })
        .encode(),
    )
}
