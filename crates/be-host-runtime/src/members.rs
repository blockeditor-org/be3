//! Who may view an editor's own block, named (`block-subject.members`): the server names the
//! accounts (`be_host_stack::list_names`), and each answer names them afresh from the contacts'
//! cards (`be_host_stack::app_graph::members_of`), so a contact's new card reaches the editor
//! without asking the server again.

use be_block_commit::CommitId;
use be_host_stack::AccountName;
use be_sdk_api::{EditorInstanceId, Failure, blocks::BlockValue};
use uuid::Uuid;

use be_host_base::host;

pub(super) struct NamesRead {
    epoch: u64,
    instance: EditorInstanceId,
    read: Result<Vec<AccountName>, String>,
}

impl NamesRead {
    pub(super) fn opened(self, epoch: u64) -> Option<(EditorInstanceId, NamesAnswer)> {
        (self.epoch == epoch).then_some((self.instance, self.read))
    }
}

pub(super) type NamesAnswer = Result<Vec<AccountName>, String>;

#[derive(Default)]
pub(super) struct Members {
    /// What the server last named, once it answered.
    named: Option<NamesAnswer>,
    /// The block's change and head when the server was last asked: it is asked again once the
    /// block changes other than by a new head, as a grant, a move or a share above it changes
    /// who may view it and a message sent does not.
    asked: Option<(u64, Option<CommitId>)>,
    asking: bool,
}

impl Members {
    /// Asks the server to name `block`'s members, unless it has, and nothing changed since that
    /// could change them. A failed ask (offline) is asked again as the graph next moves.
    pub(super) fn request(
        &mut self,
        instance: EditorInstanceId,
        epoch: u64,
        block: Uuid,
        reads: &host::Post<NamesRead>,
    ) {
        if self.asking {
            return;
        }
        let Some(node) = be_host_stack::node(block) else {
            return;
        };
        if !node.access.can_view() {
            return;
        }
        let now = (node.changed, node.head);
        let due = match self.asked {
            None => true,
            Some((changed, head)) => {
                matches!(self.named, Some(Err(_))) || (changed != now.0 && head == now.1)
            }
        };
        self.asked = Some(now);
        if !due {
            return;
        }
        self.asking = true;
        let post = reads.clone();
        be_host_stack::list_names(block, move |read| {
            post.send(NamesRead {
                epoch,
                instance,
                read,
            });
        });
    }

    pub(super) fn named(&mut self, read: NamesAnswer) {
        self.asking = false;
        self.named = Some(read);
    }

    /// What the editor is answered, once the server named the members.
    pub(super) fn value(&self) -> Option<BlockValue> {
        let named = self.named.as_ref()?;
        Some(BlockValue::Members(match named {
            Ok(named) => Ok(be_host_stack::app_graph::members_of(named)),
            Err(error) => Err(Failure::Failed(error.clone())),
        }))
    }
}
