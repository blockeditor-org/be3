use std::fmt;

mod access;
pub mod aead;
pub mod lru;
pub mod merkle;
pub mod noise;
pub use access::{
    Administrator, Admits, Asked, Authority, Ban, CarriedKey, Checked, Entry, EntryHash,
    FORMER_OWNER, Grant, Heads, Invitation, Invited, KeyRole, Label, Level, Link, LogKind, Logged,
    Logs, MAX_ANCESTRY, Member, Named, Opened, Opening, Owner, Parent, Placement, Record, Reduced,
    Retirement, Root, ShareLink, Step, StepRef, agent_account, answer, check_log, check_log_as_of,
    proof_path,
};

use hkdf::Hkdf;
use rand::TryRngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, Zeroizing};

const PHRASE_ENTROPY: usize = 16;
const RECOVERY_INFO: &[u8] = b"be3.recovery.v1";
const PAIRING_CONTEXT: &[u8] = b"be3.pairing.v1";
const INVITE_INFO: &[u8] = b"be3.invite.v1";
const INVITE_LOOKUP_INFO: &[u8] = b"be3.invite.lookup.v1";
const INVITE_SIGNING_INFO: &[u8] = b"be3.invite.signing.v1";
const SHARE_SIGNING_INFO: &[u8] = b"be3.share.signing.v1";
const SHARE_SEALED_INFO: &[u8] = b"be3.share.sealed.v1";
const OWN_INFO: &[u8] = b"be3.own.v1";
const OWN_CONTACT: &[u8] = b"be3.own.contact.v1";
const OWN_PIN: &[u8] = b"be3.own.pin.v1";
const CARD_OWNER: &[u8] = b"be3.contact.owner.v1";
const CARD_ACCEPTER: &[u8] = b"be3.contact.accepter.v1";
const FROM_INFO: &[u8] = b"be3.from.v1";
const ESCROW_PIN_INFO: &[u8] = b"be3.escrow.pin.v1";
const KEY_CHANGE_INFO: &[u8] = b"be3.key.change.v1";
const ESCROW_CHAIN_INFO: &[u8] = b"be3.escrow.chain.v4";
const IDENTITY_SIGNING_INFO: &[u8] = b"be3.identity.signing.v1";
const AUTHORSHIP_INFO: &[u8] = b"be3.authorship.v1";
const INVITE_ENTROPY: usize = 25;
const INVITE_LEN: usize = INVITE_ENTROPY * 8 / 5;
const CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
pub const CODE_LEN: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyError {
    Phrase,
    Corrupt,
    Pairing,
    Invite,
    Card,
    Sender,
    Record,
    /// A public key whose Diffie-Hellman result is fixed whatever the secret (a low-order
    /// point): nothing is sealed to or opened from it.
    Public,
}

impl fmt::Display for KeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Phrase => formatter.write_str("that is not a recovery phrase"),
            Self::Corrupt => formatter.write_str("the key did not open with this secret"),
            Self::Pairing => formatter.write_str("the code did not match"),
            Self::Invite => formatter.write_str("that is not an invite code"),
            Self::Card => formatter.write_str("the contact card did not open with this link"),
            Self::Sender => formatter.write_str("the key was not sealed by the account it names"),
            Self::Record => formatter.write_str("the access record is not signed by whom it names"),
            Self::Public => formatter.write_str("that is not a public key anything is sealed to"),
        }
    }
}

impl std::error::Error for KeyError {}

/// `N` bytes from the operating system's entropy.
pub fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .expect("the operating system has entropy");
    bytes
}

/// `plain` sealed under `key` (`aead`), binding `context`: a random nonce, then the ciphertext.
fn seal_with(key: &[u8; 32], context: &[u8], plain: &[u8]) -> Vec<u8> {
    let nonce = aead::random_nonce();
    [nonce.as_slice(), &aead::seal(key, &nonce, context, plain)].concat()
}

fn open_with(
    key: &[u8; 32],
    context: &[u8],
    sealed: &[u8],
    error: KeyError,
) -> Result<Vec<u8>, KeyError> {
    let (nonce, ciphertext) = sealed.split_at_checked(aead::NONCE_LEN).ok_or(error)?;
    let nonce: aead::Nonce = nonce.try_into().map_err(|_| error)?;
    aead::open(key, &nonce, context, ciphertext).map_err(|_| error)
}

fn expand(secret: &[u8], salt: &[u8], info: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    Hkdf::<Sha256>::new(Some(salt), secret)
        .expand(info, &mut out)
        .expect("32 bytes is a valid hkdf length");
    out
}

/// The X25519 result of `secret` and `public`, refused when it is not contributory: a
/// low-order `public` (a card, a key change or an escrow key someone published) makes it the
/// same whatever the secret, so a seal to or from it would open for anyone.
fn agree(secret: &StaticSecret, public: [u8; 32]) -> Result<Zeroizing<[u8; 32]>, KeyError> {
    let shared = secret.diffie_hellman(&PublicKey::from(public));
    match shared.was_contributory() {
        true => Ok(Zeroizing::new(shared.to_bytes())),
        false => Err(KeyError::Public),
    }
}

/// What an account seals for itself (`RecoverySecret::seal_own`), each kind under a context of
/// its own, so one never opens as another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Own {
    /// Its record of a contact, which a letter carries to its own mailbox.
    Contact,
    /// What it pinned for an organization (`contacts::Pinned`).
    Pin([u8; 16]),
}

impl Own {
    fn context(self) -> Vec<u8> {
        match self {
            Self::Contact => OWN_CONTACT.to_vec(),
            Self::Pin(organization) => [OWN_PIN, &organization].concat(),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct RecoveryPhrase {
    entropy: [u8; PHRASE_ENTROPY],
}

impl Drop for RecoveryPhrase {
    fn drop(&mut self) {
        self.entropy.zeroize();
    }
}

impl fmt::Debug for RecoverySecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecoverySecret(..)")
    }
}

impl RecoveryPhrase {
    pub fn generate() -> Self {
        Self { entropy: random() }
    }

    pub fn parse(input: &str) -> Result<Self, KeyError> {
        let words: Vec<String> = input
            .split(|character: char| character.is_whitespace() || character == ',')
            .filter(|word| !word.is_empty())
            .map(str::to_ascii_lowercase)
            .collect();
        let mnemonic =
            bip39::Mnemonic::parse_normalized(&words.join(" ")).map_err(|_| KeyError::Phrase)?;
        let entropy = mnemonic.to_entropy();
        Ok(Self {
            entropy: entropy.try_into().map_err(|_| KeyError::Phrase)?,
        })
    }

    pub fn words(&self) -> Vec<&'static str> {
        bip39::Mnemonic::from_entropy(&self.entropy)
            .expect("sixteen bytes is a valid mnemonic length")
            .words()
            .collect()
    }

    pub fn checked_positions(&self) -> [usize; 3] {
        let mut positions = [0usize; 3];
        let mut chosen = 0;
        while chosen < positions.len() {
            let candidate = usize::from(random::<1>()[0] % 12);
            if !positions[..chosen].contains(&candidate) {
                positions[chosen] = candidate;
                chosen += 1;
            }
        }
        positions.sort_unstable();
        positions
    }

    pub fn secret(&self) -> RecoverySecret {
        RecoverySecret(StaticSecret::from(expand(
            &self.entropy,
            RECOVERY_INFO,
            RECOVERY_INFO,
        )))
    }
}

impl fmt::Display for RecoveryPhrase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.words().join(" "))
    }
}

impl fmt::Debug for RecoveryPhrase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RecoveryPhrase(..)")
    }
}

#[derive(Clone)]
pub struct RecoverySecret(StaticSecret);

impl RecoverySecret {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(StaticSecret::from(bytes))
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    pub fn public(&self) -> RecoveryPublic {
        RecoveryPublic(PublicKey::from(&self.0).to_bytes())
    }

    /// Seals `plain` for this account alone, as `kind`: what it sends itself through the
    /// server. Sealing to its own public key instead would let the server make one up.
    pub fn seal_own(&self, kind: Own, plain: &[u8]) -> Vec<u8> {
        seal_with(&self.own_key(), &kind.context(), plain)
    }

    pub fn open_own(&self, kind: Own, sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        open_with(&self.own_key(), &kind.context(), sealed, KeyError::Corrupt)
    }

    fn own_key(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(expand(
            Zeroizing::new(self.0.to_bytes()).as_slice(),
            OWN_INFO,
            OWN_INFO,
        ))
    }

    pub(crate) fn signing_key(&self) -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&expand(
            &self.0.to_bytes(),
            IDENTITY_SIGNING_INFO,
            IDENTITY_SIGNING_INFO,
        ))
    }

    /// The public half of the Ed25519 key the account signs what it writes with, derived from
    /// the identity secret as `EscrowSecret` derives its own
    /// (decisions/an_ed25519_signing_key_beside_the_identity_key.md).
    pub fn signing(&self) -> SigningPublic {
        SigningPublic(self.signing_key().verifying_key().to_bytes())
    }

    /// Both public halves, which a contact card and a key change carry.
    pub fn identity(&self) -> PublicIdentity {
        PublicIdentity {
            public: self.public(),
            signing: self.signing(),
        }
    }

    /// Signs `bytes` the account wrote in `block`. The signature covers the block too, so one
    /// made for a block never verifies in another, and a domain of its own, so it can never be
    /// read as anything else this key might sign.
    pub fn sign_authored(&self, block: [u8; 16], bytes: &[u8]) -> Vec<u8> {
        use ed25519_dalek::Signer;
        self.signing_key()
            .sign(&authored(block, bytes))
            .to_bytes()
            .to_vec()
    }

    /// Signs an escrow key its organization's administrator makes (`EscrowKey::chained`), in a
    /// domain of its own.
    pub fn sign_escrow(&self, chained: &[u8]) -> Vec<u8> {
        use ed25519_dalek::Signer;
        self.signing_key()
            .sign(&[ESCROW_CHAIN_INFO, chained].concat())
            .to_bytes()
            .to_vec()
    }

    /// Seals `plain` to `to`, as the account `from` holding this secret: only `to`'s secret
    /// opens it, and only as from this key (`open_from`). Refused for a low-order `to`.
    pub fn seal_for(
        &self,
        from: [u8; 16],
        to: &Party,
        context: &[u8],
        plain: &[u8],
    ) -> Result<Vec<u8>, KeyError> {
        let ephemeral = StaticSecret::from(random::<32>());
        let ephemeral_public = PublicKey::from(&ephemeral).to_bytes();
        let key = from_key(
            &*agree(&ephemeral, to.public.0)?,
            &*agree(&self.0, to.public.0)?,
            ephemeral_public,
            &Party {
                account: from,
                public: self.public(),
            },
            to,
            context,
        );
        let mut sealed = ephemeral_public.to_vec();
        sealed.extend(seal_with(
            &key,
            &[FROM_INFO, &ephemeral_public].concat(),
            plain,
        ));
        Ok(sealed)
    }

    pub fn open_from(
        &self,
        from: &Party,
        to: [u8; 16],
        context: &[u8],
        sealed: &[u8],
    ) -> Result<Vec<u8>, KeyError> {
        let (ephemeral, rest) = sealed.split_at_checked(32).ok_or(KeyError::Sender)?;
        let ephemeral: [u8; 32] = ephemeral.try_into().map_err(|_| KeyError::Sender)?;
        let me = Party {
            account: to,
            public: self.public(),
        };
        let key = from_key(
            &*agree(&self.0, ephemeral).map_err(|_| KeyError::Sender)?,
            &*agree(&self.0, from.public.0)?,
            ephemeral,
            from,
            &me,
            context,
        );
        open_with(
            &key,
            &[FROM_INFO, &ephemeral].concat(),
            rest,
            KeyError::Sender,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Party {
    pub account: [u8; 16],
    pub public: RecoveryPublic,
}

fn from_key(
    ephemeral_shared: &[u8; 32],
    static_shared: &[u8; 32],
    ephemeral: [u8; 32],
    from: &Party,
    to: &Party,
    context: &[u8],
) -> Zeroizing<[u8; 32]> {
    let mut out = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(
        Some(&[ephemeral, from.public.0, to.public.0].concat()),
        &Zeroizing::new([ephemeral_shared.as_slice(), static_shared.as_slice()].concat()),
    )
    .expand_multi_info(&[FROM_INFO, &from.account, &to.account, context], &mut *out)
    .expect("32 bytes is a valid hkdf length");
    out
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct RecoveryPublic(pub [u8; 32]);

/// The public half of an account's signing key.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct SigningPublic(pub [u8; 32]);

impl SigningPublic {
    /// Whether `signature` is this key's `RecoverySecret::sign_authored` over `bytes` in
    /// `block`.
    pub fn verifies_authored(&self, block: [u8; 16], bytes: &[u8], signature: &[u8]) -> bool {
        verify_signature(&self.0, &authored(block, bytes), signature)
    }
}

impl SigningPublic {
    /// Whether `signature` is this key's `RecoverySecret::sign_escrow` over `chained`.
    pub fn verifies_escrow(&self, chained: &[u8], signature: &[u8]) -> bool {
        verify_signature(&self.0, &[ESCROW_CHAIN_INFO, chained].concat(), signature)
    }
}

fn authored(block: [u8; 16], bytes: &[u8]) -> Vec<u8> {
    [AUTHORSHIP_INFO, &block, bytes].concat()
}

/// An account's public keys: the identity key keys are sealed to, and the signing key.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PublicIdentity {
    pub public: RecoveryPublic,
    pub signing: SigningPublic,
}

pub fn pairing_code() -> String {
    random::<CODE_LEN>()
        .iter()
        .map(|byte| CODE_ALPHABET[usize::from(byte % 32)] as char)
        .collect()
}

pub fn normalize_code(input: &str) -> Option<String> {
    normalize(input, CODE_LEN)
}

fn normalize(input: &str, len: usize) -> Option<String> {
    let code: String = input
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| match character.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect();
    (code.len() == len && code.bytes().all(|byte| CODE_ALPHABET.contains(&byte))).then_some(code)
}

#[derive(Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct InviteCode([u8; INVITE_ENTROPY]);

impl InviteCode {
    pub fn generate() -> Self {
        Self(random())
    }

    pub fn parse(input: &str) -> Result<Self, KeyError> {
        let code = normalize(input, INVITE_LEN).ok_or(KeyError::Invite)?;
        let mut entropy = [0u8; INVITE_ENTROPY];
        for (index, byte) in code.bytes().enumerate() {
            let value = CODE_ALPHABET
                .iter()
                .position(|letter| *letter == byte)
                .ok_or(KeyError::Invite)?;
            for bit in 0..5 {
                if value & (0b10000 >> bit) != 0 {
                    let at = index * 5 + bit;
                    entropy[at / 8] |= 0x80 >> (at % 8);
                }
            }
        }
        Ok(Self(entropy))
    }

    pub fn lookup(&self) -> [u8; 16] {
        let mut lookup = [0u8; 16];
        lookup.copy_from_slice(&expand(&self.0, INVITE_LOOKUP_INFO, INVITE_LOOKUP_INFO)[..16]);
        lookup
    }

    pub fn to_bytes(&self) -> [u8; INVITE_ENTROPY] {
        self.0
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, KeyError> {
        Ok(Self(bytes.try_into().map_err(|_| KeyError::Invite)?))
    }

    fn seal(&self, context: &[u8], plain: &[u8]) -> Vec<u8> {
        seal_with(&self.key(context), context, plain)
    }

    fn open(&self, context: &[u8], sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        open_with(&self.key(context), context, sealed, KeyError::Invite)
    }

    fn key(&self, context: &[u8]) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(expand(&self.0, context, INVITE_INFO))
    }

    /// The key an invitation into `organization` made with this code signs its accepter in
    /// with (guides/blocks/keys.md, "Access logs"): its public half is in the organization's
    /// log, and the server, which never holds the code, cannot sign with it.
    pub(crate) fn invitation_signing(&self, organization: [u8; 16]) -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&expand(
            &self.0,
            &[INVITE_SIGNING_INFO, &organization].concat(),
            INVITE_SIGNING_INFO,
        ))
    }

    /// The key a share link on `block` made with this code signs its openers' grants with
    /// (guides/blocks/keys.md, "Share links"): its public half is in the block's log, and the
    /// server, which never holds the code, cannot sign with it.
    pub(crate) fn share_signing(&self, block: [u8; 16]) -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&expand(
            &self.0,
            &[SHARE_SIGNING_INFO, &block].concat(),
            SHARE_SIGNING_INFO,
        ))
    }

    /// The public half of `share_signing`, which the share link record names.
    pub fn share_key(&self, block: [u8; 16]) -> SigningPublic {
        SigningPublic(self.share_signing(block).verifying_key().to_bytes())
    }

    /// What a share link on `block` hands its opener, sealed with the code: the server stores
    /// it and never opens it.
    pub fn seal_share(&self, block: [u8; 16], plain: &[u8]) -> Vec<u8> {
        self.seal(&[SHARE_SEALED_INFO, &block].concat(), plain)
    }

    pub fn open_share(&self, block: [u8; 16], sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        self.open(&[SHARE_SEALED_INFO, &block].concat(), sealed)
    }

    /// The public half of `invitation_signing`, which the invitation record names.
    pub fn invitation_key(&self, organization: [u8; 16]) -> SigningPublic {
        SigningPublic(
            self.invitation_signing(organization)
                .verifying_key()
                .to_bytes(),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CardRole {
    Owner,
    Accepter,
}

impl CardRole {
    fn context(self) -> &'static [u8] {
        match self {
            Self::Owner => CARD_OWNER,
            Self::Accepter => CARD_ACCEPTER,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContactCard {
    pub account: [u8; 16],
    pub public: RecoveryPublic,
    pub signing: SigningPublic,
    pub display_name: String,
    pub email: String,
}

impl ContactCard {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("a contact card always encodes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, KeyError> {
        postcard::from_bytes(bytes).map_err(|_| KeyError::Card)
    }

    pub fn seal(&self, code: &InviteCode, role: CardRole) -> Vec<u8> {
        code.seal(role.context(), &self.encode())
    }

    pub fn open(code: &InviteCode, role: CardRole, sealed: &[u8]) -> Result<Self, KeyError> {
        let plain = code
            .open(role.context(), sealed)
            .map_err(|_| KeyError::Card)?;
        Self::decode(&plain)
    }
}

/// The name and email an account goes by, signed by its signing key (`sign`), which its
/// devices publish whenever its name changes: a contact shows the newest one the key it pinned
/// signs (`open`), so a rename reaches the account's contacts without a new link, and a server,
/// which renames accounts in its portal but holds no signing key, cannot rename one itself.
/// `dated` only grows, so the server keeps the newest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NameCard {
    pub account: [u8; 16],
    pub display_name: String,
    pub email: String,
    pub dated: u64,
}

const NAME_CARD_DOMAIN: &[u8] = b"be3.name-card.v1\0";

impl NameCard {
    fn message(&self) -> Vec<u8> {
        let card = postcard::to_stdvec(self).expect("a name card always encodes");
        [NAME_CARD_DOMAIN, &card].concat()
    }

    /// This card, signed by `secret`'s signing key, as it travels.
    pub fn sign(&self, secret: &RecoverySecret) -> Vec<u8> {
        use ed25519_dalek::Signer;
        let signature = secret
            .signing_key()
            .sign(&self.message())
            .to_bytes()
            .to_vec();
        postcard::to_stdvec(&(self, signature)).expect("a signed name card always encodes")
    }

    /// The card `signed` carries, when `signing` signed it.
    pub fn open(signed: &[u8], signing: &SigningPublic) -> Option<Self> {
        let (card, signature): (Self, Vec<u8>) = postcard::from_bytes(signed).ok()?;
        verify_signature(&signing.0, &card.message(), &signature).then_some(card)
    }
}

impl fmt::Display for InviteCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for index in 0..INVITE_LEN {
            if index > 0 && index % 5 == 0 {
                formatter.write_str("-")?;
            }
            let mut value = 0usize;
            for bit in 0..5 {
                let at = index * 5 + bit;
                value = value << 1 | usize::from(self.0[at / 8] & (0x80 >> (at % 8)) != 0);
            }
            write!(formatter, "{}", CODE_ALPHABET[value] as char)?;
        }
        Ok(())
    }
}

impl Drop for InviteCode {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for InviteCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("InviteCode(..)")
    }
}

#[derive(Clone)]
pub struct EscrowSecret([u8; 32]);

impl EscrowSecret {
    pub fn generate() -> Self {
        Self(random())
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.0
    }

    pub fn public(&self) -> RecoveryPublic {
        RecoveryPublic(PublicKey::from(&StaticSecret::from(self.0)).to_bytes())
    }

    /// Opens what `from` sealed to this escrow key, named `escrow` (its id), with
    /// `RecoverySecret::seal_for`: a wrap only a sender holding `from`'s identity key made.
    pub fn open_from(
        &self,
        from: &Party,
        escrow: [u8; 16],
        context: &[u8],
        sealed: &[u8],
    ) -> Result<Vec<u8>, KeyError> {
        RecoverySecret::from_bytes(self.0).open_from(from, escrow, context, sealed)
    }
}

impl Drop for EscrowSecret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for EscrowSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EscrowSecret(..)")
    }
}

thread_local! {
    /// Signatures verified on this thread (`verify_signature`).
    static VERIFIED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// How many signatures this thread has verified: what a test of how often logs are checked
/// counts, in this crate and in those that check logs (be-server, be-block-client). Always
/// counted, as a thread-local increment beside an Ed25519 verification costs nothing to speak
/// of, and a feature for it would build the crates twice.
pub fn signatures_verified() -> u64 {
    VERIFIED.with(std::cell::Cell::get)
}

pub fn verify_signature(signing: &[u8; 32], message: &[u8], signature: &[u8]) -> bool {
    VERIFIED.with(|verified| verified.set(verified.get() + 1));
    let Ok(key) = ed25519_dalek::VerifyingKey::from_bytes(signing) else {
        return false;
    };
    let Ok(signature) = ed25519_dalek::Signature::from_slice(signature) else {
        return false;
    };
    key.verify_strict(message, &signature).is_ok()
}

impl InviteCode {
    pub fn seal_escrow(&self, organization: [u8; 16], escrow: &[u8]) -> Vec<u8> {
        self.seal(&[ESCROW_PIN_INFO, &organization].concat(), escrow)
    }

    pub fn open_escrow(&self, organization: [u8; 16], sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        self.open(&[ESCROW_PIN_INFO, &organization].concat(), sealed)
    }
}

impl RecoverySecret {
    /// Tells a contact `to` the account's new keys, sealed with the old identity key the
    /// contact verified.
    pub fn seal_key_change(
        &self,
        from: [u8; 16],
        to: &Party,
        new: PublicIdentity,
    ) -> Result<Vec<u8>, KeyError> {
        self.seal_for(
            from,
            to,
            KEY_CHANGE_INFO,
            &[from.as_slice(), &new.public.0, &new.signing.0].concat(),
        )
    }

    pub fn open_key_change(
        &self,
        from: &Party,
        to: [u8; 16],
        sealed: &[u8],
    ) -> Result<PublicIdentity, KeyError> {
        let plain = self.open_from(from, to, KEY_CHANGE_INFO, sealed)?;
        let (account, keys) = plain.split_at_checked(16).ok_or(KeyError::Sender)?;
        if account != from.account || keys.len() != 64 {
            return Err(KeyError::Sender);
        }
        let (public, signing) = keys.split_at(32);
        Ok(PublicIdentity {
            public: RecoveryPublic(public.try_into().map_err(|_| KeyError::Sender)?),
            signing: SigningPublic(signing.try_into().map_err(|_| KeyError::Sender)?),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PairingMessage {
    Request {
        #[serde(with = "serde_bytes")]
        message: Vec<u8>,
        device: String,
    },
    Reply {
        #[serde(with = "serde_bytes")]
        message: Vec<u8>,
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
}

impl PairingMessage {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("a pairing message always encodes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, KeyError> {
        postcard::from_bytes(bytes).map_err(|_| KeyError::Pairing)
    }
}

pub struct Pairing(Spake2<Ed25519Group>);

impl Pairing {
    pub fn start(code: &str, context: &[u8]) -> (Self, Vec<u8>) {
        let (state, message) = Spake2::<Ed25519Group>::start_symmetric(
            &Password::new(code.as_bytes()),
            &Identity::new(&[PAIRING_CONTEXT, context].concat()),
        );
        (Self(state), message)
    }

    pub fn finish(self, theirs: &[u8]) -> Result<PairingKey, KeyError> {
        let key = self.0.finish(theirs).map_err(|_| KeyError::Pairing)?;
        Ok(PairingKey(expand(&key, PAIRING_CONTEXT, PAIRING_CONTEXT)))
    }
}

pub struct PairingKey([u8; 32]);

impl Drop for PairingKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl PairingKey {
    pub fn seal(&self, plain: &[u8]) -> Vec<u8> {
        seal_with(&self.0, PAIRING_CONTEXT, plain)
    }

    pub fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        open_with(&self.0, PAIRING_CONTEXT, sealed, KeyError::Pairing)
    }
}

#[cfg(test)]
mod tests;
