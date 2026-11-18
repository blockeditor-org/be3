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
//! The cipher is a private parameter (`Cipher`): tests run the same code as
//! `Noise_IK_25519_ChaChaPoly_SHA256` against a published vector.
//!
//! IK's first message can be replayed (Noise, section 7.7): a responder knows the initiator only
//! once a transport message from it opens. After the handshake each message carries its counter
//! (`Channel::seal`), and a receiver takes only the next counter (`Channel::open`); any other
//! breaks the channel for good, and the member handshakes again. So the server can neither read,
//! forge, reorder, replay nor drop a transport message unseen.

use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::{KeyError, RecoveryPublic, RecoverySecret, aead, random};

/// What a hosted block's prologue starts with (`hosted_prologue`).
const HOSTED_INFO: &[u8] = b"be3.hosted.v1";
const KEY_LEN: usize = 32;
const TAG_LEN: usize = 16;

/// The prologue both sides of a hosted block's channel mix in first: the block and the head of
/// its access log the joiner checked the host's right at.
pub fn hosted_prologue(block: [u8; 16], head: crate::EntryHash) -> Vec<u8> {
    [HOSTED_INFO, &block, &head].concat()
}

/// Overwrites `bytes` with zeros in a way the compiler keeps, for a secret going out of use.
fn wipe(bytes: &mut [u8]) {
    zeroize::Zeroize::zeroize(bytes);
}

/// The cipher the protocol runs on. The channel always uses this crate's one seal; Noise's own
/// ChaChaPoly runs only in tests, to check the handshake against published vectors.
#[derive(Clone, Copy)]
pub(crate) enum Cipher {
    /// XChaCha20-Poly1305 (`aead`), the counter in the first 8 bytes of its 24-byte nonce.
    XChaChaPoly,
    /// IETF ChaCha20-Poly1305, the counter after 4 zero bytes of its 12-byte nonce.
    #[cfg(test)]
    ChaChaPoly,
}

impl Cipher {
    fn protocol(self) -> &'static [u8] {
        match self {
            Cipher::XChaChaPoly => b"Noise_IK_25519_XChaChaPoly_SHA256",
            #[cfg(test)]
            Cipher::ChaChaPoly => b"Noise_IK_25519_ChaChaPoly_SHA256",
        }
    }

    fn seal(self, key: &[u8; KEY_LEN], counter: u64, associated: &[u8], plain: &[u8]) -> Vec<u8> {
        match self {
            Cipher::XChaChaPoly => {
                let mut nonce = [0u8; aead::NONCE_LEN];
                nonce[..8].copy_from_slice(&counter.to_le_bytes());
                aead::seal(key, &nonce, associated, plain)
            }
            #[cfg(test)]
            Cipher::ChaChaPoly => {
                use chacha20poly1305::aead::{Aead, KeyInit, Payload};
                let mut nonce = [0u8; 12];
                nonce[4..].copy_from_slice(&counter.to_le_bytes());
                chacha20poly1305::ChaCha20Poly1305::new(key.into())
                    .encrypt(
                        (&nonce).into(),
                        Payload {
                            msg: plain,
                            aad: associated,
                        },
                    )
                    .expect("chacha20poly1305 never fails on a valid key and nonce")
            }
        }
    }

    fn open(
        self,
        key: &[u8; KEY_LEN],
        counter: u64,
        associated: &[u8],
        sealed: &[u8],
    ) -> Result<Vec<u8>, KeyError> {
        match self {
            Cipher::XChaChaPoly => {
                let mut nonce = [0u8; aead::NONCE_LEN];
                nonce[..8].copy_from_slice(&counter.to_le_bytes());
                aead::open(key, &nonce, associated, sealed).map_err(|_| KeyError::Corrupt)
            }
            #[cfg(test)]
            Cipher::ChaChaPoly => {
                use chacha20poly1305::aead::{Aead, KeyInit, Payload};
                let mut nonce = [0u8; 12];
                nonce[4..].copy_from_slice(&counter.to_le_bytes());
                chacha20poly1305::ChaCha20Poly1305::new(key.into())
                    .decrypt(
                        (&nonce).into(),
                        Payload {
                            msg: sealed,
                            aad: associated,
                        },
                    )
                    .map_err(|_| KeyError::Corrupt)
            }
        }
    }
}

/// Noise's CipherState during the handshake.
struct CipherState {
    key: Option<[u8; KEY_LEN]>,
    nonce: u64,
}

impl Drop for CipherState {
    fn drop(&mut self) {
        if let Some(key) = &mut self.key {
            wipe(key);
        }
    }
}

/// Noise's SymmetricState: the chaining key, the handshake hash, and the cipher keyed so far.
struct Symmetric {
    cipher: Cipher,
    chaining: [u8; 32],
    hash: [u8; 32],
    state: CipherState,
}

impl Drop for Symmetric {
    fn drop(&mut self) {
        wipe(&mut self.chaining);
    }
}

/// Noise's HKDF with two outputs: HMAC-SHA256 with the chaining key as salt, which is RFC 5869's
/// HKDF with empty info.
fn hkdf2(chaining: &[u8; 32], input: &[u8]) -> ([u8; 32], [u8; 32]) {
    let mut out = [0u8; 64];
    Hkdf::<Sha256>::new(Some(chaining), input)
        .expand(&[], &mut out)
        .expect("64 bytes is a valid length for HKDF-SHA256");
    let mut first = [0u8; 32];
    let mut second = [0u8; 32];
    first.copy_from_slice(&out[..32]);
    second.copy_from_slice(&out[32..]);
    wipe(&mut out);
    (first, second)
}

impl Symmetric {
    fn new(cipher: Cipher, prologue: &[u8]) -> Self {
        let protocol = cipher.protocol();
        let hash: [u8; 32] = match protocol.len() <= 32 {
            true => {
                let mut hash = [0u8; 32];
                hash[..protocol.len()].copy_from_slice(protocol);
                hash
            }
            false => Sha256::digest(protocol).into(),
        };
        let mut state = Self {
            cipher,
            chaining: hash,
            hash,
            state: CipherState {
                key: None,
                nonce: 0,
            },
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
        wipe(&mut self.chaining);
        self.chaining = chaining;
        self.state = CipherState {
            key: Some(key),
            nonce: 0,
        };
    }

    fn encrypt_and_hash(&mut self, plain: &[u8]) -> Vec<u8> {
        let sealed = match &self.state.key {
            Some(key) => {
                let sealed = self.cipher.seal(key, self.state.nonce, &self.hash, plain);
                self.state.nonce += 1;
                sealed
            }
            None => plain.to_vec(),
        };
        self.mix_hash(&sealed);
        sealed
    }

    fn decrypt_and_hash(&mut self, sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        let plain = match &self.state.key {
            Some(key) => {
                let plain = self
                    .cipher
                    .open(key, self.state.nonce, &self.hash, sealed)?;
                self.state.nonce += 1;
                plain
            }
            None => sealed.to_vec(),
        };
        self.mix_hash(sealed);
        Ok(plain)
    }

    /// The two transport keys: the initiator's sending one first.
    fn split(&self) -> ([u8; 32], [u8; 32]) {
        hkdf2(&self.chaining, &[])
    }
}

/// The shared secret of `secret` and `public`; the caller mixes it in at once, and it is wiped.
fn dh(secret: &StaticSecret, public: &[u8; 32]) -> [u8; 32] {
    secret.diffie_hellman(&PublicKey::from(*public)).to_bytes()
}

impl Symmetric {
    fn mix_dh(&mut self, secret: &StaticSecret, public: &[u8; 32]) {
        let mut shared = dh(secret, public);
        self.mix_key(&shared);
        wipe(&mut shared);
    }
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
        let ephemeral = StaticSecret::from(random::<32>());
        Self::start_with(Cipher::XChaChaPoly, ephemeral, own, host, prologue, payload)
    }

    /// `start` on `cipher`, with the ephemeral key given: what a test checks a published
    /// vector with.
    pub(crate) fn start_with(
        cipher: Cipher,
        ephemeral: StaticSecret,
        own: &RecoverySecret,
        host: &RecoveryPublic,
        prologue: &[u8],
        payload: &[u8],
    ) -> (Self, Vec<u8>) {
        let mut symmetric = Symmetric::new(cipher, prologue);
        // The pre-message `<- s`.
        symmetric.mix_hash(&host.0);
        let ephemeral_public = PublicKey::from(&ephemeral).to_bytes();
        let mut message = ephemeral_public.to_vec();
        symmetric.mix_hash(&ephemeral_public);
        symmetric.mix_dh(&ephemeral, &host.0);
        message.extend(symmetric.encrypt_and_hash(&own.public().0));
        symmetric.mix_dh(&own.0, &host.0);
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
        self.symmetric.mix_dh(&self.ephemeral, &theirs);
        self.symmetric.mix_dh(&self.own.0, &theirs);
        let payload = self.symmetric.decrypt_and_hash(rest)?;
        let (sending, receiving) = self.symmetric.split();
        Ok((
            Channel::new(
                self.symmetric.cipher,
                sending,
                receiving,
                self.symmetric.hash,
            ),
            payload,
        ))
    }
}

/// A host's side of the handshake, the joiner's first message read: who it is (its static key)
/// and what it said, for the host to check before it answers (`accept`). The first message can
/// be replayed, so the joiner is known only once a transport message from it opens.
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
        Self::read_with(Cipher::XChaChaPoly, own, prologue, message)
    }

    pub(crate) fn read_with(
        cipher: Cipher,
        own: &RecoverySecret,
        prologue: &[u8],
        message: &[u8],
    ) -> Result<Self, KeyError> {
        let mut symmetric = Symmetric::new(cipher, prologue);
        symmetric.mix_hash(&own.public().0);
        let mut rest = message;
        let their_ephemeral: [u8; 32] = take(&mut rest, 32)?
            .try_into()
            .map_err(|_| KeyError::Corrupt)?;
        symmetric.mix_hash(&their_ephemeral);
        symmetric.mix_dh(&own.0, &their_ephemeral);
        let sealed_static = take(&mut rest, 32 + TAG_LEN)?;
        let joiner: [u8; 32] = symmetric
            .decrypt_and_hash(sealed_static)?
            .try_into()
            .map_err(|_| KeyError::Corrupt)?;
        symmetric.mix_dh(&own.0, &joiner);
        let payload = symmetric.decrypt_and_hash(rest)?;
        Ok(Self {
            symmetric,
            their_ephemeral,
            joiner: RecoveryPublic(joiner),
            payload,
        })
    }

    /// Answers the joiner, `<- e, ee, se`, with `payload`: the channel and the answer.
    pub fn accept(self, payload: &[u8]) -> (Channel, Vec<u8>) {
        self.accept_with(StaticSecret::from(random::<32>()), payload)
    }

    /// `accept` with the ephemeral key given: what a test checks a published vector with.
    pub(crate) fn accept_with(
        mut self,
        ephemeral: StaticSecret,
        payload: &[u8],
    ) -> (Channel, Vec<u8>) {
        let ephemeral_public = PublicKey::from(&ephemeral).to_bytes();
        let mut message = ephemeral_public.to_vec();
        self.symmetric.mix_hash(&ephemeral_public);
        self.symmetric.mix_dh(&ephemeral, &self.their_ephemeral);
        self.symmetric.mix_dh(&ephemeral, &self.joiner.0);
        message.extend(self.symmetric.encrypt_and_hash(payload));
        let (receiving, sending) = self.symmetric.split();
        (
            Channel::new(
                self.symmetric.cipher,
                sending,
                receiving,
                self.symmetric.hash,
            ),
            message,
        )
    }
}

/// An open channel: each side seals with its own key and opens with the other's, every message
/// carrying its counter. It takes only the next counter: one out of place (dropped, reordered or
/// replayed) breaks it for good, and the member handshakes again.
pub struct Channel {
    cipher: Cipher,
    sending: [u8; 32],
    receiving: [u8; 32],
    sent: u64,
    /// The counter it takes next.
    next: u64,
    broken: bool,
    hash: [u8; 32],
}

impl Drop for Channel {
    fn drop(&mut self) {
        wipe(&mut self.sending);
        wipe(&mut self.receiving);
    }
}

impl Channel {
    fn new(cipher: Cipher, sending: [u8; 32], receiving: [u8; 32], hash: [u8; 32]) -> Self {
        Self {
            cipher,
            sending,
            receiving,
            sent: 0,
            next: 0,
            broken: false,
            hash,
        }
    }

    /// The handshake's hash, the same on both sides: what binds anything else to this channel.
    pub fn handshake_hash(&self) -> [u8; 32] {
        self.hash
    }

    /// Whether a message out of place broke it: it opens nothing more.
    pub fn is_broken(&self) -> bool {
        self.broken
    }

    /// `plain`, sealed as the next message: its counter, then the ciphertext. The counter
    /// advances now, so a send the server refuses (`SlowDown`) or that fails is sent again with
    /// these same bytes, never sealed again: another seal would leave a gap that breaks the
    /// channel.
    pub fn seal(&mut self, plain: &[u8]) -> Vec<u8> {
        let counter = self.sent;
        self.sent += 1;
        let mut sealed = counter.to_le_bytes().to_vec();
        sealed.extend(
            self.cipher
                .seal(&self.sending, counter, &counter.to_le_bytes(), plain),
        );
        sealed
    }

    /// Opens the next message the other side sealed. One that does not open under its key with
    /// the next counter is refused and breaks the channel: after it, nothing opens.
    pub fn open(&mut self, sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        if self.broken {
            return Err(KeyError::Corrupt);
        }
        let opened = (|| {
            let mut rest = sealed;
            let counter = u64::from_le_bytes(
                take(&mut rest, 8)?
                    .try_into()
                    .map_err(|_| KeyError::Corrupt)?,
            );
            if counter != self.next {
                return Err(KeyError::Corrupt);
            }
            self.cipher
                .open(&self.receiving, counter, &counter.to_le_bytes(), rest)
        })();
        match opened {
            Ok(plain) => {
                self.next += 1;
                Ok(plain)
            }
            Err(error) => {
                self.broken = true;
                Err(error)
            }
        }
    }
}

impl std::fmt::Debug for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channel")
            .field("hash", &self.hash)
            .field("broken", &self.broken)
            .finish_non_exhaustive()
    }
}
