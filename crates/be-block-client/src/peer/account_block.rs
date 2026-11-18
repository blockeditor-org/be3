//! The account's own block (guides/blocks/keys.md, "Contacts"): owner-only, at the top of the
//! personal organization at an id derived from the account (`be_block::account_block`), holding
//! what the account keeps for itself. Any peer reaches it through a peer of the personal
//! organization of its own, which holds this block's keys alone and keeps nothing on the device:
//! the block is small, and read again as each pass needs it.

use std::sync::Arc;

use be_block::{AccountBlock, AccountBlockContent};
use be_block_commit::CommitId;
use be_block_content::{BlockMetadata, Root};
use be_block_graph::BlockParent;
use be_block_keys::{Party, RecoverySecret};
use be_block_model::Edit;
use be_block_protocol::{
    ErrorCode, KeySet, OrganizationKind, OrganizationRole, WrappedBy, WrappedKey,
};
use be_block_store::{ChunkerConfig, MemoryStore};
use uuid::Uuid;

use super::{Peer, PeerConfig, Saved};
use crate::{ClientError, Connection, Credentials, refs::RefsDb};

/// How often an update reads again after another device saved first.
const TRIES: usize = 4;

/// What each call answers: a future boxed as sent between threads. Reading contacts reaches this
/// block, and its peer reads contacts when keys arrive from a sender it does not know, so these
/// futures nest in each other's types; boxed with their bounds stated here, each is checked once,
/// where its types are known, and a caller spawning one is not asked to prove more.
pub type Sent<'a, T> = std::pin::Pin<Box<dyn Future<Output = Result<T, ClientError>> + Send + 'a>>;

pub struct AccountRecords {
    peer: Peer<MemoryStore>,
    block: Uuid,
}

/// What the account's block held when read.
pub struct Held {
    pub head: Option<CommitId>,
    pub content: AccountBlockContent,
}

impl AccountRecords {
    /// The account's block, reached over `connection`; none for an account in no personal
    /// organization (an agent), which keeps what it is sent in its mailbox instead.
    pub fn open<'a>(
        connection: &'a Arc<Connection>,
        account: Uuid,
        identity: &'a RecoverySecret,
    ) -> Sent<'a, Option<Self>> {
        Box::pin(async move {
        let organizations = crate::ask(be_block_protocol::read::<
            Vec<be_block_protocol::Organization>,
            _,
        >(
            connection, None, be_block_protocol::Query::Organizations
        ))
        .await?;
        let Some(personal) = organizations
            .into_iter()
            .find(|organization| organization.kind == OrganizationKind::Personal)
        else {
            return Ok(None);
        };
        let config = PeerConfig {
            url: String::new(),
            identity: identity.clone(),
            credentials: Credentials::Header,
            organization: Some(personal.id),
            organization_name: personal.name,
            chunker: ChunkerConfig::DEFAULT,
            refs: RefsDb::memory(),
        };
        let mut peer = Peer::assemble(
            Arc::clone(connection),
            account,
            personal.id,
            config,
            MemoryStore::new(),
        )?;
        peer.account_records = true;
        // Its keys opening are nothing the connection's other peers wait on.
        peer.keys.on_opened(|| {});
        *peer.session.lock().unwrap() = (OrganizationRole::Editor, OrganizationKind::Personal);
        let block = be_block::account_block(personal.id, account);
        peer.refresh_keys(Some(vec![block])).await?;
        Ok(Some(Self { peer, block }))
        })
    }

    pub fn block(&self) -> Uuid {
        self.block
    }

    pub fn organization(&self) -> Uuid {
        self.peer.organization()
    }

    /// The block as it is now: empty when never made. `ClientError::Locked` when it does not
    /// open on this device, as after a new phrase taken on another device.
    pub fn read(&self) -> Sent<'_, Held> {
        Box::pin(async move {
        match self.peer.summary(self.block).await {
            Err(ClientError::Refused(ErrorCode::BlockNotFound, _)) => {
                return Ok(Held {
                    head: None,
                    content: AccountBlockContent::default(),
                });
            }
            Err(error) => return Err(error),
            Ok(_) => {}
        }
        let content = self.peer.open::<AccountBlockContent>(self.block)
            .await?
            .unwrap_or_default();
        Ok(Held {
            head: self.peer.local_head(self.block),
            content,
        })
        })
    }

    /// Saves the edits `change` makes to the block as read, making the block first if it was
    /// never made; when another device saved first, reads again and asks `change` again.
    pub fn update<'a>(
        &'a self,
        change: impl Fn(&AccountBlock) -> Vec<Edit> + Send + Sync + 'a,
    ) -> Sent<'a, ()> {
        Box::pin(async move {
        for _ in 0..TRIES {
            let held = self.read().await?;
            let edits = change(&held.content.root());
            match self.save(held, &edits).await {
                Err(ClientError::Contended) => continue,
                saved => return saved,
            }
        }
        Err(ClientError::Contended)
        })
    }

    /// Saves `edits` to the block as `held` read it, making it first if it was never made.
    /// `ClientError::Contended` when another device saved first: the caller reads again.
    pub fn save<'a>(&'a self, held: Held, edits: &'a [Edit]) -> Sent<'a, ()> {
        Box::pin(async move {
        if edits.is_empty() {
            return Ok(());
        }
        let Held { head, mut content } = held;
        // Made just now, it holds nothing; made by another device since it was read, the save
        // on no head is rejected and read again.
        if head.is_none() && self.ensure().await? {
            content = AccountBlockContent::default();
        }
        for edit in edits {
            content.apply(edit);
        }
        match self.peer.save(self.block, &content, head).await? {
            Saved::Published(_) | Saved::Unchanged(_) => Ok(()),
            Saved::Rejected { .. } => Err(ClientError::Contended),
            Saved::Kept(_) => Err(ClientError::Disconnected(
                "the account's block is saved only with the server".into(),
            )),
        }
        })
    }

    /// Makes the block if the server has none: whether this made it.
    fn ensure(&self) -> Sent<'_, bool> {
        Box::pin(async move {
        match self.peer.summary(self.block).await {
            Ok(_) => return Ok(false),
            Err(ClientError::Refused(ErrorCode::BlockNotFound, _)) => {}
            Err(error) => return Err(error),
        }
        let metadata = BlockMetadata::default();
        match self.peer.create_block(
            self.block,
            AccountBlock::CONTENT_TYPE,
            BlockParent::Root,
            &metadata,
            true,
        )
        .await
        {
            Ok(_) => Ok(true),
            // Another device of the account made it first: its keys are read for it.
            Err(ClientError::Refused(ErrorCode::BlockAlreadyExists, _)) => {
                self.peer.refresh_keys(Some(vec![self.block])).await?;
                Ok(false)
            }
            Err(error) => Err(error),
        }
        })
    }

    /// The keys of the block's next epoch, sealed to `fresh` alone, for the write that moves the
    /// account to it (`contacts::set_recovery_key`): its scope key, sealed by `fresh` to itself,
    /// and a data key wrapped under it, so what is written after opens with the new phrase only
    /// (guides/blocks/keys.md, "Identity, new phrases and devices"). None while the block was
    /// never made.
    pub fn epoch_for<'a>(&'a self, fresh: &'a RecoverySecret) -> Sent<'a, Option<KeySet>> {
        Box::pin(async move {
        match self.peer.summary(self.block).await {
            Err(ClientError::Refused(ErrorCode::BlockNotFound, _)) => return Ok(None),
            Err(error) => return Err(error),
            Ok(_) => {}
        }
        let keys = &self.peer.keys;
        let (scope, mut minted) = keys.mint_own_scope(self.block)?;
        let plain = keys.carried_plain(scope)?;
        let account = self.peer.account();
        let mine = Party {
            account: account.into_bytes(),
            public: fresh.public(),
        };
        minted.wrapped = vec![WrappedKey {
            key: scope,
            wrapped_by: WrappedBy::Account(account),
            sender: account,
            sealed: fresh.seal_for(account.into_bytes(), &mine, scope.as_bytes(), &plain),
        }];
        let (_, data) = keys.mint_data(self.block, scope)?;
        minted.keys.extend(data.keys);
        minted.wrapped.extend(data.wrapped);
        Ok(Some(minted))
        })
    }
}
