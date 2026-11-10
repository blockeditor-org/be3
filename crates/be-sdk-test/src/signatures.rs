//! The test double of `sign` and `verify` (plans/authorship.md): no key and no cryptography,
//! only the contract. Every account signs bytes the same way, `signature(account, bytes)`, and
//! verifying answers how far the test trusts the account (`BeuiTest::set_signer`) for that
//! signature and `Signer::Bad` for any other, so a test forges a message by giving it a
//! signature its author did not make.

use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use be_sdk_api::{Signer, embed::host_services::Call, ops::Connection};
use uuid::Uuid;

/// What `account` signs `bytes` with in a test.
pub fn signature(account: Uuid, bytes: &[u8]) -> Vec<u8> {
    let mut hasher = DefaultHasher::new();
    account.hash(&mut hasher);
    bytes.hash(&mut hasher);
    let mut signature = b"test signature".to_vec();
    signature.extend(account.as_bytes());
    signature.extend(hasher.finish().to_be_bytes());
    signature
}

pub(crate) struct Signatures {
    me: Uuid,
    trusted: BTreeMap<Uuid, Signer>,
}

impl Signatures {
    pub(crate) fn new(me: Uuid) -> Self {
        Self {
            me,
            trusted: BTreeMap::new(),
        }
    }

    pub(crate) fn set(&mut self, account: Uuid, signer: Signer) {
        self.trusted.insert(account, signer);
    }

    /// Answers `call` if it is `sign` or `verify`, or gives it back.
    pub(crate) fn answer(&self, call: Call, connection: &mut Connection) -> Option<Call> {
        match call {
            Call::Sign { reply, bytes } => {
                reply.send(connection, &Ok(signature(self.me, &bytes)));
                None
            }
            Call::Verify {
                reply,
                account,
                bytes,
                signature: signed,
            } => {
                let account = Uuid::from_bytes(account);
                let signer = match signed == signature(account, &bytes) {
                    true => self
                        .trusted
                        .get(&account)
                        .copied()
                        .unwrap_or(Signer::Verified),
                    false => Signer::Bad,
                };
                reply.send(connection, &Ok(signer));
                None
            }
            call => Some(call),
        }
    }
}
