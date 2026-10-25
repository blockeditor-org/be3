use std::collections::HashMap;

use be_commit::CommitId;
use block_plugin_api::{EditorInstanceId, EditorMessage, Message, StoredHead};
use uuid::Uuid;

use crate::host;

pub(super) struct HeadRead {
    epoch: u64,
    instance: EditorInstanceId,
    block: Uuid,
    commit: CommitId,
    head: Result<StoredHead, String>,
}

pub(super) struct HeadReads {
    sender: host::WakingSender<HeadRead>,
    receiver: std::sync::mpsc::Receiver<HeadRead>,
}

impl Default for HeadReads {
    fn default() -> Self {
        let (sender, receiver) = host::waking_channel();
        Self { sender, receiver }
    }
}

impl HeadReads {
    pub(super) fn arrived(
        &self,
        epoch: u64,
        wants: impl Fn(EditorInstanceId, Uuid, CommitId) -> bool,
        out: &mut Vec<Message>,
    ) {
        while let Ok(read) = self.receiver.try_recv() {
            if read.epoch == epoch && wants(read.instance, read.block, read.commit) {
                out.push(Message::Editor(EditorMessage::Head {
                    instance: read.instance,
                    block_id: read.block.into_bytes(),
                    head: read.head.map(Some),
                }));
            }
        }
    }
}

#[derive(Default)]
pub(super) struct Heads {
    requested: HashMap<Uuid, Option<Option<CommitId>>>,
}

impl Heads {
    pub(super) fn watch(&mut self, blocks: Vec<Uuid>) {
        self.requested.retain(|block, _| blocks.contains(block));
        for block in blocks {
            self.requested.entry(block).or_default();
        }
    }

    pub(super) fn follows(&self, block: Uuid) -> bool {
        self.requested.contains_key(&block)
    }

    pub(super) fn wants(&self, block: Uuid, commit: CommitId) -> bool {
        self.requested.get(&block) == Some(&Some(Some(commit)))
    }

    pub(super) fn forget_sent(&mut self) {
        for requested in self.requested.values_mut() {
            *requested = None;
        }
    }

    pub(super) fn request(
        &mut self,
        instance: EditorInstanceId,
        epoch: u64,
        reads: &HeadReads,
        out: &mut Vec<Message>,
    ) {
        for (block, requested) in &mut self.requested {
            let block = *block;
            let Some(node) = crate::be::node(block) else {
                continue;
            };
            if !node.access.can_view() || *requested == Some(node.head) {
                continue;
            }
            *requested = Some(node.head);
            let Some(commit) = node.head else {
                out.push(Message::Editor(EditorMessage::Head {
                    instance,
                    block_id: block.into_bytes(),
                    head: Ok(None),
                }));
                continue;
            };
            let sender = reads.sender.clone();
            let content_type = node.content_type.into_bytes();
            crate::be::read_head(block, commit, move |read| {
                let _ = sender.send(HeadRead {
                    epoch,
                    instance,
                    block,
                    commit,
                    head: read.map(|bytes| StoredHead {
                        commit: *commit.hash().as_bytes(),
                        content_type,
                        bytes,
                    }),
                });
            });
        }
    }
}
