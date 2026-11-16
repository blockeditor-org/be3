use std::collections::HashMap;

use be_block_graph::{Access, BlockParent};
use be_block_protocol::{
    BlockSummary, ErrorCode, KeyKind, KeySet, KeyWork, Member, OrganizationRole, client,
};
use be_block_store::ObjectStore;
use be_block_keys::RecoveryPublic;
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
        // Revocations the server made on its own go in the blocks' logs, signed here.
        for revoked in &work.unsigned_revocations {
            if let Err(error) = self
                .grant_as(revoked.block, revoked.account, Access::None, None)
                .await
            {
                failed.get_or_insert(error);
            }
        }
        *self.unsigned.lock().unwrap() = work.unsigned_members.clone();
        if let Err(error) = self.tend_agents(&work.unsigned_members).await {
            failed.get_or_insert(error);
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
            // An owner-only block's keys are its author's alone, whoever the server lists.
            if !self.keys.trusted(unsealed.key) || self.keys.owner_only_covers(unsealed.key) {
                continue;
            }
            let Some(public) = self.sealing_target(unsealed.key, unsealed.account).await else {
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

    /// The key `key` is sealed to `account` with, as the logs name them (signed access 3c): to
    /// the server, only the organization settings' scope key (`KeyBook::recipient`); an escrow
    /// key, and the settings' scope key, to an administrator, with the keys the organization's
    /// log names; an owner-only key,
    /// to this account alone; a scope key, to a grantee its block's own log names with View or
    /// above, with the keys the grant names, contact or not. That log is checked from the root
    /// this device pinned in full (a weak root is a member's word, plans/signed_access.md), and
    /// a grant that rests on the server's tree (`Checked::tainted`) counts for nothing. Key
    /// work lists only accounts the server holds a grant row for, so one it revoked is sealed
    /// to no more, signed or not.
    async fn sealing_target(&self, key: Uuid, account: Uuid) -> Option<RecoveryPublic> {
        let row = self.keys.row(key)?;
        if self.keys.server().is_some_and(|(server, _)| server == account) {
            return self.keys.recipient(account, key);
        }
        // The escrow key, and the organization settings' scope key, which administering the
        // organization gives rather than a grant: to administrators its log names.
        let settings = be_block::organization_settings(self.organization);
        let administering = row.kind == KeyKind::Escrow
            || (row.kind == KeyKind::Scope && self.keys.block_of(key) == Some(settings));
        if administering
            && let Some(checked) = self.checked_organization().await.ok().flatten()
            && let Some(administrator) = checked.administrators.get(&account.into_bytes())
        {
            return Some(administrator.public);
        }
        if row.kind == KeyKind::Escrow {
            return None;
        }
        if self.keys.owner_only(key) {
            return (account == self.account).then(|| self.keys.identity().public());
        }
        if row.kind != KeyKind::Scope {
            return None;
        }
        let block = self.keys.block_of(key)?;
        // A kind of settings open to whoever views a block inside is offered by its viewers to
        // their contacts: a standing grant no log names (plans/signed_access.md, "Grants the
        // server changes first").
        if self
            .summary(block)
            .await
            .is_ok_and(|summary| summary.opens_inside)
        {
            return self.keys.contact(account);
        }
        let crate::keys::RootPin::Pinned(_) = self.keys.root_pin(block) else {
            return None;
        };
        let (_, checked) = self.checked_access(block).await.ok()?;
        let grantee = account.into_bytes();
        if checked.tainted.contains(&grantee) {
            return None;
        }
        let (level, identity) = checked.grants.get(&grantee)?;
        (*level >= be_block_keys::Level::View).then_some(identity.public)
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
        let pinned = self.refs.contacts().into_iter().collect();
        let contacts = crate::contacts::contacts(
            &self.connection(),
            self.account,
            self.keys.identity(),
            &pinned,
        )
        .await?;
        self.adopt_contacts(&contacts);
        // Keys a contact sealed before this device verified it open now, and arrive as any key
        // that opens does (`KeyBook::on_opened`).
        self.keys.absorb(KeySet::default());
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

    /// Removes a member, signing them out of the organization's log when it names them.
    pub async fn remove_member(&self, account: Uuid) -> Result<(), ClientError> {
        let checked = self.checked_organization().await?;
        let signed = match checked.as_ref().and_then(|checked| {
            let identity = checked.member(account.into_bytes())?;
            let agent = checked
                .members
                .get(&account.into_bytes())
                .is_some_and(|named| named.maker.is_some());
            Some((identity, agent))
        }) {
            Some((identity, agent)) => {
                self.member_record(checked.as_ref(), account, identity, false, agent)
            }
            None => None,
        };
        let revocations = self.removal_revocations(account, false).await?;
        client::members::remove_member(&self.connection(), account, signed, revocations).await??;
        Ok(())
    }

    /// Removes a contact, with the signed revocations, in this organization, of each one's
    /// grants on blocks the other manages that blocks' logs name.
    pub async fn remove_contact(&self, account: Uuid) -> Result<(), ClientError> {
        let revocations = self.removal_revocations(account, true).await?;
        client::contacts::remove_contact(&self.connection(), account, revocations).await??;
        Ok(())
    }

    /// The revocations removing `account` (as a member, or with `contact` as a contact) makes
    /// in this organization's logs, each signed here: the server refuses the removal without
    /// them, so a revoked grant never waits in a log for a later `tend`.
    async fn removal_revocations(
        &self,
        account: Uuid,
        contact: bool,
    ) -> Result<Vec<Vec<u8>>, ClientError> {
        let pairs =
            client::keys::removal_revocations(&self.connection(), account, contact).await??;
        let mut signed = Vec::new();
        for revoked in pairs {
            if let Some(record) = self
                .signed_grant(revoked.block, revoked.account, Access::None, None)
                .await?
            {
                signed.push(record);
            }
        }
        Ok(signed)
    }

    /// Finishes the links this account made that were accepted (`contacts::finish_links`), and
    /// signs each who accepted an invitation into this organization into its log, with the
    /// keys their card, opened with the invitation's code, names.
    pub async fn finish_links(&self) -> Result<Vec<crate::contacts::Finished>, ClientError> {
        let finished = crate::contacts::finish_links(
            &self.connection(),
            self.keys.identity(),
            self.organization,
        )
        .await?;
        for done in &finished {
            let identity = be_block_keys::PublicIdentity {
                public: done.card.public,
                signing: done.card.signing,
            };
            let account = Uuid::from_bytes(done.card.account);
            // A contact link's grant, applied when it was accepted, goes in the block's log now
            // that the accepter's card names their keys; until then no one seals to them by it.
            // One that fails leaves the server's row unsigned, sealed by no one.
            if let Some(grant) = done
                .grant
                .as_ref()
                .filter(|grant| grant.organization == self.organization)
            {
                let _ = self
                    .grant_as(grant.block, account, grant.access, Some(identity))
                    .await;
            }
            if done.invited == Some(self.organization) {
                // One that fails waits in key work's unsigned members.
                let _ = self.sign_in(account, identity, false).await;
            }
        }
        Ok(finished)
    }

    /// The organization's log checked from the root this account pinned; none when it has no
    /// log (the personal organization).
    pub async fn checked_organization(
        &self,
    ) -> Result<Option<be_block_keys::Checked>, ClientError> {
        let Some(pinned) = self.keys.organization_root() else {
            return Ok(None);
        };
        let entries = self.organization_log().await?;
        be_block_keys::check_log(
            be_block_keys::LogKind::Organization,
            self.organization.into_bytes(),
            pinned,
            &entries,
            &|_, _| None,
        )
        .map(Some)
        .map_err(|_| ClientError::Forged(self.organization))
    }

    /// A member record signing `account`, with `identity`, in or out of the organization's
    /// log as it checks at its head: none when it has no log.
    pub(crate) fn member_record(
        &self,
        checked: Option<&be_block_keys::Checked>,
        account: Uuid,
        identity: be_block_keys::PublicIdentity,
        member: bool,
        agent: bool,
    ) -> Option<Vec<u8>> {
        let checked = checked?;
        let entry = be_block_keys::Entry::sign(
            self.keys.identity(),
            self.account.into_bytes(),
            self.organization.into_bytes(),
            Some(checked.head),
            be_block_keys::Record::Member(be_block_keys::Member {
                account: account.into_bytes(),
                identity,
                member,
                agent,
            }),
        );
        Some(entry.encode())
    }

    /// The member record signing in an agent this account makes, with the keys it made for it,
    /// that `create-agent` carries: none in an organization with no log.
    pub async fn agent_record(
        &self,
        account: Uuid,
        identity: be_block_keys::PublicIdentity,
    ) -> Result<Option<Vec<u8>>, ClientError> {
        let checked = self.checked_organization().await?;
        Ok(self.member_record(checked.as_ref(), account, identity, true, true))
    }

    /// Signs `account` into the organization's log with the keys this device verified for it:
    /// a person, as an administrator; an agent, as its maker.
    pub async fn sign_in(
        &self,
        account: Uuid,
        identity: be_block_keys::PublicIdentity,
        agent: bool,
    ) -> Result<(), ClientError> {
        let checked = self.checked_organization().await?;
        if checked
            .as_ref()
            .is_some_and(|checked| checked.member(account.into_bytes()).is_some())
        {
            return Ok(());
        }
        let Some(signed) = self.member_record(checked.as_ref(), account, identity, true, agent)
        else {
            return Ok(());
        };
        client::members::sign_member(&self.connection(), signed).await??;
        Ok(())
    }

    /// Keeps the organization's log in step with the agents this account made: signs in those
    /// key work lists as signed in by no record, and signs out those the log names that the
    /// server removed on its own (collected with their block, or deleted on taking an account
    /// back). A person is signed in only from an invitation this device made
    /// (`finish_links`): that someone is a member is the server's word alone, so other unsigned
    /// members wait, listed, for an administrator to act (`unsigned_members`).
    async fn tend_agents(&self, unsigned: &[Uuid]) -> Result<(), ClientError> {
        let Some(checked) = self.checked_organization().await? else {
            return Ok(());
        };
        let me = self.account.into_bytes();
        let made: Vec<([u8; 16], be_block_keys::PublicIdentity)> = checked
            .members
            .iter()
            .filter(|(_, named)| named.maker == Some(me))
            .map(|(account, named)| (*account, named.identity))
            .collect();
        let mine_unsigned = unsigned.iter().any(|account| *account != self.account);
        if made.is_empty() && !mine_unsigned {
            return Ok(());
        }
        let members = self.members().await?;
        for member in &members {
            if member.created_by == Some(self.account)
                && unsigned.contains(&member.account)
                && let Some(identity) = self.verified_identity(member.account)
            {
                self.sign_in(member.account, identity, true).await?;
            }
        }
        for (account, identity) in made {
            let account = Uuid::from_bytes(account);
            if members.iter().any(|member| member.account == account) {
                continue;
            }
            let checked = self.checked_organization().await?;
            if let Some(signed) =
                self.member_record(checked.as_ref(), account, identity, false, true)
            {
                client::members::sign_member(&self.connection(), signed).await??;
            }
        }
        Ok(())
    }
}
