//! Signed access records (guides/blocks/keys.md): each block, and each company organization,
//! has a log of entries signed by the members who made them, which devices check back to the
//! log's root before they act on who may do what.
//!
//! An entry names the hash of the entry before it, so its place in the log orders it. The root
//! is the first entry; a device takes its hash only from somewhere the server cannot write
//! (making the log itself, or a key that carries it, `CarriedKey`), so a server that serves a log
//! rooted elsewhere is caught.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::{
    InviteCode, KeyError, PublicIdentity, RecoverySecret, SigningPublic, verify_signature,
};

/// What an access record's signature covers comes after this, so it never verifies as anything
/// else the account's signing key signs (authorship has a domain of its own).
const ACCESS_INFO: &[u8] = b"be3.access.v1";
/// What an entry's hash covers comes after this: the hash of the entry before it, whether it
/// lowered anyone's access, whether it is a parent record, and its digest (`Entry::digest`), so
/// an entry reduced to those (`Reduced`) hashes as the whole entry does.
const ACCESS_HASH_INFO: &[u8] = b"be3.access.hash.v1";
/// What an entry's digest covers comes after this.
const ACCESS_DIGEST_INFO: &[u8] = b"be3.access.digest.v1";
/// What an old signing key signs to hand an account's records on to its new keys comes after
/// this (`Retirement::endorsement`).
const RETIREMENT_INFO: &[u8] = b"be3.access.retirement.v1";
/// What an accepter's signing key signs to show it holds the keys an invitation signs it in with
/// comes after this (`Invited::endorsement`).
const INVITED_INFO: &[u8] = b"be3.access.invited.v1";
/// What an opener's signing key signs to show it holds the keys a share link grants to comes
/// after this (`Opened::endorsement`).
const OPENED_INFO: &[u8] = b"be3.access.opened.v1";

/// The hash of an access log entry, by which the next entry and a device's pin name it.
pub type EntryHash = [u8; 32];

/// A record in an access log.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Record {
    /// The first entry of a log: who made the block or organization, with the keys they made it
    /// with.
    Root(Root),
    /// An account took a new recovery phrase: its records after this are signed with `new`
    /// (guides/blocks/keys.md, "Identity, new phrases and devices").
    Retirement(Retirement),
    /// A member's access to the block, as a member who may give it gave it; revoking is a grant
    /// of `Level::None`.
    Grant(Grant),
    /// Who administers the organization, in its log.
    Administrator(Administrator),
    /// Who is a member of the organization, in its log.
    Member(Member),
    /// An invitation into the organization, opened or ended by an administrator, in its log:
    /// the key its link's code derives, which signs one person in.
    Invitation(Invitation),
    /// A kind of settings opened to whoever views a block inside, or closed again, as a member
    /// who may give access gave it: a grant to no one by name, so it names no keys.
    Opening(Opening),
    /// Where the block lies, as a member with Edit or above placed it: made there, or moved
    /// (guides/blocks/keys.md, "Parent records").
    Parent(Parent),
    /// Ownership of the block moves to another account, as its owner gave it
    /// (guides/blocks/keys.md, "Owner records"): the new owner holds what authorship gives, Manage,
    /// and the former keeps Edit.
    Owner(Owner),
    /// A share link made, or revoked, by a member who may give access (guides/blocks/keys.md,
    /// "Share links"): its key, derived from the link's code, signs the grant of whoever opens
    /// it (`Opened`).
    ShareLink(ShareLink),
    /// An account granted itself access through an open share link: signed with the link's key,
    /// which only a holder of the code has, and endorsed by the account's own keys.
    Opened(Opened),
}

impl Record {
    /// Where a grant, an opening or a parent record says its signer's right comes from.
    pub fn authority(&self) -> Option<&Authority> {
        match self {
            Record::Grant(grant) => Some(&grant.authority),
            Record::Opening(opening) => Some(&opening.authority),
            Record::Parent(parent) => Some(&parent.authority),
            Record::ShareLink(link) => Some(&link.authority),
            _ => None,
        }
    }
}

/// Whoever views a block inside may read this kind of settings (`Level::View`), or no longer
/// (`Level::None`), and where the signer's right to say so comes from.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Opening {
    pub level: Level,
    pub authority: Authority,
}

/// The block's new owner, with the keys the owner signing verified for them, which a device
/// seals to as to a grantee's.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Owner {
    pub account: [u8; 16],
    pub identity: PublicIdentity,
}

/// Who may open a share link, as its maker chose within the organization's policy. The server
/// alone checks it: no device can tell who is on the server or in an organization it is not in.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Admits {
    /// Members of the block's organization (in the personal organization, every account).
    #[default]
    Members,
    /// Any account on the server; one from outside the organization joins it as a guest.
    Anyone,
}

/// A share link on the block: `key`, derived from the link's code
/// (`InviteCode::share_key`), signs the grant of whoever opens it, at `level` at most, while it
/// is `open`. Made and revoked by a member with Manage, as `authority` says; revoking it lowers
/// everyone granted through it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ShareLink {
    pub key: SigningPublic,
    pub level: Level,
    pub admits: Admits,
    pub open: bool,
    pub authority: Authority,
}

/// `grantee`'s access through the share link whose key is `key`, with the keys it holds, which
/// endorse the record (`Entry::opened`): the server never holds the code, so it grants no one,
/// nor can a holder of the code grant keys it does not hold.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Opened {
    pub grantee: [u8; 16],
    pub identity: PublicIdentity,
    pub level: Level,
    pub key: SigningPublic,
    #[serde(with = "serde_bytes")]
    pub endorsement: Vec<u8>,
}

fn opened(
    block: [u8; 16],
    grantee: [u8; 16],
    identity: &PublicIdentity,
    key: &SigningPublic,
    level: Level,
) -> Vec<u8> {
    let level = postcard::to_stdvec(&level).expect("a level encodes");
    [
        OPENED_INFO,
        &block,
        &grantee,
        &identity.public.0,
        &identity.signing.0,
        &key.0,
        &level,
    ]
    .concat()
}

/// What the former owner keeps once an owner record moves ownership on: they may still edit, but
/// grant nothing more.
pub const FORMER_OWNER: Level = Level::Edit;

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
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
    /// May connect to the block's host (plans/hosted_blocks.md); sealed nothing.
    Join,
    View,
    Edit,
    Manage,
}

/// `grantee`'s access, with the keys the granter verified for them, which a device seals to,
/// and where the granter's own right to grant it comes from.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Grant {
    pub grantee: [u8; 16],
    pub identity: PublicIdentity,
    pub level: Level,
    pub authority: Authority,
}

/// Where a signer's Manage comes from. Named in the record, as of an entry of another log, so a
/// log checks the same however later entries of that log change: a grant an administrator
/// signed still checks after they are demoted, and one signed after it does not.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Authority {
    /// This log's own grants (or a signer revoking their own grant).
    Log,
    /// Administering the organization, as of this entry of its log.
    Organization(EntryHash),
    /// Manage (Edit, for a parent record) on a block above, as of the last link: the chain of
    /// blocks up to it, each named with the head of its log the record rests on. The first is
    /// where the block lay at the record's place, and each next where the one before lay as of
    /// its head (`answer`), each log checked from the root the placement under it names. The
    /// server takes the record only with every head the head of its log now.
    Inherited(Vec<Link>),
}

impl Authority {
    /// The block an `Authority::Inherited` rests on: its chain's last.
    pub fn above(&self) -> Option<[u8; 16]> {
        match self {
            Authority::Inherited(chain) => chain.last().map(|link| link.block),
            Authority::Log | Authority::Organization(_) => None,
        }
    }
}

/// A block on an `Authority::Inherited` chain, as of an entry of its log.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Link {
    pub block: [u8; 16],
    pub head: EntryHash,
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
/// person who accepted an invitation (`invitation`), or an agent the member signing made
/// (`agent`), whose maker may sign it out again. Signed out by an administrator, or an agent by
/// its maker.
///
/// A person signs themselves in (the entry's signer is `account`) with the key of the
/// invitation they accept (`Entry::accepted`), naming the keys they hold, which endorse the
/// record: the server never holds the invitation's code, so it cannot sign anyone in, nor can a
/// holder of the code sign in keys it does not hold; and no administrator need be online.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Member {
    pub account: [u8; 16],
    /// The keys signed in; none for a sign-out, which names no keys.
    pub identity: Option<PublicIdentity>,
    pub member: bool,
    pub agent: bool,
    /// For a person signed in, the invitation that signs the entry; none for anything else.
    pub invitation: Option<Invited>,
}

/// The invitation a person signed in with: its key (`Invitation::key`), which signs the entry,
/// and the person's own signing key's endorsement of the record, so the keys it names are ones
/// the accepter holds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Invited {
    pub key: SigningPublic,
    #[serde(with = "serde_bytes")]
    pub endorsement: Vec<u8>,
}

fn invited(
    organization: [u8; 16],
    account: [u8; 16],
    identity: &PublicIdentity,
    key: &SigningPublic,
) -> Vec<u8> {
    [
        INVITED_INFO,
        &organization,
        &account,
        &identity.public.0,
        &identity.signing.0,
        &key.0,
    ]
    .concat()
}

/// An invitation into the organization, as an administrator opened it (`open`) or ended it: its
/// key, derived from the link's code (`InviteCode::invitation_key`), signs in the one person who
/// accepts it, as an editor. Its signer must still administer the organization then; one used,
/// ended or never opened signs no one. Expiry is the server's alone: no device can check the time
/// a server keeps.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Invitation {
    pub key: SigningPublic,
    pub open: bool,
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
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
/// outside the digest, so a reader served the entry reduced (`Reduced`, guides/blocks/keys.md,
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

    /// The entry after `previous` in the log of `organization` that signs `account`, with the
    /// keys of `secret`, in as a member: signed with the key of the invitation `code` made
    /// (`Invitation`), so only whoever holds the code can make it, and endorsed by `secret`, so
    /// it names only keys their holder signs in.
    pub fn accepted(
        code: &InviteCode,
        organization: [u8; 16],
        previous: EntryHash,
        account: [u8; 16],
        secret: &RecoverySecret,
    ) -> Self {
        use ed25519_dalek::Signer;
        let key = code.invitation_signing(organization);
        let public = SigningPublic(key.verifying_key().to_bytes());
        let identity = secret.identity();
        let endorsement = secret
            .signing_key()
            .sign(&invited(organization, account, &identity, &public))
            .to_bytes()
            .to_vec();
        let record = Record::Member(Member {
            account,
            identity: Some(identity),
            member: true,
            agent: false,
            invitation: Some(Invited {
                key: public,
                endorsement,
            }),
        });
        Self::invitation_signed(&key, organization, previous, account, record)
    }

    /// The entry after `previous` in the log of `block` that grants `account`, with the keys of
    /// `secret`, `level` through the share link `code` made (`ShareLink`): signed with the link's
    /// key, so only whoever holds the code can make it, and endorsed by `secret`.
    pub fn opened(
        code: &InviteCode,
        block: [u8; 16],
        previous: EntryHash,
        account: [u8; 16],
        secret: &RecoverySecret,
        level: Level,
    ) -> Self {
        use ed25519_dalek::Signer;
        let key = code.share_signing(block);
        let public = SigningPublic(key.verifying_key().to_bytes());
        let identity = secret.identity();
        let endorsement = secret
            .signing_key()
            .sign(&opened(block, account, &identity, &public, level))
            .to_bytes()
            .to_vec();
        let record = Record::Opened(Opened {
            grantee: account,
            identity,
            level,
            key: public,
            endorsement,
        });
        Self::invitation_signed(&key, block, previous, account, record)
    }

    /// `record`, the member record signing `account` in, signed with an invitation's `key` after
    /// `previous` in the log of `organization`.
    pub(crate) fn invitation_signed(
        key: &ed25519_dalek::SigningKey,
        organization: [u8; 16],
        previous: EntryHash,
        account: [u8; 16],
        record: Record,
    ) -> Self {
        use ed25519_dalek::Signer;
        let signature = key
            .sign(&signed(
                organization,
                Some(previous),
                false,
                &record,
                account,
            ))
            .to_bytes()
            .to_vec();
        Self {
            log: organization,
            previous: Some(previous),
            lowers: false,
            record,
            signer: account,
            signature,
        }
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
            places: matches!(self.record, Record::Parent(_)),
            owns: matches!(self.record, Record::Owner(_)),
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

/// An entry reduced to what chains it (guides/blocks/keys.md, "Who is served what"): the hash of
/// the entry before it, whether it lowered anyone's access, and the digest of the rest, which
/// hash as the whole entry does (`Entry::hash`). It says nothing of whom it is about.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Reduced {
    pub previous: Option<EntryHash>,
    pub lowers: bool,
    /// Whether it is a parent record: a block's placement after one reduced so is unknown to
    /// the reader (`Checked::placement`), so a server that hides a move hides nothing it is
    /// trusted for.
    pub places: bool,
    /// Whether it is an owner record: one reduced so still lowers its signer, the owner then,
    /// to what a former owner keeps, so a server that hides a transfer leaves no one the old
    /// owner's Manage (`check_log`).
    pub owns: bool,
    pub digest: EntryHash,
}

impl Reduced {
    pub fn hash(&self) -> EntryHash {
        let mut hash = Sha256::new().chain_update(ACCESS_HASH_INFO);
        match self.previous {
            Some(previous) => hash = hash.chain_update([1]).chain_update(previous),
            None => hash = hash.chain_update([0]),
        }
        hash.chain_update([
            u8::from(self.lowers),
            u8::from(self.places),
            u8::from(self.owns),
        ])
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
/// (guides/blocks/keys.md, "Who is served what"): the entries that prove the access of the
/// reader and of each of `sealers` (whoever sealed it a key, which it judges the seal by) whole,
/// and every other reduced to what chains it (`Reduced`), so the reader learns nothing of other
/// members.
///
/// Whole: the root; every entry naming an account the path proves (its grants, administrator
/// and member records, retirements), starting from the reader and the sealers and taking in the
/// signer of each entry whole, so each signer's own right at its place checks too; every entry
/// the reader signed (an agent it made), and so the accounts those name; every opening of a
/// kind to anyone inside, which names no one; every parent record, so the reader walks where the
/// block lies (and the signers' own right there); every owner record, so the reader knows who
/// owns the block and that its former owners grant no more; and in an organization's log every
/// administrator record and every record about someone who ever administered it, so no demotion is
/// hidden there: an honest organization path reduces no entry that lowered anyone; and every
/// invitation record, so a member signed in by one checks.
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
        Record::Owner(owner) => Some(owner.account),
        Record::Opened(opened) => Some(opened.grantee),
        Record::Opening(_) | Record::Parent(_) | Record::Invitation(_) | Record::ShareLink(_) => {
            None
        }
    };
    let always = |entry: &Entry| match &entry.record {
        Record::Root(_)
        | Record::Opening(_)
        | Record::Administrator(_)
        | Record::Parent(_)
        | Record::Owner(_)
        | Record::Invitation(_)
        | Record::ShareLink(_) => true,
        Record::Member(member) => administrators.contains(&member.account),
        Record::Retirement(retirement) => administrators.contains(&retirement.account),
        Record::Grant(_) | Record::Opened(_) => false,
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

/// A key as it travels sealed or wrapped: the key; its own row (`Label`: its id, kind and
/// epoch) and the block (or, for an escrow key, the organization) it is a key of; the hash of
/// that log's root when the sender holds it pinned; and the heads of the logs its epoch was made
/// under (`Heads`). Sealed together, so a server that relabels a key row as another block's or
/// another epoch's (a rewrap of an old key served as the newest), serves a block's log rooted
/// elsewhere, or names an older head for an epoch, is caught by whoever opens it. A sender that
/// holds no pinned root (it reached the block only through escrow, say) sends none, and the key
/// pins nothing.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CarriedKey {
    pub key: [u8; 32],
    pub label: Label,
    pub block: [u8; 16],
    pub root: Option<EntryHash>,
    pub heads: Heads,
}

impl std::fmt::Debug for CarriedKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CarriedKey")
            .field("key", &"..")
            .field("label", &self.label)
            .field("block", &self.block)
            .field("root", &self.root)
            .field("heads", &self.heads)
            .finish()
    }
}

/// Which key row a carried key is: its id, kind and epoch, as its maker made it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Label {
    pub id: [u8; 16],
    pub kind: KeyRole,
    pub epoch: u64,
}

/// A key's kind, as a key row names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyRole {
    Data,
    Scope,
    Escrow,
}

impl KeyRole {
    fn byte(self) -> u8 {
        match self {
            Self::Data => 0,
            Self::Scope => 1,
            Self::Escrow => 2,
        }
    }

    fn from_byte(byte: u8) -> Result<Self, KeyError> {
        match byte {
            0 => Ok(Self::Data),
            1 => Ok(Self::Scope),
            2 => Ok(Self::Escrow),
            _ => Err(KeyError::Corrupt),
        }
    }
}

/// The heads of the access logs a key's epoch was made under, as its maker held them
/// (guides/blocks/keys.md, "Epochs name their heads"): a device seals or wraps the epoch only to
/// those granted as of a head at or after these, once it holds the log up to them, so a server that
/// withholds a revocation, a closing or a demotion from it gets the epoch sealed to no one
/// meanwhile. Each is none where its maker held no such log, and an epoch naming none is sealed by
/// that log to no one.
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
        let mut bytes = [
            self.key.as_slice(),
            &self.label.id,
            &[self.label.kind.byte()],
            &self.label.epoch.to_be_bytes(),
            &self.block,
        ]
        .concat();
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
        let (id, rest) = rest.split_at_checked(16).ok_or(KeyError::Corrupt)?;
        let (kind, rest) = rest.split_first().ok_or(KeyError::Corrupt)?;
        let (epoch, rest) = rest.split_at_checked(8).ok_or(KeyError::Corrupt)?;
        let label = Label {
            id: id.try_into().map_err(|_| KeyError::Corrupt)?,
            kind: KeyRole::from_byte(*kind)?,
            epoch: u64::from_be_bytes(epoch.try_into().map_err(|_| KeyError::Corrupt)?),
        };
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
            label,
            block: block.try_into().map_err(|_| KeyError::Corrupt)?,
            root,
            heads,
        })
    }
}

/// A log checked from its root to its head (`check_log`): the hash of its head, the signing key
/// each account that holds a record in it signs with now, and what the records give. A device
/// keeps it, and checks only what is appended after its head (`extend`, `resume`).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Checked {
    pub head: EntryHash,
    /// The hash of the log's root, which the device pinned.
    pub root: EntryHash,
    pub signers: HashMap<[u8; 16], SigningPublic>,
    /// For a block's log: each account's access as its records give it (the creator's is
    /// Manage), with the keys last named for it.
    pub grants: HashMap<[u8; 16], (Level, PublicIdentity)>,
    /// For an organization's log: its administrators, with their keys.
    pub administrators: HashMap<[u8; 16], PublicIdentity>,
    /// For an organization's log: its members other than through administering it.
    pub members: HashMap<[u8; 16], Named>,
    /// For an organization's log: each invitation key its records named, with who opened it
    /// while it is open, and none once it signed someone in or was ended (`Invitation`).
    pub invitations: HashMap<SigningPublic, Option<[u8; 16]>>,
    /// For a block's log: each share link's key its records named, with the most it grants while
    /// it is open, and none once revoked (`ShareLink`).
    pub share_links: HashMap<SigningPublic, Option<Level>>,
    /// For a block's log: whom a share link granted their access, by the link's key, while that
    /// grant is their last (a grant a manager signed after it takes them out).
    pub linked: HashMap<[u8; 16], SigningPublic>,
    /// For a block's log: who owns the block (holds what authorship gives), its creator until
    /// an owner record moves it on. None for an organization's log, and on a proof path after
    /// an owner record served reduced, which hides whom it named.
    pub owner: Option<[u8; 16]>,
    /// For a block's log: whether its last opening (`Record::Opening`) opened it to whoever
    /// views a block inside.
    pub open_inside: Level,
    /// For a block's log: where its parent records placed it, by place, oldest first; the last
    /// is where it lies now (`placement`). None for one served reduced, whose placement the
    /// reader does not know.
    pub placements: Vec<(usize, Option<Placement>)>,
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
    /// signed, by a partial log (guides/blocks/keys.md, "Who is served what").
    pub partial: bool,
    /// The place of the last opening, and of the last reduced entry that lowered anyone: one
    /// after the opening may have closed it.
    opened_at: usize,
    hidden_lowering: usize,
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
            // The owner always holds Manage, and keeps less.
            Record::Owner(_) => true,
            // Revoking a link lowers everyone it granted.
            Record::ShareLink(link) => {
                !link.open && self.linked.values().any(|key| *key == link.key)
            }
            Record::Root(_)
            | Record::Retirement(_)
            | Record::Parent(_)
            | Record::Invitation(_)
            | Record::Opened(_) => false,
        }
    }

    /// Where the block lies as its log last placed it: none for a log that places it nowhere
    /// (an organization's), or whose last parent record was served reduced.
    pub fn placement(&self) -> Option<Placement> {
        self.placements.last().and_then(|(_, placement)| *placement)
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

    /// Ends every invitation `inviter` opened that is open still: they no longer administer the
    /// organization, and an invitation lasts only while its inviter does, whatever they become
    /// later.
    fn end_invitations_of(&mut self, inviter: [u8; 16]) {
        for opened in self.invitations.values_mut() {
            if *opened == Some(inviter) {
                *opened = None;
            }
        }
    }

    /// `former` no longer owns the block: they keep what a former owner keeps
    /// (`FORMER_OWNER`), with the keys last named for them.
    fn demote_owner(&mut self, former: [u8; 16]) {
        if let Some((level, _)) = self.grants.get_mut(&former) {
            *level = FORMER_OWNER;
        }
        self.set_level(former, FORMER_OWNER);
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
        authority: &Authority,
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
            other => {
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
/// made by a signer with Edit or above, and places the block anywhere but under itself. An owner
/// record is made only by the block's owner (its creator, or whom the last owner record named),
/// who keeps Edit after it (`FORMER_OWNER`); no grant names the owner.
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
    Checked::unchecked(pinned).extend(kind, log, entries, outside)
}

impl Checked {
    /// A log nothing of which is checked yet, whose root must hash to `pinned`: `extend` with
    /// its entries from the root checks it as `check_log` does.
    pub fn unchecked(pinned: EntryHash) -> Self {
        Self {
            head: pinned,
            root: pinned,
            ..Self::default()
        }
    }

    /// This log checked on through `entries`, the ones after its head (from the root when
    /// nothing is checked yet), as `check_log` checks each: only they are checked, so a device
    /// that kept what it checked verifies each entry's signature once. A partial log (a proof
    /// path) is only ever checked whole, from the root.
    pub fn extend(
        self,
        kind: LogKind,
        log: [u8; 16],
        entries: &[impl Logged],
        outside: &dyn Fn(&Asked) -> Option<SigningPublic>,
    ) -> Result<Checked, KeyError> {
        let mut state = self;
        let mut rest = entries;
        if state.positions.is_empty() {
            let (root, after) = entries.split_first().ok_or(KeyError::Record)?;
            let StepRef::Entry(root) = root.step() else {
                return Err(KeyError::Record);
            };
            let checked = root.checked_root(kind, log)?;
            if root.lowers || root.hash() != state.root {
                return Err(KeyError::Record);
            }
            let pinned = state.root;
            state.head = pinned;
            state.signers = HashMap::from([(checked.creator, checked.identity.signing)]);
            state.positions = HashMap::from([(pinned, 0)]);
            state.set_level(checked.creator, Level::Manage);
            match kind {
                LogKind::Block => {
                    state
                        .grants
                        .insert(checked.creator, (Level::Manage, checked.identity));
                    state.owner = Some(checked.creator);
                }
                LogKind::Organization => {
                    state
                        .administrators
                        .insert(checked.creator, checked.identity);
                }
            }
            rest = after;
        } else if state.partial {
            return Err(KeyError::Record);
        }
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
                    if reduced.places {
                        state.placements.push((place, None));
                    }
                    // A transfer is signed by the owner, whom it lowers: hidden, it still leaves
                    // them no more than a former owner keeps, and the new owner unknown.
                    if reduced.owns {
                        if kind != LogKind::Block || !reduced.lowers {
                            return Err(KeyError::Record);
                        }
                        if let Some(former) = state.owner.take() {
                            state.demote_owner(former);
                        }
                    }
                    if reduced.lowers {
                        state.last_lowering = place;
                        state.hides_lowering = true;
                        state.hidden_lowering = place;
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
                    let own_revocation =
                        grant.grantee == entry.signer && grant.level == Level::None;
                    // Authorship is Manage from the root and owner records alone: no grant lowers or
                    // names the owner.
                    if kind != LogKind::Block || state.owner == Some(grant.grantee) {
                        return Err(KeyError::Record);
                    }
                    state.granting(
                        entry,
                        own,
                        &grant.authority,
                        Level::Manage,
                        own_revocation,
                        outside,
                    )?;
                    state.signers.insert(grant.grantee, grant.identity.signing);
                    state
                        .grants
                        .insert(grant.grantee, (grant.level, grant.identity));
                    state.set_level(grant.grantee, grant.level);
                    state.linked.remove(&grant.grantee);
                }
                Record::Opening(opening) => {
                    // Open to view, or closed: no other level, so the record has one meaning.
                    if kind != LogKind::Block || !matches!(opening.level, Level::None | Level::View)
                    {
                        return Err(KeyError::Record);
                    }
                    state.granting(
                        entry,
                        own,
                        &opening.authority,
                        Level::Manage,
                        false,
                        outside,
                    )?;
                    if opening.level < state.open_inside {
                        state.last_lowering = state.positions.len() - 1;
                    }
                    state.open_inside = opening.level;
                    state.opened_at = place;
                }
                Record::Parent(parent) => {
                    let under_itself = matches!(
                        parent.placement,
                        Placement::Block { block, .. } if block == log
                    );
                    if kind != LogKind::Block || under_itself {
                        return Err(KeyError::Record);
                    }
                    state.granting(entry, own, &parent.authority, Level::Edit, false, outside)?;
                    state.placements.push((place, Some(parent.placement)));
                }
                Record::Owner(owner) => {
                    // Only the owner moves ownership on, signing with their current key, to
                    // someone else.
                    let may = kind == LogKind::Block
                        && state.owner == Some(entry.signer)
                        && owner.account != entry.signer
                        && own.is_some_and(|signing| entry.signed_by(&signing));
                    if !may {
                        return Err(KeyError::Record);
                    }
                    state.demote_owner(entry.signer);
                    state.signers.insert(owner.account, owner.identity.signing);
                    state
                        .grants
                        .insert(owner.account, (Level::Manage, owner.identity));
                    state.set_level(owner.account, Level::Manage);
                    state.owner = Some(owner.account);
                    state.linked.remove(&owner.account);
                }
                Record::ShareLink(link) => {
                    // Made once, by a key never named before, granting View or Edit; revoked only
                    // while open.
                    let held = state.share_links.get(&link.key).copied();
                    let well_formed = match link.open {
                        true => held.is_none() && matches!(link.level, Level::View | Level::Edit),
                        false => held.is_some_and(|open| open.is_some()),
                    };
                    if kind != LogKind::Block || !well_formed {
                        return Err(KeyError::Record);
                    }
                    state.granting(entry, own, &link.authority, Level::Manage, false, outside)?;
                    state
                        .share_links
                        .insert(link.key, link.open.then_some(link.level));
                    if !link.open {
                        let fallen: Vec<[u8; 16]> = state
                            .linked
                            .iter()
                            .filter(|(_, key)| **key == link.key)
                            .map(|(account, _)| *account)
                            .collect();
                        for account in fallen {
                            state.linked.remove(&account);
                            if let Some((level, _)) = state.grants.get_mut(&account) {
                                *level = Level::None;
                            }
                            state.set_level(account, Level::None);
                        }
                    }
                    // On a proof path the grants a link made may be reduced: whether revoking it
                    // lowered anyone is the record's signed word, which only outdates epochs.
                    if !link.open && state.partial && entry.lowers {
                        state.last_lowering = place;
                    }
                }
                Record::Opened(opened) => {
                    // Granted by the link's key to its signer alone, at most what the link grants,
                    // raising them, and endorsed by the keys it names.
                    let most = state.share_links.get(&opened.key).copied().flatten();
                    let before = state
                        .grants
                        .get(&opened.grantee)
                        .map_or(Level::None, |(level, _)| *level);
                    // An account the log names already keeps the key it signs with here: the record
                    // may name no other, which only its own retirement changes.
                    let known = own.is_none_or(|signing| signing == opened.identity.signing);
                    let may = kind == LogKind::Block
                        && known
                        && entry.signer == opened.grantee
                        && state.owner != Some(opened.grantee)
                        && opened.level >= Level::View
                        && most.is_some_and(|most| opened.level <= most)
                        && opened.level > before
                        && entry.signed_by(&opened.key)
                        && verify_signature(
                            &opened.identity.signing.0,
                            &self::opened(
                                log,
                                opened.grantee,
                                &opened.identity,
                                &opened.key,
                                opened.level,
                            ),
                            &opened.endorsement,
                        );
                    if !may {
                        return Err(KeyError::Record);
                    }
                    state
                        .signers
                        .insert(opened.grantee, opened.identity.signing);
                    state
                        .grants
                        .insert(opened.grantee, (opened.level, opened.identity));
                    state.set_level(opened.grantee, opened.level);
                    state.linked.insert(opened.grantee, opened.key);
                }
                Record::Administrator(administrator) => {
                    if !administrator.administrator {
                        state.end_invitations_of(administrator.account);
                    }
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
                Record::Invitation(invitation) => {
                    let administrator = state.administrators.contains_key(&entry.signer)
                        && own.is_some_and(|signing| entry.signed_by(&signing));
                    // Opened once, by a key never named before; ended only while open.
                    let held = state.invitations.get(&invitation.key).copied();
                    let may = match invitation.open {
                        true => held.is_none(),
                        false => held.is_some_and(|open| open.is_some()),
                    };
                    if kind != LogKind::Organization || !administrator || !may {
                        return Err(KeyError::Record);
                    }
                    state
                        .invitations
                        .insert(invitation.key, invitation.open.then_some(entry.signer));
                }
                Record::Member(Member {
                    account,
                    identity: Some(identity),
                    member: true,
                    agent: false,
                    invitation: Some(Invited { key, endorsement }),
                }) => {
                    // A person signs themselves in with the key of an invitation still open, whose
                    // inviter administers the organization still, once: no one already named; and
                    // their own keys endorse it.
                    let inviter = state.invitations.get(key).copied().flatten();
                    let may = kind == LogKind::Organization
                        && entry.signer == *account
                        && inviter
                            .is_some_and(|inviter| state.administrators.contains_key(&inviter))
                        && !state.members.contains_key(account)
                        && !state.administrators.contains_key(account)
                        && entry.signed_by(key)
                        && verify_signature(
                            &identity.signing.0,
                            &invited(log, *account, identity, key),
                            endorsement,
                        );
                    if !may {
                        return Err(KeyError::Record);
                    }
                    state.invitations.insert(*key, None);
                    state.signers.insert(*account, identity.signing);
                    state.members.insert(
                        *account,
                        Named {
                            identity: *identity,
                            maker: None,
                        },
                    );
                }
                Record::Member(member) => {
                    let signed = own.is_some_and(|signing| entry.signed_by(&signing));
                    let administrator = state.administrators.contains_key(&entry.signer);
                    // A person is signed in only by an invitation (above); an agent by a member who is
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
                    // A sign-in names keys, and a sign-out none.
                    let may = match (member.member, member.agent, &member.identity) {
                        _ if member.invitation.is_some() => false,
                        (true, true, Some(identity)) => {
                            (administrator || person)
                                && !named
                                && member.account == agent_account(entry.signer, identity)
                        }
                        (false, _, None) => administrator || maker == Some(entry.signer),
                        _ => false,
                    };
                    if kind != LogKind::Organization || !signed || !may {
                        return Err(KeyError::Record);
                    }
                    match member.identity {
                        Some(identity) => {
                            state.signers.insert(member.account, identity.signing);
                            state.members.insert(
                                member.account,
                                Named {
                                    identity,
                                    maker: member.agent.then_some(entry.signer),
                                },
                            );
                        }
                        // Signed out, an administrator administers no more, and their invitations end.
                        None => {
                            state.members.remove(&member.account);
                            if state.administrators.remove(&member.account).is_some() {
                                state.set_level(member.account, Level::None);
                                state.end_invitations_of(member.account);
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
        if state.hidden_lowering > state.opened_at {
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

    /// This log, kept as checked to its head, checked on through `served`, the whole log as the
    /// server serves it again from its root. The entries up to this head must be the ones
    /// checked: each names the hash of the one before, and the last of them hashes to this head
    /// (no signature is verified for them). Else the server rewrote what was checked, and the
    /// log is refused as forged. Only the entries after the head are checked. A partial log is
    /// checked again from its root. A whole kept log checked on through a proof path's tail
    /// knows its prefix whole, as checked before, and is partial from there: it holds more than
    /// the path checked from its root would.
    pub fn resume(
        self,
        kind: LogKind,
        log: [u8; 16],
        served: &[impl Logged],
        outside: &dyn Fn(&Asked) -> Option<SigningPublic>,
    ) -> Result<Checked, KeyError> {
        if self.partial || self.positions.is_empty() {
            return check_log(kind, log, self.root, served, outside);
        }
        let held = self.positions.len();
        let prefix = served.get(..held).ok_or(KeyError::Record)?;
        let mut previous = None;
        for step in prefix {
            let step = step.step();
            if step.previous() != previous {
                return Err(KeyError::Record);
            }
            previous = Some(step.hash());
        }
        if previous != Some(self.head) {
            return Err(KeyError::Record);
        }
        self.extend(kind, log, &served[held..], outside)
    }
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

/// How long an `Authority::Inherited` chain may be: more than any honest tree needs.
pub const MAX_ANCESTRY: usize = 32;

/// The logs `answer` reads another log's grants from. Which block is above which comes from the
/// parent records in them alone, and a block's log is checked from the root the record placing
/// something under it names. Each log is checked at most once as of each head (`checked`), so
/// authority resting on authority costs linear in the records, not exponential in the depth.
pub struct Logs<'a> {
    pub organization: [u8; 16],
    /// The root the reader pinned for the organization's log: none for one with no log.
    pub organization_root: Option<EntryHash>,
    /// A log's entries, root first, as far as the reader holds it.
    pub log: &'a dyn Fn(LogKind, [u8; 16]) -> Option<Vec<Step>>,
    checked: RefCell<HashMap<Checking, Option<Rc<Checked>>>>,
    checks: Cell<usize>,
}

/// A log checked from a root as of one of its entries, as `Logs` remembers it.
type Checking = (LogKind, [u8; 16], EntryHash, EntryHash);

impl<'a> Logs<'a> {
    pub fn new(
        organization: [u8; 16],
        organization_root: Option<EntryHash>,
        log: &'a dyn Fn(LogKind, [u8; 16]) -> Option<Vec<Step>>,
    ) -> Self {
        Self {
            organization,
            organization_root,
            log,
            checked: RefCell::new(HashMap::new()),
            checks: Cell::new(0),
        }
    }

    /// How many logs `answer` checked through these (once each as of each head).
    pub fn checks(&self) -> usize {
        self.checks.get()
    }

    /// The `kind` log of `log`, checked from `root` as of `head`, with what its records name
    /// outside it answered from these logs. Remembered, and refused while it is being checked,
    /// so a chain that comes back round to it proves nothing.
    fn checked_as_of(
        &self,
        kind: LogKind,
        log: [u8; 16],
        root: EntryHash,
        head: EntryHash,
    ) -> Option<Rc<Checked>> {
        let key = (kind, log, root, head);
        if let Some(found) = self.checked.borrow().get(&key) {
            return found.clone();
        }
        self.checked.borrow_mut().insert(key, None);
        self.checks.set(self.checks.get() + 1);
        let entries = (self.log)(kind, log);
        let outside = |asked: &Asked| answer(self, log, asked);
        let found = entries
            .and_then(|entries| check_log_as_of(kind, log, root, &entries, head, &outside).ok())
            .map(Rc::new);
        self.checked.borrow_mut().insert(key, found.clone());
        found
    }
}

/// `check_log`'s `outside` for the log of `block`, from `logs`: the signer's key in the log a
/// record names its authority in, checked as of the entry it names, if they held what it needs
/// there then. For `Authority::Inherited`, each link of the chain must be where the one before
/// lay as of its head, starting from where `block` lay at the record's place, each log checked
/// from the root the placement under it names.
pub fn answer(logs: &Logs, block: [u8; 16], asked: &Asked) -> Option<SigningPublic> {
    let (kind, checked) = match asked.authority {
        Authority::Log => return None,
        Authority::Organization(head) => (
            LogKind::Organization,
            logs.checked_as_of(
                LogKind::Organization,
                logs.organization,
                logs.organization_root?,
                *head,
            )?,
        ),
        Authority::Inherited(chain) => {
            if chain.is_empty() || chain.len() > MAX_ANCESTRY {
                return None;
            }
            let mut placement = *asked.placement?;
            let mut reached = None;
            for link in chain {
                let Placement::Block { block: up, root } = placement else {
                    return None;
                };
                if up != link.block || up == block {
                    return None;
                }
                let checked = logs.checked_as_of(LogKind::Block, up, root, link.head)?;
                placement = checked.placement().unwrap_or(Placement::Detached);
                reached = Some(checked);
            }
            (LogKind::Block, reached?)
        }
    };
    checked.holding(kind, asked.signer, asked.level)
}
