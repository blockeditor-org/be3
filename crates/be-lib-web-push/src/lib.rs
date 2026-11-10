//! Web Push message encryption (RFC 8291, the `aes128gcm` content coding of RFC 8188): both
//! halves, so the server that sends a push and a device that receives one without a browser (a
//! UnifiedPush or FCM receiver) share one implementation. Knows nothing of blocks.

use ring::{
    aead, agreement, hmac,
    rand::{SecureRandom, SystemRandom},
};

/// The record size every push declares (RFC 8188): one record holds the whole payload.
const RECORD_SIZE: u32 = 4096;
/// The header before the ciphertext: salt, record size, key id length, the sender's key.
pub const HEADER_BYTES: usize = 16 + 4 + 1 + 65;
/// The most plaintext one push carries: the record, less its delimiter and tag.
pub const MAX_PLAIN_BYTES: usize = RECORD_SIZE as usize - 1 - 16;

/// Why a push could not be encrypted.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The device's keys are not a P-256 public key and a 16-byte secret.
    Keys,
    /// The plaintext, or its padding, does not fit one record.
    TooLong,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Keys => "the keys are not a P-256 public key and a 16-byte secret",
            Self::TooLong => "the message does not fit one push",
        })
    }
}

impl std::error::Error for Error {}

/// Encrypts `plain` to a device's keys with a new sender key pair and salt, padded to at least
/// `padded` bytes so the push's length says nothing more of what it carries.
pub fn encrypt(
    ua_public: &[u8],
    auth: &[u8],
    plain: &[u8],
    padded: usize,
) -> Result<Vec<u8>, Error> {
    if auth.len() != 16 {
        return Err(Error::Keys);
    }
    if plain.len().max(padded) > MAX_PLAIN_BYTES {
        return Err(Error::TooLong);
    }
    let random = SystemRandom::new();
    let private = agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, &random)
        .map_err(|_| Error::Keys)?;
    let as_public = private.compute_public_key().map_err(|_| Error::Keys)?;
    let mut salt = [0; 16];
    random.fill(&mut salt).map_err(|_| Error::Keys)?;
    let peer = agreement::UnparsedPublicKey::new(&agreement::ECDH_P256, ua_public);
    agreement::agree_ephemeral(private, &peer, |secret| {
        seal(
            secret,
            as_public.as_ref(),
            ua_public,
            auth,
            &salt,
            plain,
            padded,
        )
    })
    .map_err(|_| Error::Keys)
}

/// RFC 8291's encryption from the shared secret on: the content encryption key and nonce, then
/// the header and the one record, `plain` padded with zeros after its delimiter to `padded`
/// bytes. `encrypt` with every random input given.
pub fn seal(
    ecdh_secret: &[u8],
    as_public: &[u8],
    ua_public: &[u8],
    auth: &[u8],
    salt: &[u8; 16],
    plain: &[u8],
    padded: usize,
) -> Vec<u8> {
    let (key, nonce) = derive(ecdh_secret, as_public, ua_public, auth, salt);
    let mut record = plain.to_vec();
    record.push(2);
    record.resize(plain.len().max(padded) + 1, 0);
    key.seal_in_place_append_tag(nonce, aead::Aad::empty(), &mut record)
        .expect("a push fits one record");
    let mut body = Vec::with_capacity(HEADER_BYTES + record.len());
    body.extend_from_slice(salt);
    body.extend_from_slice(&RECORD_SIZE.to_be_bytes());
    body.push(u8::try_from(as_public.len()).expect("a P-256 key is 65 bytes"));
    body.extend_from_slice(as_public);
    body.extend_from_slice(&record);
    body
}

/// The sender's public key in a push, which the device agrees on a secret with.
pub fn sender_key(body: &[u8]) -> Option<&[u8]> {
    let length = usize::from(*body.get(20)?);
    body.get(21..21 + length)
}

/// Decrypts a push given the secret the device agreed on with `sender_key`, dropping its
/// padding.
pub fn open(ecdh_secret: &[u8], ua_public: &[u8], auth: &[u8], body: &[u8]) -> Option<Vec<u8>> {
    let salt: [u8; 16] = body.get(..16)?.try_into().ok()?;
    let as_public = sender_key(body)?;
    let start = 21 + as_public.len();
    let (key, nonce) = derive(ecdh_secret, as_public, ua_public, auth, &salt);
    let mut record = body.get(start..)?.to_vec();
    let plain = key
        .open_in_place(nonce, aead::Aad::empty(), &mut record)
        .ok()?;
    let end = plain.iter().rposition(|byte| *byte != 0)?;
    (plain[end] == 2).then(|| plain[..end].to_vec())
}

fn derive(
    ecdh_secret: &[u8],
    as_public: &[u8],
    ua_public: &[u8],
    auth: &[u8],
    salt: &[u8; 16],
) -> (aead::LessSafeKey, aead::Nonce) {
    let mac = |key: &[u8], parts: &[&[u8]]| {
        let mut context = hmac::Context::with_key(&hmac::Key::new(hmac::HMAC_SHA256, key));
        for part in parts {
            context.update(part);
        }
        context.sign()
    };
    let prk_key = mac(auth, &[ecdh_secret]);
    let ikm = mac(
        prk_key.as_ref(),
        &[b"WebPush: info\0", ua_public, as_public, &[1]],
    );
    let prk = mac(salt, &[ikm.as_ref()]);
    let cek = mac(prk.as_ref(), &[b"Content-Encoding: aes128gcm\0", &[1]]);
    let nonce = mac(prk.as_ref(), &[b"Content-Encoding: nonce\0", &[1]]);
    let key = aead::UnboundKey::new(&aead::AES_128_GCM, &cek.as_ref()[..16])
        .expect("16 bytes is an AES-128 key");
    let nonce = aead::Nonce::try_assume_unique_for_key(&nonce.as_ref()[..12])
        .expect("12 bytes is a GCM nonce");
    (aead::LessSafeKey::new(key), nonce)
}

#[cfg(test)]
mod tests;
