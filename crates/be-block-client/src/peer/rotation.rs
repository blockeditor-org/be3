use std::collections::{HashMap, HashSet};

use be_block_graph::{Access, BlockParent};
use be_block_protocol::{BlockSummary, KeyKind, KeySet, KeyWork, Member, OrganizationRole, client};
use be_block_commit::CommitId;
use be_block_store::{Manifest, ObjectStore, PartRef};
use uuid::Uuid;

use super::Peer;
use crate::{ClientError, Write};

impl<S: ObjectStore> Peer<S> {
    pub async fn key_work(&self) -> Result<KeyWork, ClientError> {
        Ok(client::keys::get_key_work(&self.connection()).await??)
    }

    pub async fn tend(&self) -> Result<(), ClientError> {
        let mut work = self.key_work().await?;
        self.adopt_escrows(work.escrows.clone());
        let mut failed = None;
        let escrow_rotation = work.escrow_rotation && self.keys.can_make_escrow();
        if escrow_rotation && let Err(error) = self.make_escrow().await {
            failed.get_or_insert(error);
        }
        for root in &work.rotations {
            match self.rotate(*root).await {
                // A rotation this device holds too few keys for is another device's to do,
                // and the server keeps offering it until one does: routine, not a failure.
                Ok(()) | Err(ClientError::Locked(_)) => {}
                Err(error) => {
                    failed.get_or_insert(error);
                }
            }
        }
        if escrow_rotation || !work.rotations.is_empty() {
            work = self.key_work().await?;
            self.adopt_escrows(work.escrows.clone());
        }
        let mut keys = KeySet::default();
        for unsealed in &work.unsealed {
            if !self.keys.trusted(unsealed.key) {
                continue;
            }
            let Some(public) = self.keys.contact(unsealed.account) else {
                continue;
            };
            keys.wrapped
                .push(self.keys.seal_to(unsealed.key, unsealed.account, &public)?);
        }
        for key in &work.unescrowed {
            if self.keys.trusted(*key)
                && let Some(wrap) = self.keys.escrow_wrap(*key)?
            {
                keys.wrapped.push(wrap);
            }
        }
        if let Err(error) = self.put_keys(keys).await {
            failed.get_or_insert(error);
        }
        if let Err(error) = self.save_pin().await {
            failed.get_or_insert(error);
        }
        failed.map_or(Ok(()), Err)
    }

    pub(super) async fn save_pin(&self) -> Result<(), ClientError> {
        let Some(escrow) = self.keys.unpinned() else {
            return Ok(());
        };
        let sealed = crate::contacts::seal_pin(self.keys.identity(), self.organization, &escrow);
        client::keys::pin_escrow(&self.connection(), sealed).await??;
        self.adopt_pinned(Some(escrow));
        Ok(())
    }

    async fn rotate(&self, root: Uuid) -> Result<(), ClientError> {
        let top = self.summary(root).await?;
        let mut walked = vec![top.clone()];
        let mut level = vec![top.id];
        while !level.is_empty() {
            let children = futures_util::future::try_join_all(
                level
                    .iter()
                    .map(|id| self.children(BlockParent::Block(*id))),
            )
            .await?;
            let children: Vec<BlockSummary> = children
                .into_iter()
                .flatten()
                .filter(|child| child.access.can_view())
                .collect();
            level = children.iter().map(|child| child.id).collect();
            walked.extend(children);
        }
        self.refresh_keys(Some(walked.iter().map(|node| node.id).collect()))
            .await?;

        let top_scope = super::scope_of(&top)?;
        let outer = match top_scope == top.id {
            true => None,
            false => Some(self.scope_key(top_scope).await?),
        };
        let above_top = match (top_scope == top.id, super::parent_of(&top)?) {
            (true, BlockParent::Block(parent)) => Some(self.scope_key_under(parent).await?),
            _ => None,
        };
        let scope_of = walked
            .iter()
            .map(|node| Ok((node.id, super::scope_of(node)?)))
            .collect::<Result<HashMap<Uuid, Uuid>, ClientError>>()?;
        let mut fresh: HashMap<Uuid, Uuid> = HashMap::new();
        let current = |fresh: &HashMap<Uuid, Uuid>, scope: Uuid| {
            fresh
                .get(&scope)
                .copied()
                .or(outer)
                .ok_or(ClientError::Locked(scope))
        };
        let mut keys = KeySet::default();
        let mut escrowed = Vec::new();
        for node in &walked {
            let node_scope = scope_of[&node.id];
            if node_scope == node.id {
                let parent = match super::parent_of(node)? {
                    BlockParent::Block(parent) => match scope_of.get(&parent) {
                        Some(scope) => Some(current(&fresh, *scope)?),
                        None => above_top,
                    },
                    BlockParent::Root | BlockParent::Detached => None,
                };
                let (scope, minted) = match node.owner_only {
                    true => self.keys.mint_own_scope(node.id)?,
                    false => self.keys.mint_scope(node.id, parent)?,
                };
                keys.extend(minted);
                fresh.insert(node.id, scope);
            }
            let under = current(&fresh, node_scope)?;
            let mut untrusted = HashSet::new();
            for row in self.keys.keys_of(node.id, KeyKind::Data) {
                match self.keys.trusted(row.id) {
                    true => keys.wrapped.push(self.keys.wrap(row.id, under)?),
                    false => {
                        untrusted.insert(row.id);
                    }
                }
            }
            let (_, minted) = self.keys.mint_data(node.id, under)?;
            keys.extend(minted);
            if let Some(epochs) = self.escrowed_in_head(node, &untrusted).await? {
                escrowed.push((node.id, epochs));
            }
        }
        client::keys::rotate(&self.connection(), vec![root], keys.clone()).await??;
        self.recorded(&keys);
        for (block, epochs) in escrowed {
            self.reseal_head(block, &epochs).await?;
        }
        Ok(())
    }

    /// Of `untrusted`, the epochs this device cannot rewrap (it holds them only through
    /// escrow) that `node`'s head is sealed under, which `reseal_head` then takes the head
    /// off. An untrusted epoch the head no longer uses only closes history, and one this
    /// device lacks altogether it can do nothing about.
    pub(super) async fn escrowed_in_head(
        &self,
        node: &BlockSummary,
        untrusted: &HashSet<Uuid>,
    ) -> Result<Option<HashSet<Uuid>>, ClientError> {
        let Some(head) = node.head.filter(|_| !untrusted.is_empty()) else {
            return Ok(None);
        };
        let epochs: HashSet<Uuid> = self
            .head_epochs(head)
            .await?
            .into_iter()
            .map(|(key, _)| key)
            .filter(|key| untrusted.contains(key) && self.keys.has(*key))
            .collect();
        Ok((!epochs.is_empty()).then_some(epochs))
    }

    /// The keys a commit's content is sealed under, each with a part it seals whole, fetching
    /// its nodes and each part's top object but no chunk below them.
    async fn head_epochs(&self, head: CommitId) -> Result<Vec<(Uuid, PartRef)>, ClientError> {
        let Some(root) = self.load_commit(head).await?.manifest.root else {
            return Ok(Vec::new());
        };
        let vault = self.commits.vault();
        loop {
            let missing = vault.missing_tops(root)?;
            if missing.is_empty() {
                break;
            }
            self.fetch(&missing).await?;
        }
        Ok(vault.keys_of_part(root)?)
    }

    /// Seals the parts of `block`'s head under `epochs` again under its current key, and
    /// publishes that, so the head needs none of them. An administrator's device opens them
    /// through escrow, which makes it vouch for what it reseals (guides/blocks/keys.md,
    /// Rotation).
    pub(super) async fn reseal_head(
        &self,
        block: Uuid,
        epochs: &HashSet<Uuid>,
    ) -> Result<(), ClientError> {
        for _ in 0..4 {
            let Some(head) = self.summary(block).await?.head else {
                return Ok(());
            };
            let commit = self.load_commit(head).await?;
            let Some(root) = commit.manifest.root else {
                return Ok(());
            };
            for (key, part) in self.head_epochs(head).await? {
                if epochs.contains(&key) {
                    let whole = Manifest {
                        content_type: commit.manifest.content_type,
                        root: Some(part),
                    };
                    self.fetch_range(&whole, 0, u64::MAX).await?;
                }
            }
            let key = self.sealing_key(block).await?;
            let rekeyed = self
                .commits
                .vault()
                .rekey(key, &|sealed| epochs.contains(&sealed), root)?;
            if rekeyed == root {
                return Ok(());
            }
            let metadata = self.metadata_of(head).await?;
            let write = Write::written(
                Manifest {
                    content_type: commit.manifest.content_type,
                    root: Some(rekeyed),
                },
                Some(head),
            )
            .described(commit.references.clone(), metadata.derived)
            .defined(metadata.defined);
            match self.write(block, write).await? {
                crate::Saved::Rejected { .. } => continue,
                _ => return Ok(()),
            }
        }
        Err(ClientError::Contended)
    }

    pub(super) async fn make_escrow(&self) -> Result<(), ClientError> {
        self.refresh_keys(None).await?;
        let (escrow, mut keys) = self.keys.mint_escrow()?;
        for member in self.members().await? {
            if member.role == OrganizationRole::Administrator
                && member.account != self.account
                && let Some(public) = self.keys.contact(member.account)
            {
                keys.wrapped
                    .push(self.keys.seal_to(escrow.id, member.account, &public)?);
            }
        }
        for row in self.keys.keys_of_kind(KeyKind::Scope) {
            if self.keys.trusted(row.id)
                && let Some(wrap) = self.keys.escrow_wrap(row.id)?
            {
                keys.wrapped.push(wrap);
            }
        }
        client::keys::create_escrow(&self.connection(), escrow.clone(), keys.clone()).await??;
        self.recorded(&keys);
        self.save_pin().await
    }

    pub async fn refresh_contacts(&self) -> Result<Vec<crate::Contact>, ClientError> {
        let contacts =
            crate::contacts::contacts(&self.connection(), self.account, self.keys.identity())
                .await?;
        self.adopt_contacts(
            contacts
                .iter()
                .filter_map(|contact| Some((contact.account, contact.public?))),
        );
        if self.keys.absorb(KeySet::default()) > 0 {
            self.tried.lock().unwrap().clear();
        }
        Ok(contacts)
    }

    pub async fn leave(&self, block: Uuid) -> Result<(), ClientError> {
        self.grant(block, self.account, Access::None).await
    }

    pub async fn members(&self) -> Result<Vec<Member>, ClientError> {
        Ok(client::members::list_members(&self.connection()).await??)
    }

    pub async fn set_role(&self, account: Uuid, role: OrganizationRole) -> Result<(), ClientError> {
        client::members::set_role(&self.connection(), account, role).await??;
        Ok(())
    }

    pub async fn remove_member(&self, account: Uuid) -> Result<(), ClientError> {
        client::members::remove_member(&self.connection(), account).await??;
        Ok(())
    }
}
