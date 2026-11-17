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

use std::collections::{HashMap, HashSet, VecDeque};

use crate::{KeyError, PublicIdentity, RecoverySecret, SigningPublic, verify_signature};

/// What an access record's signature covers comes after this, so it never verifies as anything
/// else the account's signing key signs (authorship has a domain of its own).
const ACCESS_INFO: &[u8] = b"be3.access.v1";
/// What an entry's hash covers comes after this: the hash of the entry before it, whether it
/// lowered anyone's access, and its digest (`Entry::digest`), so an entry reduced to those three
/// (`Reduced`) hashes as the whole entry does.
const ACCESS_HASH_INFO: &[u8] = b"be3.access.hash.v1";
/// What an entry's digest covers comes after this.
const ACCESS_DIGEST_INFO: &[u8] = b"be3.access.digest.v1";
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
    /// A kind of settings opened to whoever views a block inside, or closed again, as a member
    /// who may give access gave it: a grant to no one by name, so it names no keys.
    Opening(Opening),
    /// Where the block lies, as a member with Edit or above placed it: made there, or moved
    /// (plans/signed_access.md, step 4).
    Parent(Parent),
}

impl Record {
    /// Where a grant or an opening says its signer's Manage comes from.
    pub fn authority(&self) -> Option<Authority> {
        match self {
            Record::Grant(grant) => Some(grant.authority),
            Record::Opening(opening) => Some(opening.authority),
            Record::Parent(parent) => Some(parent.authority),
            _ => None,
        }
    }
}

/// Whoever views a block inside may read this kind of settings (`Level::View`), or no longer
/// (`Level::None`), and where the signer's right to say so comes from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Opening {
    pub level: Level,
    pub authority: Authority,
}

/// Where a block lies, as a parent record (`Record::Parent`) places it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Placement {
    /// At the top of the organization (`BlockParent::Root`).
    Top,
    /// In Recently Deleted (`BlockParent::Detached`).
    Detached,
    /// Under `block`, whose log is rooted at `root`: so a device checks the logs above a block
    /// from roots its signed placements carry, never from the server's word.
    Block { block: [u8; 16], root: EntryHash },
}

/// The block placed where `placement` says, by a signer whose Edit or above on it comes from
/// `authority`, as of where it lay before.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Parent {
    pub placement: Placement,
    pub authority: Authority,
}

/// A member's access to a block, from least to most (`be_block_graph::Access`).
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub enum Level {
    #[default]
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
    /// Manage (Edit, for a parent record) on a block above, as of this entry of its log. Above
    /// as the signed parent records place it (`answer`): the block's own as of the record's place,
    /// and those of the blocks between, as they ever placed them.
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
///
/// `lowers` says whether it lowered anyone's access (a revocation, a downgrade, a closing; a
/// demotion, or an administrator signed out), as `check_log` finds it must: its signer sets it
/// (`Checked::next`) and any other value makes the log refused. The entry's hash covers it
/// outside the digest, so a reader served the entry reduced (`Reduced`, plans/signed_access.md,
/// "Who is served what") still knows which epochs it outdates.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Entry {
    pub log: [u8; 16],
    pub previous: Option<EntryHash>,
    pub lowers: bool,
    pub record: Record,
    pub signer: [u8; 16],
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
}

#[derive(Serialize)]
struct Signed<'a> {
    log: [u8; 16],
    previous: Option<EntryHash>,
    lowers: bool,
    record: &'a Record,
    signer: [u8; 16],
}

fn signed(
    log: [u8; 16],
    previous: Option<EntryHash>,
    lowers: bool,
    record: &Record,
    signer: [u8; 16],
) -> Vec<u8> {
    let signed = Signed {
        log,
        previous,
        lowers,
        record,
        signer,
    };
    let mut bytes = ACCESS_INFO.to_vec();
    bytes.extend(postcard::to_stdvec(&signed).expect("an access record encodes"));
    bytes
}

impl Entry {
    /// `record`, signed by account `signer` with `secret`, at the place after `previous` in the
    /// log of `log`, saying whether it `lowers` anyone's access there (`Checked::lowers`). A
    /// device appending to a log it checked signs with `Checked::next`, which says both.
    pub fn sign(
        secret: &RecoverySecret,
        signer: [u8; 16],
        log: [u8; 16],
        previous: Option<EntryHash>,
        lowers: bool,
        record: Record,
    ) -> Self {
        use ed25519_dalek::Signer;
        let signature = secret
            .signing_key()
            .sign(&signed(log, previous, lowers, &record, signer))
            .to_bytes()
            .to_vec();
        Self {
            log,
            previous,
            lowers,
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
            false,
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
            false,
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
            &signed(
                self.log,
                self.previous,
                self.lowers,
                &self.record,
                self.signer,
            ),
            &self.signature,
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("an access entry encodes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, KeyError> {
        postcard::from_bytes(bytes).map_err(|_| KeyError::Record)
    }

    /// The hash the next entry, and a device's pin, name this one by.
    pub fn hash(&self) -> EntryHash {
        self.reduced().hash()
    }

    /// What the entry's hash covers beside the hash before it and `lowers`: all of it.
    pub fn digest(&self) -> EntryHash {
        Sha256::new()
            .chain_update(ACCESS_DIGEST_INFO)
            .chain_update(self.encode())
            .finalize()
            .into()
    }

    /// The entry as a proof path serves it to a reader who needs only the chain through it.
    pub fn reduced(&self) -> Reduced {
        Reduced {
            previous: self.previous,
            lowers: self.lowers,
            digest: self.digest(),
        }
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

/// An entry reduced to what chains it (plans/signed_access.md, "Who is served what"): the hash of
/// the entry before it, whether it lowered anyone's access, and the digest of the rest, which
/// hash as the whole entry does (`Entry::hash`). It says nothing of whom it is about.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Reduced {
    pub previous: Option<EntryHash>,
    pub lowers: bool,
    pub digest: EntryHash,
}

impl Reduced {
    pub fn hash(&self) -> EntryHash {
        let mut hash = Sha256::new().chain_update(ACCESS_HASH_INFO);
        match self.previous {
            Some(previous) => hash = hash.chain_update([1]).chain_update(previous),
            None => hash = hash.chain_update([0]),
        }
        hash.chain_update([u8::from(self.lowers)])
            .chain_update(self.digest)
            .finalize()
            .into()
    }
}

/// An entry of a log as the server serves it: whole, or reduced (`Reduced`) on a proof path.
/// Members with Edit or above are served every entry whole; readers below Edit a proof path
/// (`proof_path`), whole only where an entry proves their own right or a sealer's.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Step {
    Entry(Entry),
    Reduced(Reduced),
}

impl Step {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("an access log step encodes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, KeyError> {
        postcard::from_bytes(bytes).map_err(|_| KeyError::Record)
    }

    pub fn hash(&self) -> EntryHash {
        match self {
            Step::Entry(entry) => entry.hash(),
            Step::Reduced(reduced) => reduced.hash(),
        }
    }

    /// The entry, when served whole.
    pub fn entry(&self) -> Option<&Entry> {
        match self {
            Step::Entry(entry) => Some(entry),
            Step::Reduced(_) => None,
        }
    }
}

impl From<Entry> for Step {
    fn from(entry: Entry) -> Self {
        Step::Entry(entry)
    }
}

/// What `check_log` reads a log from: whole entries (`Entry`), or a proof path (`Step`).
pub trait Logged {
    fn step(&self) -> StepRef<'_>;
}

/// An entry of a log, whole or reduced, as `check_log` reads it.
#[derive(Clone, Copy)]
pub enum StepRef<'a> {
    Entry(&'a Entry),
    Reduced(&'a Reduced),
}

impl Logged for Entry {
    fn step(&self) -> StepRef<'_> {
        StepRef::Entry(self)
    }
}

impl Logged for Step {
    fn step(&self) -> StepRef<'_> {
        match self {
            Step::Entry(entry) => StepRef::Entry(entry),
            Step::Reduced(reduced) => StepRef::Reduced(reduced),
        }
    }
}

impl StepRef<'_> {
    fn hash(&self) -> EntryHash {
        match self {
            StepRef::Entry(entry) => entry.hash(),
            StepRef::Reduced(reduced) => reduced.hash(),
        }
    }

    fn previous(&self) -> Option<EntryHash> {
        match self {
            StepRef::Entry(entry) => entry.previous,
            StepRef::Reduced(reduced) => reduced.previous,
        }
    }
}

/// `entries`, a whole log of `kind`, as a proof path for `reader`, a member below Edit
/// (plans/signed_access.md, "Who is served what"): the entries that prove the access of the
/// reader and of each of `sealers` (whoever sealed it a key, which it judges the seal by) whole,
/// and every other reduced to what chains it (`Reduced`), so the reader learns nothing of other
/// members.
///
/// Whole: the root; every entry naming an account the path proves (its grants, administrator
/// and member records, retirements), starting from the reader and the sealers and taking in the
/// signer of each entry whole, so each signer's own right at its place checks too; every entry
/// the reader signed (an agent it made), and so the accounts those name; every opening of a
/// kind to anyone inside, which names no one; every parent record, so the reader walks where the
/// block lies (and the signers' own right there); and in an organization's log every administrator
/// record and every record about someone who ever administered it, so no demotion is hidden
/// there: an honest organization path reduces no entry that lowered anyone.
pub fn proof_path(
    kind: LogKind,
    entries: &[Entry],
    reader: [u8; 16],
    sealers: impl IntoIterator<Item = [u8; 16]>,
) -> Vec<Step> {
    let administrators: HashSet<[u8; 16]> = entries
        .iter()
        .filter_map(|entry| match &entry.record {
            Record::Administrator(made) => Some(made.account),
            Record::Root(root) if kind == LogKind::Organization => Some(root.creator),
            _ => None,
        })
        .collect();
    let named = |entry: &Entry| match &entry.record {
        Record::Root(root) => Some(root.creator),
        Record::Grant(grant) => Some(grant.grantee),
        Record::Administrator(made) => Some(made.account),
        Record::Member(member) => Some(member.account),
        Record::Retirement(retirement) => Some(retirement.account),
        Record::Opening(_) | Record::Parent(_) => None,
    };
    let always = |entry: &Entry| match &entry.record {
        Record::Root(_) | Record::Opening(_) | Record::Administrator(_) | Record::Parent(_) => true,
        Record::Member(member) => administrators.contains(&member.account),
        Record::Retirement(retirement) => administrators.contains(&retirement.account),
        Record::Grant(_) => false,
    };
    let mut accounts: HashSet<[u8; 16]> = sealers.into_iter().collect();
    accounts.insert(reader);
    let mut whole = vec![false; entries.len()];
    loop {
        let mut grew = false;
        for (at, entry) in entries.iter().enumerate() {
            let proves = always(entry)
                || entry.signer == reader
                || named(entry).is_some_and(|named| accounts.contains(&named));
            if whole[at] || !proves {
                continue;
            }
            whole[at] = true;
            grew |= accounts.insert(entry.signer);
            grew |= named(entry).is_some_and(|named| accounts.insert(named));
        }
        if !grew {
            break;
        }
    }
    entries
        .iter()
        .zip(whole)
        .map(|(entry, whole)| match whole {
            true => Step::Entry(entry.clone()),
            false => Step::Reduced(entry.reduced()),
        })
        .collect()
}

/// A key as it travels sealed or wrapped: the key, the block (or, for an escrow key, the
/// organization) it is a key of, the hash of that log's root when the sender holds it pinned,
/// and the heads of the logs its epoch was made under (`Heads`). Sealed together, so a server
/// that relabels a key row as another block's, serves a block's log rooted elsewhere, or names
/// an older head for an epoch, is caught by whoever opens it. A sender that holds no pinned root
/// (it reached the block only through escrow, say) sends none, and the key pins nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarriedKey {
    pub key: [u8; 32],
    pub block: [u8; 16],
    pub root: Option<EntryHash>,
    pub heads: Heads,
}

/// The heads of the access logs a key's epoch was made under, as its maker held them
/// (plans/signed_access.md, 3d): a device seals or wraps the epoch only to those granted as of
/// a head at or after these, once it holds the log up to them, so a server that withholds a
/// revocation, a closing or a demotion from it gets the epoch sealed to no one meanwhile. Each
/// is none where its maker held no such log, and an epoch naming none is sealed by that log to
/// no one.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Heads {
    /// The log of the scope it is a key of: the block's own for a scope key, the scope's it is
    /// wrapped under for a data key; none for an escrow key.
    pub block: Option<EntryHash>,
    /// The organization's log; none in an organization with no log.
    pub organization: Option<EntryHash>,
}

impl CarriedKey {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = [self.key.as_slice(), &self.block].concat();
        for hash in [self.root, self.heads.block, self.heads.organization] {
            match hash {
                Some(hash) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&hash);
                }
                None => bytes.push(0),
            }
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, KeyError> {
        let (key, rest) = bytes.split_at_checked(32).ok_or(KeyError::Corrupt)?;
        let (block, mut rest) = rest.split_at_checked(16).ok_or(KeyError::Corrupt)?;
        let mut hash = || -> Result<Option<EntryHash>, KeyError> {
            let (flag, after) = rest.split_first().ok_or(KeyError::Corrupt)?;
            let (found, after) = match flag {
                0 => (None, after),
                1 => {
                    let (hash, after) = after.split_at_checked(32).ok_or(KeyError::Corrupt)?;
                    (Some(hash.try_into().map_err(|_| KeyError::Corrupt)?), after)
                }
                _ => return Err(KeyError::Corrupt),
            };
            rest = after;
            Ok(found)
        };
        let root = hash()?;
        let heads = Heads {
            block: hash()?,
            organization: hash()?,
        };
        if !rest.is_empty() {
            return Err(KeyError::Corrupt);
        }
        Ok(Self {
            key: key.try_into().map_err(|_| KeyError::Corrupt)?,
            block: block.try_into().map_err(|_| KeyError::Corrupt)?,
            root,
            heads,
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
    /// For a block's log: whether its last opening (`Record::Opening`) opened it to whoever
    /// views a block inside.
    pub open_inside: Level,
    /// For a block's log: where its parent records placed it, by place, oldest first; the last
    /// is where it lies now (`placement`).
    pub placements: Vec<(usize, Placement)>,
    /// Each entry's place in the log, root first: whether the log reaches a head an epoch
    /// names (`holds`).
    pub positions: HashMap<EntryHash, usize>,
    /// Each account's level as the log changed it, by place (`level_as_of`): for a block's log,
    /// its grants (the creator's Manage at the root); for an organization's, administering it
    /// as Manage.
    pub levels: HashMap<[u8; 16], Vec<(usize, Level)>>,
    /// The place of the newest entry that lowered anyone's access (a revocation, a downgrade,
    /// a closing; in an organization's log, a demotion or an administrator signed out), or the
    /// root's: an epoch named under a head before it is outdated (`outdates`). On a proof path,
    /// a reduced entry that lowered anyone counts, whomever it lowered.
    pub last_lowering: usize,
    /// Whether a reduced entry lowered anyone: on a block's path, someone else's revocation; on
    /// an organization's, which reduces none that did when honest (`proof_path`), a demotion
    /// hidden from the reader, so the log names no administrator (`administrators` and `levels`
    /// are left empty) and vouches for no escrow epoch.
    pub hides_lowering: bool,
    /// Whether the log was served as a proof path (`proof_path`): some entries reduced, so
    /// `grants`, `levels`, `administrators` and `members` hold only those it proves, and a
    /// later reduced entry may have lowered any of them. Only `positions`, `head`,
    /// `last_lowering` (so `holds`, `current_at` and `outdates`), and `open_inside`, closed by
    /// any reduced lowering after it, hold as for a whole log. Nothing is sealed to anyone, nor
    /// signed, by a partial log (plans/signed_access.md, "Who is served what").
    pub partial: bool,
}

impl Checked {
    /// Whether the log as checked reaches `head`: it is one of its entries.
    pub fn holds(&self, head: &EntryHash) -> bool {
        self.positions.contains_key(head)
    }

    /// `account`'s level as of `head`, one of the log's entries: none when the log does not
    /// reach it or named no level for them by then.
    pub fn level_as_of(&self, account: [u8; 16], head: &EntryHash) -> Option<Level> {
        let at = *self.positions.get(head)?;
        self.levels
            .get(&account)?
            .iter()
            .take_while(|(place, _)| *place <= at)
            .last()
            .map(|(_, level)| *level)
    }

    /// Whether an epoch named under `head` is current by this log: the log reaches `head`, and
    /// nothing after it lowered anyone's access, so all it was sealed to may hold it still.
    pub fn current_at(&self, head: &EntryHash) -> bool {
        self.positions
            .get(head)
            .is_some_and(|at| *at >= self.last_lowering)
    }

    /// Whether an epoch named under `head` is outdated by this log: the log reaches `head`, and
    /// an entry after it lowered someone's access, whom the epoch may have been sealed to.
    pub fn outdates(&self, head: &EntryHash) -> bool {
        self.positions
            .get(head)
            .is_some_and(|at| *at < self.last_lowering)
    }

    /// Whether `record`, appended at the head of this log, lowers anyone's access, as
    /// `check_log` will find it: what its entry must say (`Entry::lowers`).
    pub fn lowers(&self, record: &Record) -> bool {
        let last = |account: &[u8; 16]| {
            self.levels
                .get(account)
                .and_then(|changes| changes.last())
                .map(|(_, level)| *level)
        };
        match record {
            Record::Grant(grant) => last(&grant.grantee).is_some_and(|before| grant.level < before),
            Record::Opening(opening) => opening.level < self.open_inside,
            Record::Administrator(made) => {
                !made.administrator
                    && last(&made.account).is_some_and(|before| before > Level::None)
            }
            Record::Member(member) => {
                !member.member
                    && self.administrators.contains_key(&member.account)
                    && last(&member.account).is_some_and(|before| before > Level::None)
            }
            Record::Root(_) | Record::Retirement(_) | Record::Parent(_) => false,
        }
    }

    /// Where the block lies as its log last placed it: none for a log that places it nowhere
    /// (an organization's).
    pub fn placement(&self) -> Option<Placement> {
        self.placements.last().map(|(_, placement)| *placement)
    }

    /// `record`, signed by `signer` with `secret` as the next entry of the log of `log` after
    /// this head, saying whether it lowers anyone (`lowers`). None on a partial log, which
    /// hides what most records must follow, but for those a reader below Edit makes about
    /// itself or what it made, whose `lowers` the path proves (`proof_path`): a grant to the
    /// signer (a reader leaving), and signing an agent in or out (only an administrator, whose
    /// records a path never reduces, lowers anyone so).
    pub fn next(
        &self,
        secret: &RecoverySecret,
        signer: [u8; 16],
        log: [u8; 16],
        record: Record,
    ) -> Option<Entry> {
        let own = match &record {
            Record::Grant(grant) => grant.grantee == signer,
            Record::Member(member) => member.agent,
            _ => false,
        };
        if self.partial && !own {
            return None;
        }
        let lowers = self.lowers(&record);
        Some(Entry::sign(
            secret,
            signer,
            log,
            Some(self.head),
            lowers,
            record,
        ))
    }

    fn set_level(&mut self, account: [u8; 16], level: Level) {
        let at = self.positions.len() - 1;
        let held = self.levels.entry(account).or_default();
        if held.last().is_some_and(|(_, before)| level < *before) {
            self.last_lowering = at;
        }
        held.push((at, level));
    }

    /// Checks that `entry`, a grant, an opening or a parent record naming `authority`, is
    /// signed by someone who holds `needs` at its place (from this log or from `outside`, or
    /// anyone revoking their own grant), and notes its signer's key.
    fn granting(
        &mut self,
        entry: &Entry,
        own: Option<SigningPublic>,
        authority: Authority,
        needs: Level,
        own_revocation: bool,
        outside: &dyn Fn(&Asked) -> Option<SigningPublic>,
    ) -> Result<(), KeyError> {
        let level = self
            .grants
            .get(&entry.signer)
            .map_or(Level::None, |(level, _)| *level);
        let signing = match authority {
            Authority::Log if own_revocation || level >= needs => own,
            Authority::Log => None,
            // Signed with the key this log names for them when it names one: a key retired
            // here signs nothing after, whatever the other log was as of then.
            ref other => {
                let placement = self.placement();
                let asked = Asked {
                    signer: entry.signer,
                    authority: other,
                    level: needs,
                    placement: placement.as_ref(),
                };
                outside(&asked).map(|signing| own.unwrap_or(signing))
            }
        };
        let Some(signing) = signing.filter(|signing| entry.signed_by(signing)) else {
            return Err(KeyError::Record);
        };
        // One who grants from outside the log holds a record in it now, retired here as in any
        // log they signed in.
        self.signers.entry(entry.signer).or_insert(signing);
        Ok(())
    }
}

/// Checks the `kind` log of `log` from its root, which must be the one `pinned`, to its head:
/// every entry names the hash of the one before and is signed by its signer's current key, and is
/// one its signer may make at its place: a retirement only of the signer's own keys, endorsed by
/// the key it retires; a grant to another account only by a signer with Manage (from the log or
/// from `outside`), and anyone's revocation of their own grant; an administrator record only by
/// an administrator. Anything else makes the whole log refused, since a device acts only on a
/// log it checked to its head. A grant names where its signer's Manage comes from
/// (`Authority`); `outside` answers for another log, as of the entry the grant names, with the
/// signer's key there if they held what the record needs then (`Asked`). A parent record is
/// made by a signer with Edit or above, and places the block anywhere but under itself.
///
/// A proof path (`Step::Reduced` among the entries) checks the same where whole; a reduced entry
/// is taken for what its hash commits to (the chain, and whether it lowered anyone), and the log
/// is `partial`. The root is never reduced.
pub fn check_log(
    kind: LogKind,
    log: [u8; 16],
    pinned: EntryHash,
    entries: &[impl Logged],
    outside: &dyn Fn(&Asked) -> Option<SigningPublic>,
) -> Result<Checked, KeyError> {
    let (root, rest) = entries.split_first().ok_or(KeyError::Record)?;
    let StepRef::Entry(root) = root.step() else {
        return Err(KeyError::Record);
    };
    let checked = root.checked_root(kind, log)?;
    if root.lowers {
        return Err(KeyError::Record);
    }
    if root.hash() != pinned {
        return Err(KeyError::Record);
    }
    let mut state = Checked {
        head: pinned,
        signers: HashMap::from([(checked.creator, checked.identity.signing)]),
        positions: HashMap::from([(pinned, 0)]),
        ..Checked::default()
    };
    state.set_level(checked.creator, Level::Manage);
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
    // The place of the last opening, and of the last reduced entry that lowered anyone: one after
    // the opening may have closed it.
    let mut opened_at = 0;
    let mut hidden_lowering = 0;
    for step in rest {
        let step = step.step();
        if step.previous() != Some(state.head) {
            return Err(KeyError::Record);
        }
        let hash = step.hash();
        let place = state.positions.len();
        if state.positions.insert(hash, place).is_some() {
            return Err(KeyError::Record);
        }
        let entry = match step {
            StepRef::Entry(entry) => entry,
            // Reduced on a proof path: whom it is about is hidden, so a lowering may have closed
            // the kind or lowered anyone the path names.
            StepRef::Reduced(reduced) => {
                state.partial = true;
                if reduced.lowers {
                    state.last_lowering = place;
                    state.hides_lowering = true;
                    hidden_lowering = place;
                }
                state.head = hash;
                continue;
            }
        };
        if entry.log != log {
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
                let own_revocation = grant.grantee == entry.signer && grant.level == Level::None;
                // Authorship is Manage from the root (and, later, owner records) alone: no grant
                // lowers or names the creator.
                if kind != LogKind::Block || grant.grantee == checked.creator {
                    return Err(KeyError::Record);
                }
                state.granting(
                    entry,
                    own,
                    grant.authority,
                    Level::Manage,
                    own_revocation,
                    outside,
                )?;
                state.signers.insert(grant.grantee, grant.identity.signing);
                state
                    .grants
                    .insert(grant.grantee, (grant.level, grant.identity));
                state.set_level(grant.grantee, grant.level);
            }
            Record::Opening(opening) => {
                // Open to view, or closed: no other level, so the record has one meaning.
                if kind != LogKind::Block || !matches!(opening.level, Level::None | Level::View) {
                    return Err(KeyError::Record);
                }
                state.granting(
                    entry,
                    own,
                    opening.authority,
                    Level::Manage,
                    false,
                    outside,
                )?;
                if opening.level < state.open_inside {
                    state.last_lowering = state.positions.len() - 1;
                }
                state.open_inside = opening.level;
                opened_at = place;
            }
            Record::Parent(parent) => {
                let under_itself = matches!(
                    parent.placement,
                    Placement::Block { block, .. } if block == log
                );
                if kind != LogKind::Block || under_itself {
                    return Err(KeyError::Record);
                }
                state.granting(entry, own, parent.authority, Level::Edit, false, outside)?;
                state.placements.push((place, parent.placement));
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
                        state.set_level(administrator.account, Level::Manage);
                    }
                    false => {
                        state.administrators.remove(&administrator.account);
                        state.set_level(administrator.account, Level::None);
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
                        if state.administrators.remove(&member.account).is_some() {
                            state.set_level(member.account, Level::None);
                        }
                    }
                }
            }
        }
        // The entry says whether it lowered anyone, as it did: its hash commits a reader of a
        // proof path to that.
        if entry.lowers != (state.last_lowering == place) {
            return Err(KeyError::Record);
        }
        state.head = hash;
    }
    if hidden_lowering > opened_at {
        state.open_inside = Level::None;
    }
    // An honest path of an organization's log reduces nothing that lowered anyone
    // (`proof_path`): one that does hides whom it demoted, so it names no administrator, nor
    // anyone's administering as of any entry, for taking a seal, an escrow wrap or an epoch.
    if kind == LogKind::Organization && state.hides_lowering {
        state.administrators.clear();
        state.levels.clear();
    }
    Ok(state)
}

/// The log as `check_log` checks it up to `as_of`, one of its entries, which a grant names as
/// where its signer's Manage comes from (`Authority`). Refused when the log has no such entry.
pub fn check_log_as_of(
    kind: LogKind,
    log: [u8; 16],
    pinned: EntryHash,
    entries: &[impl Logged],
    as_of: EntryHash,
    outside: &dyn Fn(&Asked) -> Option<SigningPublic>,
) -> Result<Checked, KeyError> {
    let end = entries
        .iter()
        .position(|entry| entry.step().hash() == as_of)
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
        self.holding(kind, account, Level::Manage)
    }

    /// `account`'s signing key, if it holds `level` or above on what the log is of: on a block,
    /// by its grants; on an organization, administering it counts as Manage.
    pub fn holding(&self, kind: LogKind, account: [u8; 16], level: Level) -> Option<SigningPublic> {
        let holds = match kind {
            LogKind::Block => self
                .grants
                .get(&account)
                .is_some_and(|(held, _)| *held >= level),
            LogKind::Organization => {
                level <= Level::Manage && self.administrators.contains_key(&account)
            }
        };
        holds.then(|| self.signers.get(&account).copied()).flatten()
    }
}

/// What `check_log` asks `outside` of a record naming its signer's right elsewhere: whether
/// `signer` held `level` or above in the log `authority` names, as of the entry it names, with
/// the block's `placement` as of the record's place (where an `Authority::Inherited` is above
/// from).
pub struct Asked<'a> {
    pub signer: [u8; 16],
    pub authority: &'a Authority,
    pub level: Level,
    pub placement: Option<&'a Placement>,
}

/// How many logs deep `answer` follows authority named outside a log, and how many blocks it
/// walks up looking for an ancestor: more than any honest tree needs.
pub const MAX_ANCESTRY: usize = 32;

/// The logs `answer` reads another log's grants from. Which block is above which comes from the
/// parent records in them alone, and a block's log is checked from the root the record placing
/// something under it names.
pub struct Logs<'a> {
    pub organization: [u8; 16],
    /// The root the reader pinned for the organization's log: none for one with no log.
    pub organization_root: Option<EntryHash>,
    /// A log's entries, root first, as far as the reader holds it.
    pub log: &'a dyn Fn(LogKind, [u8; 16]) -> Option<Vec<Step>>,
}

/// `check_log`'s `outside` for the log of `block`, from `logs`: the signer's key in the log a
/// record names its authority in, checked as of the entry it names, if they held what it needs
/// there then. For `Authority::Inherited`, the block named must be above: reached from where
/// `block` lay at the record's place through the parent records of the blocks between (each as
/// it ever placed itself), and its log is checked from the root the placement under it names.
pub fn answer(logs: &Logs, block: [u8; 16], asked: &Asked) -> Option<SigningPublic> {
    answer_within(logs, block, asked, MAX_ANCESTRY)
}

fn answer_within(
    logs: &Logs,
    block: [u8; 16],
    asked: &Asked,
    depth: usize,
) -> Option<SigningPublic> {
    let depth = depth.checked_sub(1)?;
    let (kind, log, pinned, head) = match *asked.authority {
        Authority::Log => return None,
        Authority::Organization(head) => (
            LogKind::Organization,
            logs.organization,
            logs.organization_root?,
            head,
        ),
        Authority::Inherited { block: above, head } if above != block => {
            let root = ancestor_root(logs, *asked.placement?, above, depth)?;
            (LogKind::Block, above, root, head)
        }
        Authority::Inherited { .. } => return None,
    };
    let entries = (logs.log)(kind, log)?;
    let outside = |asked: &Asked| answer_within(logs, log, asked, depth);
    check_log_as_of(kind, log, pinned, &entries, head, &outside)
        .ok()?
        .holding(kind, asked.signer, asked.level)
}

/// The root of `target`'s log, as the placement under it names it, when `target` is reached
/// from `from` going up: `from` itself, or any block a block between ever placed itself under,
/// each checked from the root the placement reaching it names.
fn ancestor_root(
    logs: &Logs,
    from: Placement,
    target: [u8; 16],
    depth: usize,
) -> Option<EntryHash> {
    let mut queue = VecDeque::from([from]);
    let mut seen = HashSet::new();
    while let Some(placement) = queue.pop_front() {
        let Placement::Block { block, root } = placement else {
            continue;
        };
        if block == target {
            return Some(root);
        }
        if !seen.insert(block) || seen.len() > MAX_ANCESTRY {
            continue;
        }
        let Some(entries) = (logs.log)(LogKind::Block, block) else {
            continue;
        };
        let outside = |asked: &Asked| answer_within(logs, block, asked, depth);
        let Ok(checked) = check_log(LogKind::Block, block, root, &entries, &outside) else {
            continue;
        };
        queue.extend(checked.placements.iter().map(|(_, placement)| *placement));
    }
    None
}
