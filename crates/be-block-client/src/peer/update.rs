//! The one read-change-write loop (guides/blocks/client.md, "Changing a block against its head"):
//! read a block's head, make its new content from it and write against that head, and when
//! another write landed first, read again and make again, up to `UPDATE_ATTEMPTS` writes.

use be_block_commit::CommitId;
use be_block_content::{BlockContent, Entries, LiveEdit, derived_metadata};
use be_block_store::{ObjectStore, Part};
use uuid::Uuid;

use super::{BlockRead, Peer, Saved, Write};
use crate::ClientError;

/// How many writes an update makes before it gives up with `ClientError::Contended`: each after
/// the first follows a write that landed between its read and its write.
pub const UPDATE_ATTEMPTS: usize = 8;

/// The content an update writes: its stored form and what it says of itself
/// (`Write::described`).
#[derive(Clone, Debug)]
pub struct NewContent {
    pub parts: Part<'static>,
    pub references: Vec<Uuid>,
    pub derived: Entries,
}

impl NewContent {
    /// `content` as written by a member of `organization`, as `Peer::save` writes it.
    pub fn of<C: BlockContent>(content: &C, organization: Uuid) -> Self {
        Self {
            parts: content.parts().into_owned(),
            references: content.references_in(organization),
            derived: derived_metadata(content),
        }
    }
}

impl<S: ObjectStore> Peer<S> {
    /// Reads `block` as this device has it newest (`current`), asks `make` for its new content
    /// and writes that against the head read. When another write landed first, it reads again
    /// and asks `make` again, so nothing it writes goes over a write `make` did not see. None
    /// when `make` answers nothing; `ClientError::Contended` after `UPDATE_ATTEMPTS` writes
    /// refused. `make` answers a future that borrows nothing of it, so the update is `Send`
    /// whenever that future is: state it keeps across reads lives outside it.
    pub async fn update<E, Fut>(
        &self,
        block: Uuid,
        content_type: Uuid,
        mut make: impl FnMut(BlockRead) -> Fut,
    ) -> Result<Option<Saved>, E>
    where
        E: From<ClientError>,
        Fut: Future<Output = Result<Option<NewContent>, E>>,
    {
        for _ in 0..UPDATE_ATTEMPTS {
            let read = self.current(block).await?;
            let head = read.summary.head;
            let Some(change) = make(read).await? else {
                return Ok(None);
            };
            match self.write_change(block, content_type, &change, head).await? {
                Saved::Rejected { .. } => continue,
                saved => return Ok(Some(saved)),
            }
        }
        Err(ClientError::Contended.into())
    }

    /// `update`'s force form: writes `change` as it is, against the newest head and, when
    /// another write landed first, against the head that write left, whatever that holds. For
    /// content that replaces the block's whole (a reset to blank, a revision restored).
    pub async fn force_update(
        &self,
        block: Uuid,
        content_type: Uuid,
        change: &NewContent,
    ) -> Result<Saved, ClientError> {
        let mut head = self.newest_head(block).await?;
        for _ in 0..UPDATE_ATTEMPTS {
            match self.write_change(block, content_type, change, head).await? {
                Saved::Rejected { head: newer } => head = newer,
                saved => return Ok(saved),
            }
        }
        Err(ClientError::Contended)
    }

    /// `update` for content this device decodes: `ops` gets the content as read and answers the
    /// operations to apply to it. None when they change nothing.
    pub async fn update_content<C, Fut>(
        &self,
        block: Uuid,
        mut ops: impl FnMut(C) -> Fut,
    ) -> Result<Option<Saved>, ClientError>
    where
        C: LiveEdit + Clone + Default,
        Fut: Future<Output = Result<Vec<C::Op>, ClientError>>,
    {
        let organization = self.organization();
        self.update(block, C::CONTENT_TYPE, |read: BlockRead| {
            let content = match &read.stored {
                Some(stored) => C::from_stored(stored).map_err(ClientError::from),
                None => Ok(C::default()),
            };
            let asked = content.as_ref().ok().map(|content| ops(content.clone()));
            async move {
                let mut content = content?;
                let Some(asked) = asked else {
                    return Ok(None);
                };
                let ops = asked.await?;
                if ops.is_empty() {
                    return Ok(None);
                }
                let before = content.encode();
                for op in &ops {
                    content.apply(op);
                }
                if content.encode() == before {
                    return Ok(None);
                }
                Ok(Some(NewContent::of(&content, organization)))
            }
        })
        .await
    }

    /// `block` as this device has it newest: the head of its own commits still in the outbox,
    /// read from its store, or else what the server holds, with the session's journal past it.
    pub async fn current(&self, block: Uuid) -> Result<BlockRead, ClientError> {
        if let Some(head) = self.local_head(block)
            && !self.pending_commits(block).is_empty()
        {
            let mut summary = match self.known_summary(block) {
                Some(summary) => summary,
                None => self.summary(block).await?,
            };
            summary.head = Some(head);
            return Ok(BlockRead {
                summary,
                stored: Some(self.read_at(block, head).await?),
                journal: None,
            });
        }
        let read = self
            .read_contents(&[(block, self.local_head(block))])
            .await?
            .into_iter()
            .next()
            .ok_or(ClientError::Unexpected)?;
        if let Some(head) = read.summary.head {
            self.remember(block, head)?;
        }
        Ok(read)
    }

    async fn write_change(
        &self,
        block: Uuid,
        content_type: Uuid,
        change: &NewContent,
        expected: Option<CommitId>,
    ) -> Result<Saved, ClientError> {
        let write = Write::encoded(content_type, change.parts.clone(), expected)
            .described(change.references.clone(), change.derived.clone());
        self.write(block, write).await
    }
}
