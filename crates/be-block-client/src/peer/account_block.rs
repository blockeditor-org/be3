//! The account's own block (guides/blocks/keys.md, "Contacts"): owner-only, at the top of the
//! personal organization at an id derived from the account (`be_block::account_block`), holding
//! what the account keeps for itself. Any peer reaches it through a peer of the personal
//! organization of its own, which holds this block's keys alone and keeps nothing on the device,
//! opened once per connection and identity: the block is small, and read again as each pass
//! needs it.

use std::sync::Arc;

use be_block::{AccountBlock, AccountBlockContent};
use be_block_content::{BlockMetadata, Root};
use be_block_graph::BlockParent;
use be_block_keys::{Party, RecoverySecret};
use be_block_model::Edit;
use be_block_protocol::{
    ErrorCode, KeySet, OrganizationKind, OrganizationRole, WrappedBy, WrappedKey,
};
use be_block_store::{ChunkerConfig, MemoryStore, StoreError};
use uuid::Uuid;

use super::{Peer, PeerConfig, Saved};
use crate::{ClientError, Connection, Credentials, refs::RefsDb};

/// What each call answers: a future boxed as sent between threads. Reading contacts reaches this
/// block, and its peer reads contacts when keys arrive from a sender it does not know, so these
/// futures nest in each other's types; boxed with their bounds stated here, each is checked once,
/// where its types are known, and a caller spawning one is not asked to prove more.
/// On the web nothing is sent between threads, and the browser's futures are not `Send`.
#[cfg(not(target_arch = "wasm32"))]
pub type Sent<'a, T> = std::pin::Pin<Box<dyn Future<Output = Result<T, ClientError>> + Send + 'a>>;
#[cfg(target_arch = "wasm32")]
pub type Sent<'a, T> = std::pin::Pin<Box<dyn Future<Output = Result<T, ClientError>> + 'a>>;

/// Edits answered at once, for an `AccountRecords::update` that reads nothing else.
pub(crate) fn answered<'a>(edits: Vec<Edit>) -> Sent<'a, Vec<Edit>> {
    Box::pin(std::future::ready(Ok(edits)))
}

pub struct AccountRecords {
    peer: Peer<MemoryStore>,
    block: Uuid,
    /// The personal organization's name, as its peer was made with it.
    name: String,
}

/// What a connection learned reaching the account's block, which it keeps for the next pass
/// (`Connection::account_block`): for which account and identity, the personal organization
/// (none for an account in none), and the block's key rows as last read. Plain data, so the
/// connection holds no peer that holds it back; a peer opened from it reads keys again only
/// when a read meets one it lacks.
#[derive(Clone)]
pub(crate) struct Reached {
    account: Uuid,
    public: [u8; 32],
    personal: Option<(Uuid, String)>,
    keys: KeySet,
}

impl AccountRecords {
    /// The account's block, reached over `connection`; none for an account in no personal
    /// organization (an agent), which keeps what it is sent in its mailbox instead. The
    /// organizations and the block's keys are read once per connection and identity.
    pub fn open<'a>(
        connection: &'a Arc<Connection>,
        account: Uuid,
        identity: &'a RecoverySecret,
    ) -> Sent<'a, Option<Arc<Self>>> {
        Box::pin(async move {
            let public = identity.public().0;
            let reached = connection
                .account_block
                .lock()
                .unwrap()
                .clone()
                .filter(|reached| reached.account == account && reached.public == public);
            if let Some(reached) = reached {
                let Some((organization, name)) = reached.personal else {
                    return Ok(None);
                };
                let records = Self::assemble(connection, account, identity, organization, name)?;
                records.peer.refs.add_keys(&reached.keys)?;
                records.peer.keys.absorb(reached.keys);
                return Ok(Some(Arc::new(records)));
            }
            let records = Self::reach(connection, account, identity).await?;
            *connection.account_block.lock().unwrap() = Some(Reached {
                account,
                public,
                personal: records
                    .as_ref()
                    .map(|records| (records.peer.organization(), records.name.clone())),
                keys: records
                    .as_ref()
                    .map(|records| records.peer.refs.keys())
                    .unwrap_or_default(),
            });
            Ok(records.map(Arc::new))
        })
    }

    /// Lets go of what `connection` kept of the block, as its account takes a new phrase.
    pub fn forget(connection: &Arc<Connection>) {
        *connection.account_block.lock().unwrap() = None;
    }

    /// A peer of `organization` that reaches the account's block alone.
    fn assemble(
        connection: &Arc<Connection>,
        account: Uuid,
        identity: &RecoverySecret,
        organization: Uuid,
        name: String,
    ) -> Result<Self, ClientError> {
        let config = PeerConfig {
            url: String::new(),
            identity: identity.clone(),
            credentials: Credentials::Header,
            organization: Some(organization),
            organization_name: name.clone(),
            chunker: ChunkerConfig::DEFAULT,
            refs: RefsDb::memory(),
        };
        let mut peer = Peer::assemble(
            Arc::clone(connection),
            account,
            organization,
            config,
            MemoryStore::new(),
        )?;
        peer.account_records = true;
        // Its keys opening are nothing the connection's other peers wait on.
        peer.keys.on_opened(|| {});
        *peer.session.lock().unwrap() = (OrganizationRole::Editor, OrganizationKind::Personal);
        Ok(Self {
            peer,
            block: be_block::account_block(organization, account),
            name,
        })
    }

    fn reach<'a>(
        connection: &'a Arc<Connection>,
        account: Uuid,
        identity: &'a RecoverySecret,
    ) -> Sent<'a, Option<Self>> {
        Box::pin(async move {
            let organizations =
                crate::ask(be_block_protocol::read::<
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
            let records =
                Self::assemble(connection, account, identity, personal.id, personal.name)?;
            records.peer.refresh_keys(Some(vec![records.block])).await?;
            Ok(Some(records))
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
    pub fn read(&self) -> Sent<'_, AccountBlockContent> {
        Box::pin(async move {
            match self.peer.summary(self.block).await {
                Err(ClientError::Refused(ErrorCode::BlockNotFound, _)) => {
                    return Ok(AccountBlockContent::default());
                }
                Err(error) => return Err(error),
                Ok(_) => {}
            }
            Ok(self
                .peer
                .open::<AccountBlockContent>(self.block)
                .await?
                .unwrap_or_default())
        })
    }

    /// Saves the edits `make` answers for the block as read, making the block first if it was
    /// never made (`Peer::update_content`): when another device saved first, `make` is asked
    /// again of the block as it is then, so a write never goes over one it did not read.
    pub fn update<'a>(
        &'a self,
        mut make: impl FnMut(AccountBlockContent) -> Sent<'a, Vec<Edit>> + Send + 'a,
    ) -> Sent<'a, ()> {
        Box::pin(async move {
            let saved = match self.peer.summary(self.block).await {
                Ok(_) => self.peer.update_content(self.block, &mut make).await?,
                // Never made: made only when `make` has something to keep in it (an account's
                // first pass, before its identity is registered, has nothing), and the edits
                // it answered written on no head. Another device that made and wrote it first
                // refuses that write, and `make` is asked again of what it wrote.
                Err(ClientError::Refused(ErrorCode::BlockNotFound, _)) => {
                    let edits = make(AccountBlockContent::default()).await?;
                    if edits.is_empty() {
                        return Ok(());
                    }
                    self.make().await?;
                    let mut content = AccountBlockContent::default();
                    for edit in &edits {
                        content.apply(edit);
                    }
                    match self.peer.save(self.block, &content, None).await? {
                        Saved::Rejected { .. } => {
                            self.peer.update_content(self.block, &mut make).await?
                        }
                        saved => Some(saved),
                    }
                }
                Err(error) => return Err(error),
            };
            match saved {
                Some(Saved::Kept(_)) => Err(ClientError::Disconnected(
                    "the account's block is saved only with the server".into(),
                )),
                _ => Ok(()),
            }
        })
    }

    /// Makes the block, which the server did not have when read.
    fn make(&self) -> Sent<'_, ()> {
        Box::pin(async move {
            let metadata = BlockMetadata::default();
            match self
                .peer
                .create_block(
                    self.block,
                    AccountBlock::CONTENT_TYPE,
                    BlockParent::Root,
                    &metadata,
                    true,
                )
                .await
            {
                Ok(_) => Ok(()),
                // Another device of the account made it first: its keys are read for it.
                Err(ClientError::Refused(ErrorCode::BlockAlreadyExists, _)) => {
                    self.peer.refresh_keys(Some(vec![self.block])).await?;
                    Ok(())
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
                sealed: fresh
                    .seal_for(account.into_bytes(), &mine, scope.as_bytes(), &plain)
                    .map_err(|_| StoreError::Corrupt)?,
            }];
            let (_, data) = keys.mint_data(self.block, scope)?;
            minted.keys.extend(data.keys);
            minted.wrapped.extend(data.wrapped);
            Ok(Some(minted))
        })
    }
}
