use std::collections::HashMap;

use be_graph::{Access, BlockParent};
use be_protocol::{
    BlockSummary, ClientMessage, KeyKind, KeySet, KeyWork, Member, OrganizationRole, ServerMessage,
};
use be_store::ObjectStore;
use uuid::Uuid;

use super::Peer;
use crate::ClientError;

impl<S: ObjectStore> Peer<S> {
    pub async fn key_work(&self) -> Result<KeyWork, ClientError> {
        match self
            .connection
            .request(|request| ClientMessage::GetKeyWork { request })
            .await?
        {
            ServerMessage::KeyWork { work, .. } => Ok(work),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn tend(&self) -> Result<(), ClientError> {
        let mut work = self.key_work().await?;
        self.keys.set_escrow(work.escrow);
        let mut failed = None;
        if work.escrow_rotation
            && let Err(error) = self.make_escrow().await
        {
            failed.get_or_insert(error);
        }
        for root in &work.rotations {
            if let Err(error) = self.rotate(*root).await {
                failed.get_or_insert(error);
            }
        }
        if work.escrow_rotation || !work.rotations.is_empty() {
            work = self.key_work().await?;
            self.keys.set_escrow(work.escrow);
        }
        let mut keys = KeySet::default();
        for unsealed in &work.unsealed {
            if !self.keys.has(unsealed.key) {
                continue;
            }
            let Some(public) = self.keys.contact(unsealed.account) else {
                continue;
            };
            keys.wrapped
                .push(self.keys.seal_to(unsealed.key, unsealed.account, &public)?);
        }
        for key in &work.unescrowed {
            if self.keys.has(*key)
                && let Some(wrap) = self.keys.escrow_wrap(*key)?
            {
                keys.wrapped.push(wrap);
            }
        }
        if let Err(error) = self.put_keys(keys).await {
            failed.get_or_insert(error);
        }
        failed.map_or(Ok(()), Err)
    }

    pub async fn rotate(&self, root: Uuid) -> Result<(), ClientError> {
        let top = self.summary(root).await?;
        let mut walked = vec![top.clone()];
        let mut next = 0;
        while let Some(id) = walked.get(next).map(|node| node.id) {
            let children = self.children(BlockParent::Block(id)).await?;
            walked.extend(children);
            next += 1;
        }
        self.refresh_keys(Some(walked.iter().map(|node| node.id).collect()))
            .await?;
        let outer = match top.scope == top.id {
            true => None,
            false => Some(self.scope_key(top.scope).await?),
        };
        let above_top = match (top.scope == top.id, top.parent) {
            (true, BlockParent::Block(parent)) => Some(self.scope_key_under(parent).await?),
            _ => None,
        };
        let scope_of: HashMap<Uuid, Uuid> =
            walked.iter().map(|node| (node.id, node.scope)).collect();
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
            if node.scope == node.id {
                let parent = match node.parent {
                    BlockParent::Block(parent) => match scope_of.get(&parent) {
                        Some(scope) => Some(current(&fresh, *scope)?),
                        None => above_top,
                    },
                    BlockParent::Root | BlockParent::Detached => None,
                };
                let (scope, minted) = self.keys.mint_scope(node.id, parent)?;
                keys.extend(minted);
                for (account, public) in self.recipients(node).await? {
                    keys.wrapped
                        .push(self.keys.seal_to(scope, account, &public)?);
                }
                fresh.insert(node.id, scope);
            }
            let under = current(&fresh, node.scope)?;
            for row in self.keys.keys_of(node.id, KeyKind::Data) {
                if self.keys.has(row.id) {
                    keys.wrapped.push(self.keys.wrap(row.id, under)?);
                }
            }
            let (_, minted) = self.keys.mint_data(node.id, under)?;
            keys.extend(minted);
        }
        self.connection
            .request(|request| ClientMessage::Rotate {
                request,
                roots: vec![root],
                keys: keys.clone(),
            })
            .await?;
        self.recorded(&keys);
        Ok(())
    }

    async fn recipients(
        &self,
        node: &BlockSummary,
    ) -> Result<Vec<(Uuid, be_keys::RecoveryPublic)>, ClientError> {
        let at_root = !matches!(node.parent, BlockParent::Block(_));
        Ok(self
            .list_access(node.id)
            .await?
            .into_iter()
            .filter(|entry| entry.account != self.account && entry.effective.can_view())
            .filter(|entry| {
                entry.granted.is_some_and(Access::can_view) || (entry.author && at_root)
            })
            .filter_map(|entry| Some((entry.account, self.keys.contact(entry.account)?)))
            .collect())
    }

    pub(super) async fn make_escrow(&self) -> Result<(), ClientError> {
        self.refresh_keys(None).await?;
        let (escrow, mut keys) = self.keys.mint_escrow(self.organization)?;
        for member in self.members().await? {
            if member.role == OrganizationRole::Administrator
                && member.account != self.account
                && let Some(public) = self.keys.contact(member.account)
            {
                keys.wrapped
                    .push(self.keys.seal_to(escrow.id, member.account, &public)?);
            }
        }
        self.keys.set_escrow(Some(escrow));
        for row in self.keys.keys_of_kind(KeyKind::Scope) {
            if self.keys.has(row.id)
                && let Some(wrap) = self.keys.escrow_wrap(row.id)?
            {
                keys.wrapped.push(wrap);
            }
        }
        self.connection
            .request(|request| ClientMessage::CreateEscrow {
                request,
                escrow,
                keys: keys.clone(),
            })
            .await?;
        self.recorded(&keys);
        Ok(())
    }

    pub async fn refresh_contacts(&self) -> Result<Vec<crate::Contact>, ClientError> {
        let contacts = crate::contacts::contacts(&self.connection, self.keys.identity()).await?;
        self.keys.set_contacts(
            contacts
                .iter()
                .filter_map(|contact| Some((contact.account, contact.public?))),
        );
        Ok(contacts)
    }

    pub async fn leave(&self, block: Uuid) -> Result<(), ClientError> {
        self.grant(block, self.account, Access::None).await
    }

    pub async fn members(&self) -> Result<Vec<Member>, ClientError> {
        match self
            .connection
            .request(|request| ClientMessage::ListMembers { request })
            .await?
        {
            ServerMessage::Members { members, .. } => Ok(members),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn set_role(&self, account: Uuid, role: OrganizationRole) -> Result<(), ClientError> {
        self.connection
            .request(|request| ClientMessage::SetRole {
                request,
                account,
                role,
            })
            .await?;
        Ok(())
    }

    pub async fn remove_member(&self, account: Uuid) -> Result<(), ClientError> {
        self.connection
            .request(|request| ClientMessage::RemoveMember { request, account })
            .await?;
        Ok(())
    }
}
