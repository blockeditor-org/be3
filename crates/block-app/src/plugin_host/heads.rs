use std::collections::HashMap;

use be_commit::CommitId;
use block_plugin_api::{EditorInstanceId, StoredHead};
use uuid::Uuid;

use block_app_base::host;

pub(super) type Head = Result<Option<StoredHead>, String>;

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
        mut arrived: impl FnMut(EditorInstanceId, Uuid, CommitId, Head),
    ) {
        while let Ok(read) = self.receiver.try_recv() {
            if read.epoch == epoch {
                arrived(read.instance, read.block, read.commit, read.head.map(Some));
            }
        }
    }
}

#[derive(Default)]
pub(super) struct Heads {
    requested: HashMap<Uuid, Option<Option<CommitId>>>,
}

impl Heads {
    pub(super) fn watch(&mut self, block: Uuid) {
        self.requested.insert(block, None);
    }

    pub(super) fn unwatch(&mut self, block: Uuid) {
        self.requested.remove(&block);
    }

    pub(super) fn follows(&self, block: Uuid) -> bool {
        self.requested.contains_key(&block)
    }

    pub(super) fn wants(&self, block: Uuid, commit: CommitId) -> bool {
        self.requested.get(&block) == Some(&Some(Some(commit)))
    }

    pub(super) fn request(
        &mut self,
        instance: EditorInstanceId,
        epoch: u64,
        reads: &HeadReads,
        mut now: impl FnMut(Uuid, Head),
    ) {
        for (block, requested) in &mut self.requested {
            let block = *block;
            let Some(node) = block_app_stack::node(block) else {
                continue;
            };
            if !node.access.can_view() || *requested == Some(node.head) {
                continue;
            }
            *requested = Some(node.head);
            let Some(commit) = node.head else {
                now(block, Ok(None));
                continue;
            };
            let sender = reads.sender.clone();
            let content_type = node.content_type.into_bytes();
            block_app_stack::read_head(block, commit, move |read| {
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
