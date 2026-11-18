use std::collections::HashMap;

use be_block_graph::{Access, BlockParent};
use be_block_keys::RecoveryPublic;
use be_block_protocol::{
    BlockSummary, ErrorCode, KeyKind, KeySet, KeyWork, Member, OrganizationRole,
};
use be_block_store::ObjectStore;
use uuid::Uuid;

use super::Peer;
use crate::ClientError;

/// A rotation `tend` owes a tree a demotion outdated (`Peer::rotate_demoted`) that failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingRotation {
    /// The tree's top, the scope the rotation starts from (`Peer::rotation_top`).
    pub top: Uuid,
    /// When it first failed, in commit time (`Peer::latest_believed`, milliseconds).
    pub since: i64,
    /// How many `tend`s it failed in since.
    pub failures: u32,
    /// The last failure.
    pub error: String,
}

impl<S: ObjectStore> Peer<S> {
    /// The organization's key work for this account (`query.key-work`).
    pub async fn key_work(&self) -> Result<KeyWork, ClientError> {
        let work = be_block_protocol::read::<KeyWork, _>(
            &self.connection(),
            Some(self.organization),
            be_block_protocol::Query::KeyWork,
        );
        Ok(work.await??)
    }

    pub async fn tend(&self) -> Result<(), ClientError> {
        let work = self.key_work().await?;
        self.tend_with(work).await
    }

    /// `tend` from key work already read.
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
        // Revocations the server made on its own go in the blocks' logs, signed here, before
        // the rotations they call for, so each new epoch names a head after them.
        for revoked in &work.unsigned_revocations {
            if let Err(error) = self
                .grant_as(revoked.block, revoked.account, Access::None, None)
                .await
            {
                failed.get_or_insert(error);
            }
        }
        for root in &work.rotations {
            if let Err(error) = self.rotate(*root).await {
                failed.get_or_insert(error);
            }
        }
        let demoted = self.rotate_demoted().await;
        *self.unsigned.lock().unwrap() = work.unsigned_members.clone();
        if let Err(error) = self.tend_agents(&work.unsigned_members).await {
            failed.get_or_insert(error);
        }
        if escrow_rotation || !work.rotations.is_empty() || demoted {
            work = self.key_work().await?;
            if work.escrows != self.keys.escrows() {
                self.learn_administrators().await;
            }
            self.adopt_escrows(work.escrows.clone());
        }
        let mut keys = KeySet::default();
        let organization = match work.unsealed.is_empty() {
            true => None,
            false => self.checked_organization().await.ok().flatten(),
        };
        let mut outdated = Vec::new();
        for unsealed in &work.unsealed {
            // An owner-only block's keys are its author's alone, whoever the server lists.
            if !self.keys.trusted(unsealed.key) || self.keys.owner_only_covers(unsealed.key) {
                continue;
            }
            match self
                .sealing_target(unsealed.key, unsealed.account, organization.as_ref())
                .await
            {
                Target::Seal(public) => {
                    keys.wrapped
                        .push(self.keys.seal_to(unsealed.key, unsealed.account, &public)?)
                }
                Target::Outdated(scope) if !outdated.contains(&scope) => outdated.push(scope),
                Target::Outdated(_) | Target::Nobody => {}
            }
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
        // A scope whose newest epoch the log outdates is due a rotation, whoever the server
        // asked: the new epoch names this device's heads, and the key work it brings seals it.
        for scope in outdated {
            if let Err(error) = self.rotate(scope).await {
                failed.get_or_insert(error);
            }
        }
        if let Err(error) = self.save_pin().await {
            failed.get_or_insert(error);
        }
        failed.map_or(Ok(()), Err)
    }

    /// Gives a new epoch, from the top, to every scope this device holds and may edit whose
    /// newest key a demotion outdates (`KeyBook::demoted`), each tree once (`rotation_top`): a
    /// member reaching a scope only from inside it cannot rotate it, as its new epoch is
    /// wrapped under the scope above, so the editors above do it when key work tells them of
    /// the demotion rather than at their next write there, which may never come. Whether this
    /// account may edit a tree's top is judged from its signed logs (`may_write_signed`), as a
    /// reseal judges it, never the server's summary. Opportunistic: a scope this device cannot
    /// rotate is left to one that can, and one another device rotated first is seen current
    /// once its keys are read. A rotation that fails is kept as pending since its first failure
    /// (`pending_rotations`) until one goes through or the tree is current again. Answers
    /// whether it rotated any.
    async fn rotate_demoted(&self) -> bool {
        let Ok(Some(_)) = self.checked_organization().await else {
            return false;
        };
        let demoted = |block: Uuid| {
            self.keys.scope_key(block).is_some_and(|key| {
                self.keys.trusted(key)
                    && !self.keys.owner_only_covers(key)
                    && self.keys.demoted(key, block)
            })
        };
        let mut scopes: Vec<Uuid> = self
            .keys
            .keys_of_kind(KeyKind::Scope)
            .into_iter()
            .filter_map(|row| self.keys.block_of(row.id))
            .collect();
        scopes.sort();
        scopes.dedup();
        scopes.retain(|scope| demoted(*scope));
        self.pending_rotations
            .lock()
            .unwrap()
            .retain(|top, _| demoted(*top));
        let mut rotated = false;
        for scope in scopes {
            if !demoted(scope) {
                continue;
            }
            let top = self.rotation_top(scope).await;
            if !matches!(self.may_write_signed(top).await, Ok(true)) {
                continue;
            }
            // Another device of an editor above may have rotated it since key work told them.
            if self.refresh_keys(Some(vec![top])).await.is_err() || !demoted(top) {
                self.pending_rotations.lock().unwrap().remove(&top);
                continue;
            }
            match self.rotate(top).await {
                Ok(()) => {
                    self.pending_rotations.lock().unwrap().remove(&top);
                    rotated = true;
                }
                Err(error) => self.rotation_failed(top, &error),
            }
        }
        rotated
    }

    /// Notes that the rotation `tend` owes the tree under `top` failed again, so a grantee
    /// inside it, refused as `Locked` meanwhile, can be diagnosed from an editor above.
    fn rotation_failed(&self, top: Uuid, error: &ClientError) {
        let now = self.latest_believed();
        let mut pending = self.pending_rotations.lock().unwrap();
        let entry = pending.entry(top).or_insert(PendingRotation {
            top,
            since: now,
            failures: 0,
            error: String::new(),
        });
        entry.failures += 1;
        entry.error = error.to_string();
        tracing::warn!(
            %top,
            since = entry.since,
            failures = entry.failures,
            %error,
            "a rotation a demotion calls for failed again"
        );
    }

    /// The trees a demotion outdated whose rotation this device's `tend` owes and could not do
    /// yet, oldest first: until one goes through, a grantee inside the tree who reaches it only
    /// from inside is refused as `Locked`.
    pub fn pending_rotations(&self) -> Vec<PendingRotation> {
        let mut pending: Vec<PendingRotation> = self
            .pending_rotations
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect();
        pending.sort_by_key(|rotation| (rotation.since, rotation.top));
        pending
    }

    /// The key `key` is sealed to `account` with, as the logs name them: to
    /// the server, only the organization settings' scope key (`KeyBook::recipient`); any key,
    /// the escrow key among them, to an administrator, with the keys the organization's log
    /// names; an owner-only block's keys to no one (`KeyBook::owner_only_covers`); a scope
    /// key, to a grantee its block's own log names with View or above, with the keys the grant
    /// names, contact or not, or, for a kind its log opens to anyone inside, to a contact the
    /// server lists as viewing inside. That log is checked from the root this device pinned in
    /// full (a weak root is a member's word, `RootPin::Weak`); a grant or opening on
    /// authority from a block above counts as the logs above, reached through signed parent
    /// records, prove it. Key work lists
    /// only accounts the server holds a grant row for, so one it revoked is sealed to no more,
    /// signed or not. `organization` is the organization's log, checked.
    ///
    /// Each log counts only once it reaches the head the key's epoch names (`KeyBook::heads`), and
    /// then as of its head now, at or after that one: a server that withholds a revocation, a
    /// closing or a demotion made before the epoch gets it sealed to no one by that log until it
    /// serves the log that far. And only while the epoch is current by it (`Checked::current_at`):
    /// one named before an entry that lowered someone's access is sealed to no one, and when it is
    /// its scope's newest, the scope is due a rotation (`Target::Outdated`).
    async fn sealing_target(
        &self,
        key: Uuid,
        account: Uuid,
        organization: Option<&be_block_keys::Checked>,
    ) -> Target {
        if self.keys.owner_only_covers(key) {
            return Target::Nobody;
        }
        let Some(row) = self.keys.row(key) else {
            return Target::Nobody;
        };
        if self
            .keys
            .server()
            .is_some_and(|(server, _)| server == account)
        {
            return self.keys.recipient(account, key).into();
        }
        let heads = self.keys.heads(key);
        // An escrow epoch's head is the one its maker signed in its record.
        let organization_head = match row.kind {
            KeyKind::Escrow => self
                .keys
                .escrows()
                .into_iter()
                .find(|escrow| escrow.id == key)
                .map(|escrow| escrow.head),
            _ => heads.organization,
        };
        // Administering the organization gives the escrow key, and with it every key but an
        // owner-only block's: to an administrator its log names, any of them (the settings'
        // scope key and its blocks' data keys, which key work lists for them). A key naming no
        // organization head (one minted before this device read the organization's log, which
        // connecting does first) goes to no administrator this way. An administrator now may
        // hold an epoch a demotion outdates: it is written under no more (`KeyBook::outdated`), and
        // the settings' keys reach a new administrator this way. A proof path names no
        // administrator it does not prove, and hides whom a reduced entry demoted: only a whole log
        // seals.
        if let Some(administrator) = organization
            .filter(|checked| !checked.partial)
            .filter(|checked| organization_head.is_some_and(|head| checked.holds(&head)))
            .and_then(|checked| checked.administrators.get(&account.into_bytes()))
        {
            return Target::Seal(administrator.public);
        }
        if row.kind != KeyKind::Scope {
            return Target::Nobody;
        }
        let Some(block) = self.keys.block_of(key) else {
            return Target::Nobody;
        };
        let crate::keys::RootPin::Pinned(_) = self.keys.root_pin(block) else {
            return Target::Nobody;
        };
        let Ok((_, checked)) = self.checked_access(block).await else {
            return Target::Nobody;
        };
        let Some(head) = heads.block.filter(|head| checked.holds(head)) else {
            return Target::Nobody;
        };
        if checked.outdates(&head) {
            return match self.keys.scope_key(block) == Some(key) {
                true => Target::Outdated(block),
                false => Target::Nobody,
            };
        }
        let grantee = account.into_bytes();
        // A proof path (served below Edit) may hide the grantee's revocation behind a reduced
        // entry: it seals to no grantee. Its opening is closed by any reduced lowering after it
        // (`Checked::partial`), so a viewer still offers a kind its log opens.
        if !checked.partial
            && let Some((level, identity)) = checked.grants.get(&grantee)
            && *level >= be_block_keys::Level::View
        {
            return Target::Seal(identity.public);
        }
        // A kind of settings its log opens to whoever views a block inside is offered by its
        // viewers to their contacts. That the contact views inside is the server's word
        // (guides/blocks/keys.md, "Grants the server changes first"); that the kind is open is
        // the log's, so a block no manager opened is offered to no one.
        match checked.open_inside >= be_block_keys::Level::View {
            true => self.keys.contact(account).into(),
            false => Target::Nobody,
        }
    }

    pub(super) async fn save_pin(&self) -> Result<(), ClientError> {
        let Some(escrow) = self.keys.unpinned() else {
            return Ok(());
        };
        let pinned = crate::contacts::Pinned {
            root: self.keys.organization_root(),
            escrow: Some(escrow),
            made: 0,
        };
        crate::contacts::keep_pin(
            &self.connection(),
            self.account,
            self.keys.identity(),
            self.organization,
            &pinned,
        )
        .await?;
        self.adopt_pinned(pinned);
        Ok(())
    }

    pub(super) async fn rotate(&self, root: Uuid) -> Result<(), ClientError> {
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
        // Each key goes under the key of a block the signed parent records place it under, never
        // one the server only lists it below: the top under the scope or parent
        // its summary names, walked up once, and each block below placed under the one that
        // listed it, one log each.
        let top_parent = match super::parent_of(&top)? {
            BlockParent::Block(parent) => Some(parent),
            BlockParent::Root | BlockParent::Detached => None,
        };
        let top_scope = super::scope_of(&top)?;
        if top_scope != top.id {
            self.signed_below(top.id, top_scope).await?;
        } else if let Some(parent) = top_parent {
            self.signed_below(top.id, parent).await?;
        }
        let placed = futures_util::future::join_all(walked.iter().skip(1).map(|node| async {
            match super::parent_of(node)? {
                BlockParent::Block(parent) => self.placed_under(node.id, parent).await,
                BlockParent::Root | BlockParent::Detached => Err(ClientError::Forged(node.id)),
            }
        }))
        .await;
        placed
            .into_iter()
            .collect::<Result<Vec<()>, ClientError>>()?;
        // Each new epoch names the heads of the logs it is made under: read
        // now, so they are past the revocation that called for the rotation. A log that does
        // not check here leaves the head this device last held.
        let _ = self.checked_organization().await;
        for node in &walked {
            if !node.owner_only && super::scope_of(node)? == node.id {
                let _ = self.checked_access(node.id).await;
            }
        }
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
        be_block_protocol::write::<(), _>(
            &self.connection(),
            Some(self.organization),
            be_block_protocol::Op::PutKeys(be_block_protocol::PutKeys::Rotate {
                roots: vec![root],
                keys: keys.clone(),
            }),
        )
        .await??;
        self.recorded(&keys);
        Ok(())
    }

    /// Makes the next escrow epoch under the organization log's head now, sealed to each
    /// other administrator that log names who is a contact, with the keys the log names.
    pub(super) async fn make_escrow(&self) -> Result<(), ClientError> {
        self.refresh_keys(None).await?;
        let Some(checked) = self.checked_organization().await? else {
            return Ok(());
        };
        if checked.partial {
            return Ok(());
        }
        let (escrow, mut keys) = self.keys.mint_escrow(checked.head)?;
        for (account, identity) in &checked.administrators {
            let account = Uuid::from_bytes(*account);
            if account != self.account && self.keys.contact(account).is_some() {
                keys.wrapped
                    .push(self.keys.seal_to(escrow.id, account, &identity.public)?);
            }
        }
        for row in self.keys.keys_of_kind(KeyKind::Scope) {
            if self.keys.trusted(row.id)
                && let Some(wrap) = self.keys.escrow_wrap_under(row.id, &escrow)?
            {
                keys.wrapped.push(wrap);
            }
        }
        be_block_protocol::write::<(), _>(
            &self.connection(),
            Some(self.organization),
            be_block_protocol::Op::PutKeys(be_block_protocol::PutKeys::Escrow {
                escrow: escrow.clone(),
                keys: keys.clone(),
            }),
        )
        .await??;
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
        Ok(
            be_block_protocol::read::<Vec<be_block_protocol::Member>, _>(
                &self.connection(),
                Some(self.organization),
                be_block_protocol::Query::Members,
            )
            .await??,
        )
    }

    /// Sets a member's role. Making or unmaking an administrator of a company organization is a
    /// record in its log, signed here, naming the keys this device verified for them (or, to
    /// unmake one, those the log names).
    pub async fn set_role(&self, account: Uuid, role: OrganizationRole) -> Result<(), ClientError> {
        let signed = self.administrator_record(account, role).await?;
        be_block_protocol::write::<(), _>(
            &self.connection(),
            Some(self.organization),
            be_block_protocol::Op::AppendLog(be_block_protocol::LogRecord::Role {
                account: account,
                role: role,
                signed: signed,
            }),
        )
        .await??;
        Ok(())
    }

    async fn administrator_record(
        &self,
        account: Uuid,
        role: OrganizationRole,
    ) -> Result<Option<Vec<u8>>, ClientError> {
        use be_block_keys::{Administrator, Record};
        let Some(checked) = self.checked_organization().await? else {
            return Ok(None);
        };
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
        let entry = checked
            .next(
                self.keys.identity(),
                self.account.into_bytes(),
                self.organization.into_bytes(),
                Record::Administrator(Administrator {
                    account: account.into_bytes(),
                    identity,
                    administrator,
                }),
            )
            .ok_or_else(|| {
                ClientError::Refused(
                    ErrorCode::PermissionDenied,
                    "this account is not served the organization's whole log".into(),
                )
            })?;
        Ok(Some(entry.encode()))
    }

    /// Removes a member: an agent with `delete-agent`, signing it out of the organization's log
    /// when it names it; a person by signing them out of the log (`append-log`), as an
    /// administrator. Each carries the signed revocations of their grants that logs name.
    pub async fn remove_member(&self, account: Uuid) -> Result<(), ClientError> {
        let checked = self.checked_organization().await?;
        // An agent or a person as the log names them; one it names nowhere, as the server lists
        // them.
        let named = checked
            .as_ref()
            .filter(|checked| checked.member(account.into_bytes()).is_some());
        let agent = match named {
            Some(checked) => checked
                .members
                .get(&account.into_bytes())
                .is_some_and(|named| named.maker.is_some()),
            None => self
                .members()
                .await?
                .iter()
                .find(|member| member.account == account)
                .is_some_and(|member| member.created_by.is_some()),
        };
        let revocations = self.removal_revocations(account, false).await?;
        let op = match agent {
            true => be_block_protocol::Op::DeleteAgent {
                account: account,
                signed: named
                    .and_then(|checked| self.member_record(Some(checked), account, None, true)),
                revocations: revocations,
            },
            // A sign-out names no keys, so one of a person no record signed in (the server
            // added them on its own) names nothing the server says either.
            false => {
                let signed = self
                    .member_record(checked.as_ref(), account, None, false)
                    .ok_or_else(|| {
                        ClientError::Refused(
                            ErrorCode::PermissionDenied,
                            "only an administrator served the organization's whole log removes a person"
                                .into(),
                        )
                    })?;
                be_block_protocol::Op::AppendLog(be_block_protocol::LogRecord::Member {
                    signed,
                    revocations,
                })
            }
        };
        be_block_protocol::write::<(), _>(&self.connection(), Some(self.organization), op)
            .await??;
        Ok(())
    }

    /// Revokes an outstanding invitation into this organization: as an administrator, ending
    /// its key in the organization's log; as an inviter who no longer administers, with no
    /// record, since the log ended it with their role.
    pub async fn revoke_invitation(&self, link: Uuid) -> Result<(), ClientError> {
        let rows = be_block_protocol::read::<Vec<be_block_protocol::InvitationRow>, _>(
            &self.connection(),
            Some(self.organization),
            be_block_protocol::Query::Invitations,
        )
        .await??;
        let checked = self.checked_organization().await?;
        let administers = checked.as_ref().is_some_and(|checked| {
            checked
                .administrators
                .contains_key(&self.account.into_bytes())
        });
        let outstanding = rows.iter().find(|row| {
            row.id == link
                && matches!(
                    row.state,
                    be_block_protocol::InvitationState::Waiting
                        | be_block_protocol::InvitationState::Opened
                )
        });
        let signed = match (outstanding, checked.as_ref(), administers) {
            (Some(row), Some(checked), true) => checked
                .next(
                    self.keys.identity(),
                    self.account.into_bytes(),
                    self.organization.into_bytes(),
                    be_block_keys::Record::Invitation(be_block_keys::Invitation {
                        key: be_block_keys::SigningPublic(row.key),
                        open: false,
                    }),
                )
                .map(|entry| entry.encode()),
            _ => None,
        };
        be_block_protocol::write::<(), _>(
            &self.connection(),
            Some(self.organization),
            be_block_protocol::Op::DeleteLink { link, signed },
        )
        .await??;
        Ok(())
    }

    /// Removes a contact, with the signed revocations, in this organization, of each one's
    /// grants on blocks the other manages that blocks' logs name.
    pub async fn remove_contact(&self, account: Uuid) -> Result<(), ClientError> {
        let revocations = self.removal_revocations(account, true).await?;
        be_block_protocol::write::<(), _>(
            &self.connection(),
            Some(self.organization),
            be_block_protocol::Op::RemoveContact {
                account: account,
                revocations: revocations,
            },
        )
        .await??;
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
        let pairs = be_block_protocol::read::<Vec<be_block_protocol::Revocation>, _>(
            &self.connection(),
            Some(self.organization),
            be_block_protocol::Query::Revocations {
                account: account,
                contact: contact,
            },
        )
        .await??;
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

    /// Finishes the links this account made that were accepted (`contacts::finish_links`,
    /// then `finish_link` for each), deleting each only once it finished, or can never finish:
    /// one that a device stops halfway through, or that fails while the server is out of
    /// reach, is finished again on the next sync, rather than leave a contact link's grant
    /// unsigned or an invitation's accepter waiting, unsigned, for an administrator. Every link
    /// is tried; the first error, if any, is answered once they all were. An invitation's
    /// accepter signed themselves in as they accepted it; finishing makes them a contact.
    pub async fn finish_links(&self) -> Result<Vec<crate::contacts::Finished>, ClientError> {
        let finished = crate::contacts::finish_links(
            &self.connection(),
            self.account,
            self.keys.identity(),
            self.organization,
            &self.contact_pins(),
        )
        .await?;
        let mut failed = None;
        for done in &finished {
            let finish = self.finish_link(done).await;
            let unreachable = finish.as_ref().is_err_and(ClientError::is_unreachable);
            if let Err(error) = finish {
                failed.get_or_insert(error);
            }
            if unreachable {
                continue;
            }
            let deleted = be_block_protocol::write_all::<(), _>(
                &self.connection(),
                None,
                vec![
                    be_block_protocol::Op::DeleteLink {
                        link: done.link,
                        signed: None,
                    },
                    be_block_protocol::Op::DeleteMessage(done.letter),
                ],
            )
            .await;
            if let Err(error) = deleted
                .map_err(ClientError::from)
                .and_then(|deleted| deleted.map_err(ClientError::from))
            {
                failed.get_or_insert(error);
            }
            let forgotten = async {
                if let Some(block) = crate::AccountRecords::open(
                    &self.connection(),
                    self.account,
                    self.keys.identity(),
                )
                .await?
                {
                    crate::contacts::forget_link(&block, done.link).await?;
                }
                Ok::<_, ClientError>(())
            };
            if let Err(error) = forgotten.await {
                failed.get_or_insert(error);
            }
        }
        match failed {
            Some(error) => Err(error),
            None => Ok(finished),
        }
    }

    /// Finishes an accepted link in this organization, as often as it is asked to: signs a
    /// contact link's grant into the block's log and seals the block's scope to the accepter,
    /// with the keys their card, opened with the link's code, names.
    pub(crate) async fn finish_link(
        &self,
        done: &crate::contacts::Finished,
    ) -> Result<(), ClientError> {
        let identity = be_block_keys::PublicIdentity {
            public: done.card.public,
            signing: done.card.signing,
        };
        let account = Uuid::from_bytes(done.card.account);
        if let Some(grant) = done
            .grant
            .as_ref()
            .filter(|grant| grant.organization == self.organization)
        {
            // A contact link's grant, applied when it was accepted, goes in the block's log now
            // that the accepter's card names their keys; until then no one seals to them by it.
            // Only while the log names them nowhere: a later grant or revocation is in the log
            // (`signed_grant` records a revocation of a grant the server applied), and finishing
            // again must not undo it. The server's grant is checked too, for a revocation made
            // before this device knew the accepter's keys, which could name no one.
            let (_, checked) = self.checked_access(grant.block).await?;
            let level = match checked.grants.get(&account.into_bytes()) {
                Some((level, _)) => *level,
                None => {
                    let held = self
                        .list_access(grant.block)
                        .await?
                        .entries
                        .iter()
                        .any(|entry| {
                            entry.account == account && entry.granted == Some(grant.access)
                        });
                    if !held {
                        return Ok(());
                    }
                    self.grant_as(grant.block, account, grant.access, Some(identity))
                        .await?;
                    super::grant_level(grant.access)
                }
            };
            if level >= be_block_keys::Level::View {
                self.seal_share(grant.block, account, &done.card.public)
                    .await?;
            }
        }
        Ok(())
    }

    /// The organization's log checked from the root this account pinned; none when it has no
    /// log (the personal organization). The key book keeps it as checked
    /// (`KeyBook::set_organization`): what the heads of an epoch made here name.
    pub async fn checked_organization(
        &self,
    ) -> Result<Option<be_block_keys::Checked>, ClientError> {
        let Some(pinned) = self.keys.organization_root() else {
            return Ok(None);
        };
        let entries = self.organization_log().await?;
        // Checked on from what this device kept, as a block's is (`Peer::checked_access`).
        let kept = self
            .keys
            .kept_checked(self.organization, pinned)
            .filter(|kept| kept.positions.len() <= entries.len());
        let checked = kept
            .unwrap_or_else(|| be_block_keys::Checked::unchecked(pinned))
            .resume(
                be_block_keys::LogKind::Organization,
                self.organization.into_bytes(),
                &entries,
                &|_| None,
            )
            .map_err(|_| ClientError::Forged(self.organization))?;
        // Kept as a block's is (`KeyBook::note_log`): one short of what was kept is withheld,
        // and changes nothing here.
        match self.keys.served(self.organization, &checked) {
            crate::keys::Served::Take => {}
            crate::keys::Served::Stale(newer) => return Ok(Some(newer.0)),
            crate::keys::Served::Withheld => {
                return Err(ClientError::Withheld(self.organization));
            }
        }
        if let Some(kept) = self.keys.note_log(self.organization, &checked, &entries) {
            let _ = self.refs.keep_log(self.organization, kept);
        }
        self.keys.set_organization(Some(&checked));
        Ok(Some(checked))
    }

    /// A member record signing `account` into the organization's log as it checks at its head,
    /// with `identity`, or with none out of it: none when it has no log.
    pub(crate) fn member_record(
        &self,
        checked: Option<&be_block_keys::Checked>,
        account: Uuid,
        identity: Option<be_block_keys::PublicIdentity>,
        agent: bool,
    ) -> Option<Vec<u8>> {
        let member = identity.is_some();
        let entry = checked?.next(
            self.keys.identity(),
            self.account.into_bytes(),
            self.organization.into_bytes(),
            be_block_keys::Record::Member(be_block_keys::Member {
                account: account.into_bytes(),
                identity,
                member,
                agent,
                invitation: None,
            }),
        )?;
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
        Ok(self.member_record(checked.as_ref(), account, Some(identity), true))
    }

    /// Signs `account`, an agent this account made, into the organization's log with the keys
    /// this device made for it. A person signs themselves in as they accept an invitation
    /// (`contacts::accept_link`).
    pub async fn sign_in_agent(
        &self,
        account: Uuid,
        identity: be_block_keys::PublicIdentity,
    ) -> Result<(), ClientError> {
        let checked = self.checked_organization().await?;
        if checked
            .as_ref()
            .is_some_and(|checked| checked.member(account.into_bytes()).is_some())
        {
            return Ok(());
        }
        let Some(signed) = self.member_record(checked.as_ref(), account, Some(identity), true)
        else {
            return Ok(());
        };
        be_block_protocol::write::<(), _>(
            &self.connection(),
            Some(self.organization),
            be_block_protocol::Op::AppendLog(be_block_protocol::LogRecord::Member {
                signed,
                revocations: Vec::new(),
            }),
        )
        .await??;
        Ok(())
    }

    /// Keeps the organization's log in step with the agents this account made: signs in those
    /// key work lists as signed in by no record, and signs out those the log names that the
    /// server removed on its own (collected with their block, or deleted on taking an account
    /// back). A person signs themselves in as they accept an invitation, so a person listed
    /// unsigned is one the server added on its own: that they are a member is the server's word
    /// alone, and they stay listed (`unsigned_members`) for an administrator to remove.
    async fn tend_agents(&self, unsigned: &[Uuid]) -> Result<(), ClientError> {
        let Some(checked) = self.checked_organization().await? else {
            return Ok(());
        };
        let me = self.account.into_bytes();
        let made: Vec<[u8; 16]> = checked
            .members
            .iter()
            .filter(|(_, named)| named.maker == Some(me))
            .map(|(account, _)| *account)
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
                self.sign_in_agent(member.account, identity).await?;
            }
        }
        for account in made {
            let account = Uuid::from_bytes(account);
            if members.iter().any(|member| member.account == account) {
                continue;
            }
            let checked = self.checked_organization().await?;
            if let Some(signed) = self.member_record(checked.as_ref(), account, None, true) {
                be_block_protocol::write::<(), _>(
                    &self.connection(),
                    Some(self.organization),
                    be_block_protocol::Op::AppendLog(be_block_protocol::LogRecord::Member {
                        signed,
                        revocations: Vec::new(),
                    }),
                )
                .await??;
            }
        }
        Ok(())
    }
}

/// Whom key work seals a key to (`Peer::sealing_target`).
enum Target {
    Seal(RecoveryPublic),
    Nobody,
    /// The key is its scope's newest epoch and the scope's log outdates it: the scope is due a
    /// rotation.
    Outdated(Uuid),
}

impl From<Option<RecoveryPublic>> for Target {
    fn from(public: Option<RecoveryPublic>) -> Self {
        public.map_or(Target::Nobody, Target::Seal)
    }
}
