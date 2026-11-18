//! The test double of `sign` and `verify` (plans/authorship.md): no key and no cryptography,
//! only the contract. Every account signs bytes the same way, `signature(account, bytes)`, and
//! verifying answers how far the test trusts the account (`BeuiTest::set_signer`) for that
//! signature (`Signer::Replaced` only for a record that says when it arrived, `Changed` for one
//! that does not) and `Signer::Bad` for any other, so a test forges a message by giving it a
//! signature its author did not make.

use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use be_sdk_api::{
    Signer,
    embed::host_services::Call,
    ops::{OpId, Outcome},
};
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
    signing_block: Option<Uuid>,
}

impl Signatures {
    pub(crate) fn new(me: Uuid) -> Self {
        Self {
            me,
            trusted: BTreeMap::new(),
            signing_block: None,
        }
    }

    pub(crate) fn set(&mut self, account: Uuid, signer: Signer) {
        self.trusted.insert(account, signer);
    }

    /// What `signing-block` answers instead of the block shown, as for a revision preview.
    pub(crate) fn set_signing_block(&mut self, block: Uuid) {
        self.signing_block = Some(block);
    }

    /// The answer to `call` if it is `sign`, `verify` or `signing-block`, or the call back;
    /// `shown` is the block the editor shows.
    pub(crate) fn answer(&self, call: Call, shown: Option<Uuid>) -> Result<(OpId, Outcome), Call> {
        match call {
            Call::SigningBlock { reply } => {
                let block = self.signing_block.or(shown).unwrap_or_default();
                Ok(reply.answer(&Ok(block.into_bytes())))
            }
            Call::Sign { reply, bytes } => Ok(reply.answer(&Ok(signature(self.me, &bytes)))),
            Call::Verify {
                reply,
                account,
                bytes,
                signature: signed,
                arrived,
            } => {
                let account = Uuid::from_bytes(account);
                let signer = match signed == signature(account, &bytes) {
                    true => match self.trusted.get(&account).copied() {
                        // Only a record that keeps when the server took it can show it
                        // arrived after the key was replaced, as the host's `verify` answers.
                        Some(Signer::Replaced) if arrived.is_none() => Signer::Changed,
                        trusted => trusted.unwrap_or(Signer::Verified),
                    },
                    false => Signer::Bad,
                };
                Ok(reply.answer(&Ok(signer)))
            }
            call => Err(call),
        }
    }
}
