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

use crate::{KeyError, PublicIdentity, RecoverySecret, verify_signature};

/// What an access record's signature covers comes after this, so it never verifies as anything
/// else the account's signing key signs (authorship has a domain of its own).
const ACCESS_INFO: &[u8] = b"be3.access.v1";
/// What an entry's hash covers comes after this.
const ACCESS_HASH_INFO: &[u8] = b"be3.access.hash.v1";

/// The hash of an access log entry, by which the next entry and a device's pin name it.
pub type EntryHash = [u8; 32];

/// A record in an access log.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Record {
    /// The first entry of a log: who made the block or organization, with the keys they made it
    /// with.
    Root(Root),
}

/// What a log is the log of, so a block's records never verify as an organization's, nor the
/// other way round.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
        let Record::Root(root) = &self.record;
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
