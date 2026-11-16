//! Signed access records (plans/signed_access.md): each block, and each company organization,
//! has a log of entries signed by the members who made them, which devices check back to the
//! log's root before they act on who may do what.
//!
//! An entry names the hash of the entry before it, so its place in the log orders it. The root
//! is the first entry; a device takes its hash only from somewhere the server cannot write
//! (making the log itself, or a key that carries it, `CarriedKey`), so a server that serves a log
//! rooted elsewhere is caught.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use std::collections::HashMap;

use crate::{KeyError, PublicIdentity, RecoverySecret, SigningPublic, verify_signature};

/// What an access record's signature covers comes after this, so it never verifies as anything
/// else the account's signing key signs (authorship has a domain of its own).
const ACCESS_INFO: &[u8] = b"be3.access.v1";
/// What an entry's hash covers comes after this.
const ACCESS_HASH_INFO: &[u8] = b"be3.access.hash.v1";
/// What an old signing key signs to hand an account's records on to its new keys comes after
/// this (`Retirement::endorsement`).
const RETIREMENT_INFO: &[u8] = b"be3.access.retirement.v1";

/// The hash of an access log entry, by which the next entry and a device's pin name it.
pub type EntryHash = [u8; 32];

/// A record in an access log.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Record {
    /// The first entry of a log: who made the block or organization, with the keys they made it
    /// with.
    Root(Root),
    /// An account took a new recovery phrase: its records after this are signed with `new`
    /// (plans/signed_access.md, "A new recovery phrase").
    Retirement(Retirement),
    /// A member's access to the block, as a member who may give it gave it; revoking is a grant
    /// of `Level::None`.
    Grant(Grant),
    /// Who administers the organization, in its log.
    Administrator(Administrator),
    /// Who is a member of the organization, in its log.
    Member(Member),
}

/// A member's access to a block, from least to most (`be_block_graph::Access`).
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Level {
    None,
    KnowExists,
    View,
    Edit,
    Manage,
}

/// `grantee`'s access, with the keys the granter verified for them, which a device seals to,
/// and where the granter's own right to grant it comes from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Grant {
    pub grantee: [u8; 16],
    pub identity: PublicIdentity,
    pub level: Level,
    pub authority: Authority,
}

/// Where a signer's Manage comes from. Named in the record, as of an entry of another log, so a
/// log checks the same however later entries of that log change: a grant an administrator
/// signed still checks after they are demoted, and one signed after it does not.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Authority {
    /// This log's own grants (or a signer revoking their own grant).
    Log,
    /// Administering the organization, as of this entry of its log.
    Organization(EntryHash),
    /// Manage on a block above, as of this entry of its log. That the block is above comes from
    /// the server's tree until parents are signed (plans/signed_access.md, step 4).
    Inherited { block: [u8; 16], head: EntryHash },
}

/// Whether `account`, with the keys the administrator signing verified, administers the
/// organization.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Administrator {
    pub account: [u8; 16],
    pub identity: PublicIdentity,
    pub administrator: bool,
}

/// Whether `account`, with the keys its signer verified, is a member of the organization: a
/// person an administrator signed in (from the invitation they made), or an agent the member
/// signing made (`agent`), whose maker may sign it out again.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Member {
    pub account: [u8; 16],
    pub identity: PublicIdentity,
    pub member: bool,
    pub agent: bool,
}

/// The account id of the agent `maker` makes with `identity`: derived from both, so a member
/// record signing in an agent can claim no account but one made for those keys by its signer,
/// never a person's.
pub fn agent_account(maker: [u8; 16], identity: &PublicIdentity) -> [u8; 16] {
    let hash = Sha256::new()
        .chain_update(b"be3.agent.v1")
        .chain_update(maker)
        .chain_update(identity.public.0)
        .chain_update(identity.signing.0)
        .finalize();
    let mut account = [0; 16];
    account.copy_from_slice(&hash[..16]);
    account
}

/// A member as the organization's log names them: their keys, and for an agent, its maker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Named {
    pub identity: PublicIdentity,
    pub maker: Option<[u8; 16]>,
}

/// An account's move to new keys, endorsed by the key it retires and signed (as an entry) by
/// the new one: the server cannot retire an account to keys of its own, and the old key signs
/// nothing in the log after it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Retirement {
    pub account: [u8; 16],
    pub old: SigningPublic,
    pub new: PublicIdentity,
    #[serde(with = "serde_bytes")]
    pub endorsement: Vec<u8>,
}

fn endorsed(account: [u8; 16], new: &PublicIdentity) -> Vec<u8> {
    [RETIREMENT_INFO, &account, &new.public.0, &new.signing.0].concat()
}

/// What a log is the log of, so a block's records never verify as an organization's, nor the
/// other way round.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum LogKind {
    Block,
    Organization,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Root {
    pub kind: LogKind,
    pub creator: [u8; 16],
    pub identity: PublicIdentity,
}

/// One signed entry of the log of `log` (a block or organization id).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Entry {
    pub log: [u8; 16],
    pub previous: Option<EntryHash>,
    pub record: Record,
    pub signer: [u8; 16],
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
}

#[derive(Serialize)]
struct Signed<'a> {
    log: [u8; 16],
    previous: Option<EntryHash>,
    record: &'a Record,
    signer: [u8; 16],
}

fn signed(
    log: [u8; 16],
    previous: Option<EntryHash>,
    record: &Record,
    signer: [u8; 16],
) -> Vec<u8> {
    let signed = Signed {
        log,
        previous,
        record,
        signer,
    };
    let mut bytes = ACCESS_INFO.to_vec();
    bytes.extend(postcard::to_stdvec(&signed).expect("an access record encodes"));
    bytes
}

impl Entry {
    /// `record`, signed by account `signer` with `secret`, at the place after `previous` in the
    /// log of `log`.
    pub fn sign(
        secret: &RecoverySecret,
        signer: [u8; 16],
        log: [u8; 16],
        previous: Option<EntryHash>,
        record: Record,
    ) -> Self {
        use ed25519_dalek::Signer;
        let signature = secret
            .signing_key()
            .sign(&signed(log, previous, &record, signer))
            .to_bytes()
            .to_vec();
        Self {
            log,
            previous,
            record,
            signer,
            signature,
        }
    }

    /// The root of the `kind` log of `log`, made by `creator` with `secret`.
    pub fn root(secret: &RecoverySecret, creator: [u8; 16], kind: LogKind, log: [u8; 16]) -> Self {
        Self::sign(
            secret,
            creator,
            log,
            None,
            Record::Root(Root {
                kind,
                creator,
                identity: secret.identity(),
            }),
        )
    }

    /// The entry after `previous` in the log of `log` that retires `account`'s `old` keys for
    /// `new`'s: endorsed by the old key, signed by the new one.
    pub fn retirement(
        old: &RecoverySecret,
        new: &RecoverySecret,
        account: [u8; 16],
        log: [u8; 16],
        previous: EntryHash,
    ) -> Self {
        use ed25519_dalek::Signer;
        let endorsement = old
            .signing_key()
            .sign(&endorsed(account, &new.identity()))
            .to_bytes()
            .to_vec();
        Self::sign(
            new,
            account,
            log,
            Some(previous),
            Record::Retirement(Retirement {
                account,
                old: old.signing(),
                new: new.identity(),
                endorsement,
            }),
        )
    }

    /// Whether the signature is `signing`'s over this entry.
    pub fn signed_by(&self, signing: &crate::SigningPublic) -> bool {
        verify_signature(
            &signing.0,
            &signed(self.log, self.previous, &self.record, self.signer),
            &self.signature,
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("an access entry encodes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, KeyError> {
        postcard::from_bytes(bytes).map_err(|_| KeyError::Record)
    }

    pub fn hash(&self) -> EntryHash {
        Sha256::new()
            .chain_update(ACCESS_HASH_INFO)
            .chain_update(self.encode())
            .finalize()
            .into()
    }

    /// The root this entry is, when it is the well-formed, self-signed first entry of the `kind`
    /// log of `log`.
    pub fn checked_root(&self, kind: LogKind, log: [u8; 16]) -> Result<Root, KeyError> {
        let Record::Root(root) = &self.record else {
            return Err(KeyError::Record);
        };
        let well_formed = root.kind == kind
            && self.log == log
            && self.previous.is_none()
            && self.signer == root.creator;
        match well_formed && self.signed_by(&root.identity.signing) {
            true => Ok(*root),
            false => Err(KeyError::Record),
        }
    }
}

/// A key as it travels sealed or wrapped: the key, the block (or, for an escrow key, the
/// organization) it is a key of, and the hash of that log's root when the sender holds it
/// pinned. Sealed together, so a server that relabels a key row as another block's, or serves a
/// block's log rooted elsewhere, is caught by whoever opens it. A sender that holds no pinned
/// root (it reached the block only through escrow, say) sends none, and the key pins nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarriedKey {
    pub key: [u8; 32],
    pub block: [u8; 16],
    pub root: Option<EntryHash>,
}

impl CarriedKey {
    const BARE: usize = 32 + 16 + 1;

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = [self.key.as_slice(), &self.block].concat();
        match &self.root {
            Some(root) => {
                bytes.push(1);
                bytes.extend_from_slice(root);
            }
            None => bytes.push(0),
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, KeyError> {
        let (head, root) = bytes
            .split_at_checked(Self::BARE)
            .ok_or(KeyError::Corrupt)?;
        let (key, rest) = head.split_at(32);
        let (block, flag) = rest.split_at(16);
        let root = match (flag, root.len()) {
            ([0], 0) => None,
            ([1], 32) => Some(root.try_into().map_err(|_| KeyError::Corrupt)?),
            _ => return Err(KeyError::Corrupt),
        };
        Ok(Self {
            key: key.try_into().map_err(|_| KeyError::Corrupt)?,
            block: block.try_into().map_err(|_| KeyError::Corrupt)?,
            root,
        })
    }
}

/// A log checked from its root to its head (`check_log`): the hash of its head, the signing key
/// each account that holds a record in it signs with now, and what the records give.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Checked {
    pub head: EntryHash,
    pub signers: HashMap<[u8; 16], SigningPublic>,
    /// For a block's log: each account's access as its records give it (the creator's is
    /// Manage), with the keys last named for it.
    pub grants: HashMap<[u8; 16], (Level, PublicIdentity)>,
    /// For an organization's log: its administrators, with their keys.
    pub administrators: HashMap<[u8; 16], PublicIdentity>,
    /// For an organization's log: its members other than through administering it.
    pub members: HashMap<[u8; 16], Named>,
}

/// Checks the `kind` log of `log` from its root, which must be the one `pinned`, to its head:
/// every entry names the hash of the one before and is signed by its signer's current key, and is
/// one its signer may make at its place: a retirement only of the signer's own keys, endorsed by
/// the key it retires; a grant to another account only by a signer with Manage (from the log or
/// from `outside`), and anyone's revocation of their own grant; an administrator record only by
/// an administrator. Anything else makes the whole log refused, since a device acts only on a
/// log it checked to its head. A grant names where its signer's Manage comes from
/// (`Authority`); `outside` answers for another log, as of the entry the grant names, with the
/// signer's key there if they held Manage then.
pub fn check_log(
    kind: LogKind,
    log: [u8; 16],
    pinned: EntryHash,
    entries: &[Entry],
    outside: &dyn Fn([u8; 16], &Authority) -> Option<SigningPublic>,
) -> Result<Checked, KeyError> {
    let (root, rest) = entries.split_first().ok_or(KeyError::Record)?;
    let checked = root.checked_root(kind, log)?;
    if root.hash() != pinned {
        return Err(KeyError::Record);
    }
    let mut state = Checked {
        head: pinned,
        signers: HashMap::from([(checked.creator, checked.identity.signing)]),
        ..Checked::default()
    };
    match kind {
        LogKind::Block => {
            state
                .grants
                .insert(checked.creator, (Level::Manage, checked.identity));
        }
        LogKind::Organization => {
            state
                .administrators
                .insert(checked.creator, checked.identity);
        }
    }
    for entry in rest {
        if entry.log != log || entry.previous != Some(state.head) {
            return Err(KeyError::Record);
        }
        let own = state.signers.get(&entry.signer).copied();
        match &entry.record {
            Record::Root(_) => return Err(KeyError::Record),
            Record::Retirement(retirement) => {
                let current = own.ok_or(KeyError::Record)?;
                let endorsed_by_current = retirement.account == entry.signer
                    && retirement.old == current
                    && verify_signature(
                        &current.0,
                        &endorsed(retirement.account, &retirement.new),
                        &retirement.endorsement,
                    );
                if !endorsed_by_current || !entry.signed_by(&retirement.new.signing) {
                    return Err(KeyError::Record);
                }
                state.signers.insert(entry.signer, retirement.new.signing);
                for (account, identity) in state
                    .grants
                    .iter_mut()
                    .map(|(account, (_, identity))| (account, identity))
                    .chain(state.administrators.iter_mut())
                    .chain(
                        state
                            .members
                            .iter_mut()
                            .map(|(account, named)| (account, &mut named.identity)),
                    )
                {
                    if *account == entry.signer {
                        *identity = retirement.new;
                    }
                }
            }
            Record::Grant(grant) => {
                let level = state
                    .grants
                    .get(&entry.signer)
                    .map_or(Level::None, |(level, _)| *level);
                let own_revocation = grant.grantee == entry.signer && grant.level == Level::None;
                let signing = match grant.authority {
                    Authority::Log if own_revocation || level == Level::Manage => own,
                    Authority::Log => None,
                    // Signed with the key this log names for them when it names one: a key
                    // retired here signs nothing after, whatever the other log was as of then.
                    ref other => outside(entry.signer, other).map(|signing| own.unwrap_or(signing)),
                };
                // Authorship is Manage from the root (and, later, owner records) alone: no grant
                // lowers or names the creator.
                let may = kind == LogKind::Block
                    && grant.grantee != checked.creator
                    && signing.is_some_and(|signing| entry.signed_by(&signing));
                if !may {
                    return Err(KeyError::Record);
                }
                // One who grants from outside the log holds a record in it now, retired here as
                // in any log they signed in.
                if let Some(signing) = signing {
                    state.signers.entry(entry.signer).or_insert(signing);
                }
                state.signers.insert(grant.grantee, grant.identity.signing);
                state
                    .grants
                    .insert(grant.grantee, (grant.level, grant.identity));
            }
            Record::Administrator(administrator) => {
                let may = kind == LogKind::Organization
                    && state.administrators.contains_key(&entry.signer)
                    && own.is_some_and(|signing| entry.signed_by(&signing));
                if !may {
                    return Err(KeyError::Record);
                }
                state
                    .signers
                    .insert(administrator.account, administrator.identity.signing);
                match administrator.administrator {
                    true => {
                        state
                            .administrators
                            .insert(administrator.account, administrator.identity);
                    }
                    false => {
                        state.administrators.remove(&administrator.account);
                    }
                }
            }
            Record::Member(member) => {
                let signed = own.is_some_and(|signing| entry.signed_by(&signing));
                let administrator = state.administrators.contains_key(&entry.signer);
                // A person is signed in only by an administrator; an agent by a member who is
                // no agent, its maker; a member out by an administrator, or an agent by its maker.
                let maker = state
                    .members
                    .get(&member.account)
                    .and_then(|named| named.maker);
                let person = state
                    .members
                    .get(&entry.signer)
                    .is_some_and(|named| named.maker.is_none());
                // No one already named is signed in again: their keys change only by their own
                // retirement, or by signing them out and in, both in the log.
                let named = state.members.contains_key(&member.account)
                    || state.administrators.contains_key(&member.account);
                let may = match (member.member, member.agent) {
                    (true, false) => administrator && !named,
                    (true, true) => {
                        (administrator || person)
                            && !named
                            && member.account == agent_account(entry.signer, &member.identity)
                    }
                    (false, _) => administrator || maker == Some(entry.signer),
                };
                if kind != LogKind::Organization || !signed || !may {
                    return Err(KeyError::Record);
                }
                state
                    .signers
                    .insert(member.account, member.identity.signing);
                match member.member {
                    true => {
                        state.members.insert(
                            member.account,
                            Named {
                                identity: member.identity,
                                maker: member.agent.then_some(entry.signer),
                            },
                        );
                    }
                    // Signed out, an administrator administers no more.
                    false => {
                        state.members.remove(&member.account);
                        state.administrators.remove(&member.account);
                    }
                }
            }
        }
        state.head = entry.hash();
    }
    Ok(state)
}

/// The log as `check_log` checks it up to `as_of`, one of its entries, which a grant names as
/// where its signer's Manage comes from (`Authority`). Refused when the log has no such entry.
pub fn check_log_as_of(
    kind: LogKind,
    log: [u8; 16],
    pinned: EntryHash,
    entries: &[Entry],
    as_of: EntryHash,
    outside: &dyn Fn([u8; 16], &Authority) -> Option<SigningPublic>,
) -> Result<Checked, KeyError> {
    let end = entries
        .iter()
        .position(|entry| entry.hash() == as_of)
        .ok_or(KeyError::Record)?;
    check_log(kind, log, pinned, &entries[..=end], outside)
}

impl Checked {
    /// `account`'s keys, if the organization's log names them a member or an administrator:
    /// those an escrow wrap from them is opened with.
    pub fn member(&self, account: [u8; 16]) -> Option<PublicIdentity> {
        self.administrators
            .get(&account)
            .copied()
            .or_else(|| self.members.get(&account).map(|named| named.identity))
    }

    /// `account`'s signing key, if it manages what the log is of: Manage on a block, or
    /// administering an organization.
    pub fn manager(&self, kind: LogKind, account: [u8; 16]) -> Option<SigningPublic> {
        let manages = match kind {
            LogKind::Block => self
                .grants
                .get(&account)
                .is_some_and(|(level, _)| *level == Level::Manage),
            LogKind::Organization => self.administrators.contains_key(&account),
        };
        manages
            .then(|| self.signers.get(&account).copied())
            .flatten()
    }
}

/// The logs `answer` reads another log's grants from, and the tree it trusts for which block is
/// above which (the server's until parents are signed).
pub struct Logs<'a> {
    pub organization: [u8; 16],
    /// A log's pinned root and its entries, root first, as far as the reader holds it.
    pub log: &'a dyn Fn(LogKind, [u8; 16]) -> Option<(EntryHash, Vec<Entry>)>,
    /// Whether the first block is above the second.
    pub above: &'a dyn Fn([u8; 16], [u8; 16]) -> bool,
}

/// `check_log`'s `outside` for the log of `block`, from `logs`: the signer's key in the log a
/// grant names its authority in, checked as of the entry it names, if they managed there then.
pub fn answer(
    logs: &Logs,
    block: [u8; 16],
    signer: [u8; 16],
    authority: &Authority,
) -> Option<SigningPublic> {
    let (kind, log, head) = match *authority {
        Authority::Log => return None,
        Authority::Organization(head) => (LogKind::Organization, logs.organization, head),
        Authority::Inherited { block: above, head } if (logs.above)(above, block) => {
            (LogKind::Block, above, head)
        }
        Authority::Inherited { .. } => return None,
    };
    let (pinned, entries) = (logs.log)(kind, log)?;
    let outside = |signer: [u8; 16], authority: &Authority| answer(logs, log, signer, authority);
    check_log_as_of(kind, log, pinned, &entries, head, &outside)
        .ok()?
        .manager(kind, signer)
}
