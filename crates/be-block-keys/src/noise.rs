//! The private channel between a hosted block's host and a member (plans/hosted_blocks.md,
//! "The channel"): a Noise IK handshake (Noise Protocol Framework, revision 34) between the two
//! accounts' X25519 identity keys, then messages sealed under the keys it splits into.
//!
//! The joiner is the initiator: it knows the host's static key from the record of the block's
//! access log that names the host's account, and sends a fresh ephemeral key and its own static
//! key, which the host checks against the joiner's grant. The prologue binds the block and the
//! head of its log the joiner checked the host at, so both sides speak of one block and one
//! state of its access. The cipher is this crate's one symmetric seal (`aead`,
//! decisions/one_symmetric_seal.md) in place of Noise's ChaChaPoly, with the 64-bit counter
//! nonce in the first 8 bytes of its 24-byte nonce, so the protocol name says `XChaChaPoly`.
//!
//! After the handshake each message carries its counter (`Channel::seal`), and a receiver takes
//! only a counter past the last it took (`Channel::open`): the relay drops messages when it must,
//! but the server can neither read, forge, reorder nor replay one.

use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::{KeyError, RecoveryPublic, RecoverySecret, aead, random};

const PROTOCOL: &[u8] = b"Noise_IK_25519_XChaChaPoly_SHA256";
/// What a hosted block's prologue starts with (`prologue`).
const HOSTED_INFO: &[u8] = b"be3.hosted.v1";
const KEY_LEN: usize = 32;
const TAG_LEN: usize = 16;

/// The prologue both sides of a hosted block's channel mix in first: the block and the head of
/// its access log the joiner checked the host's right at.
pub fn hosted_prologue(block: [u8; 16], head: crate::EntryHash) -> Vec<u8> {
    [HOSTED_INFO, &block, &head].concat()
}

#[derive(Clone)]
struct CipherState {
    key: Option<[u8; KEY_LEN]>,
    nonce: u64,
}

fn nonce_of(counter: u64) -> aead::Nonce {
    let mut nonce = [0u8; aead::NONCE_LEN];
    nonce[..8].copy_from_slice(&counter.to_le_bytes());
    nonce
}

impl CipherState {
    fn empty() -> Self {
        Self {
            key: None,
            nonce: 0,
        }
    }

    fn keyed(key: [u8; KEY_LEN]) -> Self {
        Self {
            key: Some(key),
            nonce: 0,
        }
    }

    fn encrypt(&mut self, associated: &[u8], plain: &[u8]) -> Vec<u8> {
        let Some(key) = self.key else {
            return plain.to_vec();
        };
        let sealed = aead::seal(&key, &nonce_of(self.nonce), associated, plain);
        self.nonce += 1;
        sealed
    }

    fn decrypt(&mut self, associated: &[u8], sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        let Some(key) = self.key else {
            return Ok(sealed.to_vec());
        };
        let plain = aead::open(&key, &nonce_of(self.nonce), associated, sealed)
            .map_err(|_| KeyError::Corrupt)?;
        self.nonce += 1;
        Ok(plain)
    }
}

/// Noise's SymmetricState: the chaining key, the handshake hash, and the cipher keyed so far.
#[derive(Clone)]
struct Symmetric {
    chaining: [u8; 32],
    hash: [u8; 32],
    cipher: CipherState,
}

/// Noise's HKDF with two outputs: HMAC-SHA256 with the chaining key as salt, which is RFC 5869's
/// HKDF with empty info.
fn hkdf2(chaining: &[u8; 32], input: &[u8]) -> ([u8; 32], [u8; 32]) {
    let mut out = [0u8; 64];
    Hkdf::<Sha256>::new(Some(chaining), input)
        .expand(&[], &mut out)
        .expect("64 bytes is a valid length for HKDF-SHA256");
    let (first, second) = out.split_at(32);
    (
        first.try_into().expect("32 bytes"),
        second.try_into().expect("32 bytes"),
    )
}

impl Symmetric {
    fn new(prologue: &[u8]) -> Self {
        let hash: [u8; 32] = match PROTOCOL.len() <= 32 {
            true => {
                let mut hash = [0u8; 32];
                hash[..PROTOCOL.len()].copy_from_slice(PROTOCOL);
                hash
            }
            false => Sha256::digest(PROTOCOL).into(),
        };
        let mut state = Self {
            chaining: hash,
            hash,
            cipher: CipherState::empty(),
        };
        state.mix_hash(prologue);
        state
    }

    fn mix_hash(&mut self, data: &[u8]) {
        self.hash = Sha256::new()
            .chain_update(self.hash)
            .chain_update(data)
            .finalize()
            .into();
    }

    fn mix_key(&mut self, input: &[u8]) {
        let (chaining, key) = hkdf2(&self.chaining, input);
        self.chaining = chaining;
        self.cipher = CipherState::keyed(key);
    }

    fn encrypt_and_hash(&mut self, plain: &[u8]) -> Vec<u8> {
        let sealed = self.cipher.encrypt(&self.hash, plain);
        self.mix_hash(&sealed);
        sealed
    }

    fn decrypt_and_hash(&mut self, sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        let plain = self.cipher.decrypt(&self.hash, sealed)?;
        self.mix_hash(sealed);
        Ok(plain)
    }

    /// The two transport keys: the initiator's sending one first.
    fn split(&self) -> ([u8; 32], [u8; 32]) {
        hkdf2(&self.chaining, &[])
    }
}

fn dh(secret: &StaticSecret, public: &[u8; 32]) -> [u8; 32] {
    secret.diffie_hellman(&PublicKey::from(*public)).to_bytes()
}

fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Result<&'a [u8], KeyError> {
    let (taken, rest) = bytes.split_at_checked(len).ok_or(KeyError::Corrupt)?;
    *bytes = rest;
    Ok(taken)
}

/// A joiner's side of the handshake, its first message sent: it waits for the host's answer
/// (`finish`).
pub struct Initiator {
    symmetric: Symmetric,
    ephemeral: StaticSecret,
    own: RecoverySecret,
}

impl Initiator {
    /// Starts the handshake as `own` to the host whose static key is `host`, under `prologue`
    /// (`hosted_prologue`), with `payload`: the first message, `-> e, es, s, ss`.
    pub fn start(
        own: &RecoverySecret,
        host: &RecoveryPublic,
        prologue: &[u8],
        payload: &[u8],
    ) -> (Self, Vec<u8>) {
        let mut symmetric = Symmetric::new(prologue);
        // The pre-message `<- s`.
        symmetric.mix_hash(&host.0);
        let ephemeral = StaticSecret::from(random::<32>());
        let ephemeral_public = PublicKey::from(&ephemeral).to_bytes();
        let mut message = ephemeral_public.to_vec();
        symmetric.mix_hash(&ephemeral_public);
        symmetric.mix_key(&dh(&ephemeral, &host.0));
        message.extend(symmetric.encrypt_and_hash(&own.public().0));
        symmetric.mix_key(&dh(&own.0, &host.0));
        message.extend(symmetric.encrypt_and_hash(payload));
        (
            Self {
                symmetric,
                ephemeral,
                own: own.clone(),
            },
            message,
        )
    }

    /// Reads the host's answer, `<- e, ee, se`: the channel and the answer's payload. Fails
    /// unless it was made by the holder of the host's static key, for this handshake.
    pub fn finish(mut self, message: &[u8]) -> Result<(Channel, Vec<u8>), KeyError> {
        let mut rest = message;
        let theirs: [u8; 32] = take(&mut rest, 32)?
            .try_into()
            .map_err(|_| KeyError::Corrupt)?;
        self.symmetric.mix_hash(&theirs);
        self.symmetric.mix_key(&dh(&self.ephemeral, &theirs));
        self.symmetric.mix_key(&dh(&self.own.0, &theirs));
        let payload = self.symmetric.decrypt_and_hash(rest)?;
        let (sending, receiving) = self.symmetric.split();
        Ok((
            Channel::new(sending, receiving, self.symmetric.hash),
            payload,
        ))
    }
}

/// A host's side of the handshake, the joiner's first message read: who it is (its static key)
/// and what it said, for the host to check before it answers (`accept`).
pub struct Responder {
    symmetric: Symmetric,
    their_ephemeral: [u8; 32],
    /// The joiner's static key, which the host checks its grant names.
    pub joiner: RecoveryPublic,
    /// What the joiner sent with it.
    pub payload: Vec<u8>,
}

impl Responder {
    /// Reads a joiner's first message as `own`, the host, under `prologue`. Fails unless it was
    /// made for `own`'s static key, by the holder of the static key it carries, under the same
    /// prologue.
    pub fn read(own: &RecoverySecret, prologue: &[u8], message: &[u8]) -> Result<Self, KeyError> {
        let mut symmetric = Symmetric::new(prologue);
        symmetric.mix_hash(&own.public().0);
        let mut rest = message;
        let their_ephemeral: [u8; 32] = take(&mut rest, 32)?
            .try_into()
            .map_err(|_| KeyError::Corrupt)?;
        symmetric.mix_hash(&their_ephemeral);
        symmetric.mix_key(&dh(&own.0, &their_ephemeral));
        let sealed_static = take(&mut rest, 32 + TAG_LEN)?;
        let joiner: [u8; 32] = symmetric
            .decrypt_and_hash(sealed_static)?
            .try_into()
            .map_err(|_| KeyError::Corrupt)?;
        symmetric.mix_key(&dh(&own.0, &joiner));
        let payload = symmetric.decrypt_and_hash(rest)?;
        Ok(Self {
            symmetric,
            their_ephemeral,
            joiner: RecoveryPublic(joiner),
            payload,
        })
    }

    /// Admits the joiner: the answer, `<- e, ee, se`, with `payload`, and the channel.
    pub fn accept(mut self, payload: &[u8]) -> (Channel, Vec<u8>) {
        let ephemeral = StaticSecret::from(random::<32>());
        let ephemeral_public = PublicKey::from(&ephemeral).to_bytes();
        let mut message = ephemeral_public.to_vec();
        self.symmetric.mix_hash(&ephemeral_public);
        self.symmetric
            .mix_key(&dh(&ephemeral, &self.their_ephemeral));
        self.symmetric.mix_key(&dh(&ephemeral, &self.joiner.0));
        message.extend(self.symmetric.encrypt_and_hash(payload));
        let (receiving, sending) = self.symmetric.split();
        (
            Channel::new(sending, receiving, self.symmetric.hash),
            message,
        )
    }
}

/// An open channel: each side seals with its own key and opens with the other's, every message
/// carrying its counter, and takes a counter only past the last it took.
pub struct Channel {
    sending: [u8; 32],
    receiving: [u8; 32],
    sent: u64,
    /// The next counter it takes at least.
    next: u64,
    hash: [u8; 32],
}

impl Channel {
    fn new(sending: [u8; 32], receiving: [u8; 32], hash: [u8; 32]) -> Self {
        Self {
            sending,
            receiving,
            sent: 0,
            next: 0,
            hash,
        }
    }

    /// The handshake's hash, the same on both sides: what binds anything else to this channel.
    pub fn handshake_hash(&self) -> [u8; 32] {
        self.hash
    }

    /// `plain`, sealed as the next message: its counter, then the ciphertext.
    pub fn seal(&mut self, plain: &[u8]) -> Vec<u8> {
        let counter = self.sent;
        self.sent += 1;
        let mut sealed = counter.to_le_bytes().to_vec();
        sealed.extend(aead::seal(
            &self.sending,
            &nonce_of(counter),
            &counter.to_le_bytes(),
            plain,
        ));
        sealed
    }

    /// Opens a message the other side sealed: refused unless it opens under its key with its
    /// counter, and the counter is past every one taken before, so nothing is replayed or
    /// reordered. One that fails leaves the channel as it was.
    pub fn open(&mut self, sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        let mut rest = sealed;
        let counter = u64::from_le_bytes(
            take(&mut rest, 8)?
                .try_into()
                .map_err(|_| KeyError::Corrupt)?,
        );
        if counter < self.next {
            return Err(KeyError::Corrupt);
        }
        let plain = aead::open(
            &self.receiving,
            &nonce_of(counter),
            &counter.to_le_bytes(),
            rest,
        )
        .map_err(|_| KeyError::Corrupt)?;
        self.next = counter + 1;
        Ok(plain)
    }
}

impl std::fmt::Debug for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channel")
            .field("hash", &self.hash)
            .finish_non_exhaustive()
    }
}
