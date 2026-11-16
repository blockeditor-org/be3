use std::collections::HashMap;

use be_block_graph::{Access, BlockParent};
use be_block_protocol::{
    BlockSummary, ErrorCode, KeyKind, KeySet, KeyWork, Member, OrganizationRole, client,
};
use be_block_store::ObjectStore;
use uuid::Uuid;

use super::Peer;
use crate::ClientError;

impl<S: ObjectStore> Peer<S> {
    /// The organization's key work for this account: the first value of a `key-work-changed`
    /// stream, which starts with it.
    pub async fn key_work(&self) -> Result<KeyWork, ClientError> {
        let mut work = client::keys::key_work_changed(&self.connection());
        match work.recv().await {
            Some(work) => Ok(work),
            None => Err(match work.ended() {
                Some(Err(error)) => ClientError::from(error),
                _ => ClientError::Disconnected("the key work stream ended".into()),
            }),
        }
    }

    pub async fn tend(&self) -> Result<(), ClientError> {
        let work = self.key_work().await?;
        self.tend_with(work).await
    }

    /// `tend` from key work already read, such as a value of the connection's
    /// `key-work-changed` stream (`Event::KeyWork`).
    pub async fn tend_with(&self, mut work: KeyWork) -> Result<(), ClientError> {
        // A new escrow key is taken only from an administrator its organization's log names.
        if work.escrows != self.keys.escrows() {
            self.learn_administrators().await;
        }
        self.adopt_escrows(work.escrows.clone());
        let mut failed = None;
        let escrow_rotation = work.escrow_rotation && self.keys.can_make_escrow();
        if escrow_rotation && let Err(error) = self.make_escrow().await {
            failed.get_or_insert(error);
        }
        for root in &work.rotations {
            if let Err(error) = self.rotate(*root).await {
                failed.get_or_insert(error);
            }
        }
        if escrow_rotation || !work.rotations.is_empty() {
            work = self.key_work().await?;
            if work.escrows != self.keys.escrows() {
                self.learn_administrators().await;
            }
            self.adopt_escrows(work.escrows.clone());
        }
        let mut keys = KeySet::default();
        for unsealed in &work.unsealed {
            if !self.keys.trusted(unsealed.key) {
                continue;
            }
            let Some(public) = self.keys.recipient(unsealed.account, unsealed.key) else {
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
        let pinned = crate::contacts::Pinned {
            root: self.keys.organization_root(),
            escrow: Some(escrow),
        };
        let sealed = crate::contacts::seal_pin(self.keys.identity(), self.organization, &pinned);
        client::keys::pin_escrow(&self.connection(), sealed).await??;
        self.adopt_pinned(pinned);
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
            for row in self.keys.keys_of(node.id, KeyKind::Data) {
                if self.keys.trusted(row.id) {
                    keys.wrapped.push(self.keys.wrap(row.id, under)?);
                }
            }
            let (_, minted) = self.keys.mint_data(node.id, under)?;
            keys.extend(minted);
        }
        client::keys::rotate(&self.connection(), vec![root], keys.clone()).await??;
        self.recorded(&keys);
        Ok(())
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
                && let Some(wrap) = self.keys.escrow_wrap_under(row.id, &escrow)?
            {
                keys.wrapped.push(wrap);
            }
        }
        client::keys::create_escrow(&self.connection(), escrow.clone(), keys.clone()).await??;
        self.keys.adopt_escrow(escrow);
        self.recorded(&keys);
        self.save_pin().await
    }

    pub async fn refresh_contacts(&self) -> Result<Vec<crate::Contact>, ClientError> {
        let contacts =
            crate::contacts::contacts(&self.connection(), self.account, self.keys.identity())
                .await?;
        self.adopt_contacts(&contacts);
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

    /// Sets a member's role. Making or unmaking an administrator of a company organization is a
    /// record in its log, signed here, naming the keys this device verified for them (or, to
    /// unmake one, those the log names).
    pub async fn set_role(&self, account: Uuid, role: OrganizationRole) -> Result<(), ClientError> {
        let signed = self.administrator_record(account, role).await?;
        client::members::set_role(&self.connection(), account, role, signed).await??;
        Ok(())
    }

    async fn administrator_record(
        &self,
        account: Uuid,
        role: OrganizationRole,
    ) -> Result<Option<Vec<u8>>, ClientError> {
        use be_block_keys::{Administrator, Entry, LogKind, Record};
        let Some(pinned) = self.keys.organization_root() else {
            return Ok(None);
        };
        let entries = self.organization_log().await?;
        let checked = be_block_keys::check_log(
            LogKind::Organization,
            self.organization.into_bytes(),
            pinned,
            &entries,
            &|_, _| None,
        )
        .map_err(|_| ClientError::Forged(self.organization))?;
        let administrator = role == OrganizationRole::Administrator;
        let named = checked.administrators.get(&account.into_bytes()).copied();
        if administrator == named.is_some() {
            return Ok(None);
        }
        let identity = self
            .verified_identity(account)
            .or(named)
            .or_else(|| (account == self.account).then(|| self.keys.identity().identity()))
            .ok_or_else(|| {
                ClientError::Refused(
                    ErrorCode::NotAContact,
                    "make an administrator only of a contact: send them a contact link first"
                        .into(),
                )
            })?;
        let entry = Entry::sign(
            self.keys.identity(),
            self.account.into_bytes(),
            self.organization.into_bytes(),
            entries.last().map(Entry::hash),
            Record::Administrator(Administrator {
                account: account.into_bytes(),
                identity,
                administrator,
            }),
        );
        Ok(Some(entry.encode()))
    }

    pub async fn remove_member(&self, account: Uuid) -> Result<(), ClientError> {
        client::members::remove_member(&self.connection(), account).await??;
        Ok(())
    }
}
