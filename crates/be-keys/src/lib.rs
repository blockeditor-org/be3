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
const INVITE_INFO: &[u8] = b"be3.invite.v1";
const INVITE_LOOKUP_INFO: &[u8] = b"be3.invite.lookup.v1";
const OWN_INFO: &[u8] = b"be3.own.v1";
const CARD_OWNER: &[u8] = b"be3.contact.owner.v1";
const CARD_ACCEPTER: &[u8] = b"be3.contact.accepter.v1";
const GRANT_INFO: &[u8] = b"be3.contact.grant.v1";
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
}

impl fmt::Display for KeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Phrase => formatter.write_str("that is not a recovery phrase"),
            Self::Corrupt => formatter.write_str("the key did not open with this secret"),
            Self::Pairing => formatter.write_str("the code did not match"),
            Self::Invite => formatter.write_str("that is not an invite code"),
            Self::Card => formatter.write_str("the contact card did not open with this link"),
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

    pub fn seal_own(&self, plain: &[u8]) -> Vec<u8> {
        seal_with(&self.own_key(), plain)
    }

    pub fn open_own(&self, sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        open_with(&self.own_key(), sealed, KeyError::Corrupt)
    }

    fn own_key(&self) -> [u8; 32] {
        expand(&self.0.to_bytes(), OWN_INFO, OWN_INFO)
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

#[derive(Clone, Eq, PartialEq)]
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

    pub fn seal(&self, context: &[u8], plain: &[u8]) -> Vec<u8> {
        seal_with(&self.key(context), plain)
    }

    pub fn open(&self, context: &[u8], sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        open_with(&self.key(context), sealed, KeyError::Invite)
    }

    pub fn seal_grant(&self, key: &[u8]) -> Vec<u8> {
        self.seal(GRANT_INFO, key)
    }

    pub fn open_grant(&self, sealed: &[u8]) -> Result<Vec<u8>, KeyError> {
        self.open(GRANT_INFO, sealed)
    }

    fn key(&self, context: &[u8]) -> [u8; 32] {
        expand(&self.0, context, INVITE_INFO)
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

    pub fn seal_own(&self, secret: &RecoverySecret) -> Vec<u8> {
        secret.seal_own(&self.encode())
    }

    pub fn open_own(secret: &RecoverySecret, sealed: &[u8]) -> Result<Self, KeyError> {
        Self::decode(&secret.open_own(sealed)?)
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

impl fmt::Debug for InviteCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("InviteCode(..)")
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
