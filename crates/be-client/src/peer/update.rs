use be_block::{BlockContent, Entries, derived_metadata};
use be_commit::CommitId;
use be_store::ObjectStore;
use uuid::Uuid;

use super::{Peer, Saved};
use crate::ClientError;

const ATTEMPTS: usize = 8;

#[derive(Clone)]
pub struct Encoded {
    pub bytes: Vec<u8>,
    pub references: Vec<Uuid>,
    pub derived: Entries,
}

impl Encoded {
    pub fn of<C: BlockContent>(content: &C, organization: Uuid) -> Self {
        Self {
            bytes: content.encode(),
            references: content.references_in(organization),
            derived: derived_metadata(content),
        }
    }
}

pub struct Updated {
    pub head: Option<CommitId>,
    pub bytes: Option<Vec<u8>>,
    pub changed: bool,
}

impl<S: ObjectStore> Peer<S> {
    pub async fn contents(
        &self,
        block: Uuid,
    ) -> Result<(Option<CommitId>, Option<Vec<u8>>), ClientError> {
        if let Some(head) = self.local_head(block)
            && !self.pending_commits(block).is_empty()
        {
            return Ok((Some(head), Some(self.read_at(head).await?)));
        }
        let read = self
            .read_contents(&[(block, self.local_head(block))])
            .await?;
        let Some((summary, bytes)) = read.into_iter().next() else {
            return Ok((None, None));
        };
        if let Some(head) = summary.head {
            self.remember(block, head)?;
        }
        Ok((summary.head, bytes))
    }

    pub async fn update<E>(
        &self,
        block: Uuid,
        content_type: Uuid,
        fail: impl Fn(ClientError) -> E,
        mut change: impl AsyncFnMut(Option<Vec<u8>>) -> Result<Option<Encoded>, E>,
    ) -> Result<Updated, E> {
        for _ in 0..ATTEMPTS {
            let (head, bytes) = self.contents(block).await.map_err(&fail)?;
            let unchanged = |bytes| Updated {
                head,
                bytes,
                changed: false,
            };
            let Some(next) = change(bytes.clone()).await? else {
                return Ok(unchanged(bytes));
            };
            if Some(&next.bytes) == bytes.as_ref() {
                return Ok(unchanged(bytes));
            }
            let saved = self
                .publish_encoded(
                    block,
                    content_type,
                    &next.bytes,
                    next.references,
                    next.derived,
                    head,
                    None,
                )
                .await
                .map_err(&fail)?;
            let (head, changed) = match saved {
                Saved::Rejected { .. } => continue,
                Saved::Published(head) => (head, true),
                Saved::Unchanged(head) => (head, false),
            };
            return Ok(Updated {
                head: Some(head),
                bytes: Some(next.bytes),
                changed,
            });
        }
        Err(fail(ClientError::Contended))
    }

    pub async fn overwrite(
        &self,
        block: Uuid,
        content_type: Uuid,
        encoded: &Encoded,
    ) -> Result<CommitId, ClientError> {
        let mut expected = self.newest_head(block).await?;
        for _ in 0..ATTEMPTS {
            match self
                .publish_encoded(
                    block,
                    content_type,
                    &encoded.bytes,
                    encoded.references.clone(),
                    encoded.derived.clone(),
                    expected,
                    None,
                )
                .await?
            {
                Saved::Rejected { head } => expected = head,
                Saved::Published(head) | Saved::Unchanged(head) => return Ok(head),
            }
        }
        Err(ClientError::Contended)
    }
}
