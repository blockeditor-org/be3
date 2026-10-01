use std::fmt;

use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit},
};
use hkdf::Hkdf;
use rand::TryRngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use x25519_dalek::{PublicKey, StaticSecret};

const NONCE_LEN: usize = 12;
const PHRASE_ENTROPY: usize = 16;
const RECOVERY_INFO: &[u8] = b"be3.recovery.v1";
const SEAL_INFO: &[u8] = b"be3.seal.v1";
const PAIRING_CONTEXT: &[u8] = b"be3.pairing.v1";
const CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
pub const CODE_LEN: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyError {
    Phrase,
    Corrupt,
    Pairing,
}

impl fmt::Display for KeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Phrase => formatter.write_str("that is not a recovery phrase"),
            Self::Corrupt => formatter.write_str("the key did not open with this secret"),
            Self::Pairing => formatter.write_str("the code did not match"),
        }
    }
}

impl std::error::Error for KeyError {}

fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .expect("the operating system has entropy");
    bytes
}

fn seal_with(key: &[u8; 32], plain: &[u8]) -> Vec<u8> {
    let nonce: [u8; NONCE_LEN] = random();
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plain)
        .expect("chacha20poly1305 never fails on a valid key and nonce");
    let mut sealed = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    sealed
}

fn open_with(key: &[u8; 32], sealed: &[u8], error: KeyError) -> Result<Vec<u8>, KeyError> {
    let (nonce, ciphertext) = sealed.split_at_checked(NONCE_LEN).ok_or(error)?;
    ChaCha20Poly1305::new(Key::from_slice(key))
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| error)
}

fn expand(secret: &[u8], salt: &[u8], info: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    Hkdf::<Sha256>::new(Some(salt), secret)
        .expand(info, &mut out)
        .expect("32 bytes is a valid hkdf length");
    out
}

#[derive(Clone, Eq, PartialEq)]
pub struct RecoveryPhrase {
    entropy: [u8; PHRASE_ENTROPY],
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

pub struct RecoverySecret(StaticSecret);

impl RecoverySecret {
    pub fn public(&self) -> RecoveryPublic {
        RecoveryPublic(PublicKey::from(&self.0).to_bytes())
    }

    pub fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        let (ephemeral, rest) = sealed.split_at_checked(32).ok_or(KeyError::Corrupt)?;
        let ephemeral: [u8; 32] = ephemeral.try_into().map_err(|_| KeyError::Corrupt)?;
        let shared = self.0.diffie_hellman(&PublicKey::from(ephemeral));
        let key = expand(
            shared.as_bytes(),
            &[ephemeral, self.public().0].concat(),
            SEAL_INFO,
        );
        open_with(&key, rest, KeyError::Corrupt)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct RecoveryPublic(pub [u8; 32]);

impl RecoveryPublic {
    pub fn seal(&self, plain: &[u8]) -> Vec<u8> {
        let ephemeral = StaticSecret::from(random::<32>());
        let ephemeral_public = PublicKey::from(&ephemeral).to_bytes();
        let shared = ephemeral.diffie_hellman(&PublicKey::from(self.0));
        let key = expand(
            shared.as_bytes(),
            &[ephemeral_public, self.0].concat(),
            SEAL_INFO,
        );
        let mut sealed = ephemeral_public.to_vec();
        sealed.extend(seal_with(&key, plain));
        sealed
    }
}

pub fn pairing_code() -> String {
    random::<CODE_LEN>()
        .iter()
        .map(|byte| CODE_ALPHABET[usize::from(byte % 32)] as char)
        .collect()
}

pub fn normalize_code(input: &str) -> Option<String> {
    let code: String = input
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| match character.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect();
    (code.len() == CODE_LEN && code.bytes().all(|byte| CODE_ALPHABET.contains(&byte)))
        .then_some(code)
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

impl PairingKey {
    pub fn seal(&self, plain: &[u8]) -> Vec<u8> {
        seal_with(&self.0, plain)
    }

    pub fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        open_with(&self.0, sealed, KeyError::Pairing)
    }
}

#[cfg(test)]
mod tests;
