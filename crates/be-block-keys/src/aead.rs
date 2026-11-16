//! The one symmetric seal (decisions/one_symmetric_seal.md): XChaCha20-Poly1305 with a 24-byte
//! nonce and associated data. Every symmetric seal draws on it: the vault's objects (whose
//! header and block are the associated data), this crate's seals to accounts, own seals,
//! invitations and pairing (their context), and the server's backups. They differ only in what
//! they bind and how they choose the nonce.

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::{Hkdf, HkdfExtract};
use sha2::Sha256;

pub const NONCE_LEN: usize = 24;

pub type Nonce = [u8; NONCE_LEN];

/// A seal did not open: the key, the nonce, the associated data or the bytes were not those it
/// was made with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Unopened;

/// `plain` sealed under `key` with `nonce`, authenticating `associated` too: the ciphertext and
/// its tag, without the nonce, which the caller keeps beside it.
pub fn seal(key: &[u8; 32], nonce: &Nonce, associated: &[u8], plain: &[u8]) -> Vec<u8> {
    XChaCha20Poly1305::new(key.into())
        .encrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: plain,
                aad: associated,
            },
        )
        .expect("xchacha20poly1305 never fails on a valid key and nonce")
}

/// Opens what `seal` made with the same key, nonce and associated data.
pub fn open(
    key: &[u8; 32],
    nonce: &Nonce,
    associated: &[u8],
    sealed: &[u8],
) -> Result<Vec<u8>, Unopened> {
    XChaCha20Poly1305::new(key.into())
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: sealed,
                aad: associated,
            },
        )
        .map_err(|_| Unopened)
}

/// A nonce drawn at random: for what is never deduplicated, so two equal plaintexts seal
/// differently.
pub fn random_nonce() -> Nonce {
    crate::random()
}

/// A nonce derived from what is sealed, so equal plaintexts under one key and one binding seal
/// to equal bytes and deduplicate: a keyed hash, under a nonce key drawn from `key` (never `key`
/// itself), of `binding` and `plain`. 192 bits leave collisions out of reach.
pub fn derived_nonce(key: &[u8; 32], binding: &[u8], plain: &[u8]) -> Nonce {
    let mut nonce_key = [0u8; 32];
    Hkdf::<Sha256>::new(None, key)
        .expand(NONCE_INFO, &mut nonce_key)
        .expect("32 bytes is a valid hkdf length");
    // HKDF's extract is HMAC under the nonce key, fed a piece at a time.
    let mut extract = HkdfExtract::<Sha256>::new(Some(&nonce_key));
    extract.input_ikm(&(binding.len() as u64).to_be_bytes());
    extract.input_ikm(binding);
    extract.input_ikm(plain);
    let (_, derived) = extract.finalize();
    let mut nonce = [0u8; NONCE_LEN];
    derived
        .expand(NONCE_INFO, &mut nonce)
        .expect("24 bytes is a valid hkdf length");
    nonce
}

const NONCE_INFO: &[u8] = b"be3.vault.nonce.v1";
