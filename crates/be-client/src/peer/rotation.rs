use std::collections::HashMap;

use be_graph::{Access, BlockParent};
use be_protocol::{BlockSummary, KeyKind, KeySet, KeyWork, Member, OrganizationRole, client};
use be_store::ObjectStore;
use uuid::Uuid;

use super::Peer;
use crate::ClientError;

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
            if let Err(error) = self.rotate(*root).await {
                failed.get_or_insert(error);
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
            let children: Vec<BlockSummary> = children.into_iter().flatten().collect();
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
                let (scope, minted) = self.keys.mint_scope(node.id, parent)?;
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
