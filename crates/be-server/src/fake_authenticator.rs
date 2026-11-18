//! A software authenticator for be-server's tests: it makes an ES256 passkey with real "none"
//! attestation and signs real assertions, answering in the JSON the portal's wasm posts
//! (`PublicKeyCredential.toJSON()`). A test plays the browser: it reads the options a page
//! offers and hands the authenticator the origin the browser would report.

use base64::Engine;
use ring::rand::SystemRandom;
use ring::signature::{ECDSA_P256_SHA256_ASN1_SIGNING, EcdsaKeyPair, KeyPair};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn unb64(text: &str) -> Vec<u8> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(text.trim_end_matches('='))
        .unwrap()
}

/// A CBOR item's head: its major type and its length or value.
fn head(major: u8, value: u64, out: &mut Vec<u8>) {
    let major = major << 5;
    match value {
        0..=23 => out.push(major | value as u8),
        24..=0xff => out.extend([major | 24, value as u8]),
        0x100..=0xffff => {
            out.push(major | 25);
            out.extend((value as u16).to_be_bytes());
        }
        _ => {
            out.push(major | 26);
            out.extend((value as u32).to_be_bytes());
        }
    }
}

fn int(value: i64, out: &mut Vec<u8>) {
    match value >= 0 {
        true => head(0, value as u64, out),
        false => head(1, (-1 - value) as u64, out),
    }
}

fn bytes(value: &[u8], out: &mut Vec<u8>) {
    head(2, value.len() as u64, out);
    out.extend(value);
}

fn text(value: &str, out: &mut Vec<u8>) {
    head(3, value.len() as u64, out);
    out.extend(value.as_bytes());
}

pub struct FakeAuthenticator {
    key: EcdsaKeyPair,
    pub credential_id: Vec<u8>,
    user_handle: Vec<u8>,
    /// The RP ID it signs for: the one it was made for, unless a test changes it.
    pub rp_id: String,
    pub counter: u32,
}

impl FakeAuthenticator {
    /// Makes a passkey for the options a page offers to add one, as a browser at `origin` asks
    /// for it, and answers the authenticator and what the browser posts. `rp_id`, when given,
    /// is the RP ID it claims instead of the one in the options.
    pub fn create(options: &str, origin: &str, rp_id: Option<&str>) -> (Self, String) {
        let options: Value = serde_json::from_str(options).unwrap();
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng)
            .unwrap();
        let authenticator = Self {
            key,
            credential_id: uuid::Uuid::new_v4().as_bytes().to_vec(),
            user_handle: unb64(options["user"]["id"].as_str().unwrap()),
            rp_id: rp_id
                .unwrap_or_else(|| options["rp"]["id"].as_str().unwrap())
                .to_owned(),
            counter: 1,
        };
        let client_data = authenticator.client_data("webauthn.create", &options, origin);
        let point = authenticator.key.public_key().as_ref();
        let mut cose = Vec::new();
        head(5, 5, &mut cose);
        int(1, &mut cose);
        int(2, &mut cose); // kty: EC2
        int(3, &mut cose);
        int(-7, &mut cose); // alg: ES256
        int(-1, &mut cose);
        int(1, &mut cose); // crv: P-256
        int(-2, &mut cose);
        bytes(&point[1..33], &mut cose);
        int(-3, &mut cose);
        bytes(&point[33..65], &mut cose);
        // User present, user verified, attested credential data.
        let mut data = authenticator.data(0x01 | 0x04 | 0x40);
        data.extend([0; 16]);
        data.extend((authenticator.credential_id.len() as u16).to_be_bytes());
        data.extend(&authenticator.credential_id);
        data.extend(cose);
        let mut attestation = Vec::new();
        head(5, 3, &mut attestation);
        text("fmt", &mut attestation);
        text("none", &mut attestation);
        text("attStmt", &mut attestation);
        head(5, 0, &mut attestation);
        text("authData", &mut attestation);
        bytes(&data, &mut attestation);
        let id = b64(&authenticator.credential_id);
        let answer = json!({
            "id": id,
            "rawId": id,
            "type": "public-key",
            "response": {
                "attestationObject": b64(&attestation),
                "clientDataJSON": b64(client_data.as_bytes()),
                "transports": ["internal"],
            },
            "clientExtensionResults": {},
            "authenticatorAttachment": "platform",
        });
        (authenticator, answer.to_string())
    }

    /// Signs the challenge in the options a page offers to sign in or prove, as a browser at
    /// `origin` asks for it, and answers what the browser posts.
    pub fn get(&mut self, options: &str, origin: &str) -> String {
        let options: Value = serde_json::from_str(options).unwrap();
        self.counter += 1;
        let client_data = self.client_data("webauthn.get", &options, origin);
        let data = self.data(0x01 | 0x04);
        let mut signed = data.clone();
        signed.extend(Sha256::digest(client_data.as_bytes()));
        let signature = self.key.sign(&SystemRandom::new(), &signed).unwrap();
        let id = b64(&self.credential_id);
        json!({
            "id": id,
            "rawId": id,
            "type": "public-key",
            "response": {
                "authenticatorData": b64(&data),
                "clientDataJSON": b64(client_data.as_bytes()),
                "signature": b64(signature.as_ref()),
                "userHandle": b64(&self.user_handle),
            },
            "clientExtensionResults": {},
            "authenticatorAttachment": "platform",
        })
        .to_string()
    }

    fn client_data(&self, kind: &str, options: &Value, origin: &str) -> String {
        json!({
            "type": kind,
            "challenge": options["challenge"],
            "origin": origin,
            "crossOrigin": false,
        })
        .to_string()
    }

    /// Authenticator data with `flags`: the RP ID's hash, the flags and the counter.
    fn data(&self, flags: u8) -> Vec<u8> {
        let mut data = Sha256::digest(self.rp_id.as_bytes()).to_vec();
        data.push(flags);
        data.extend(self.counter.to_be_bytes());
        data
    }
}
