//! Signing what the account writes, and telling who signed what others wrote
//! (plans/authorship.md).

use be_block_keys::SigningPublic;
use be_block_protocol::client;
use be_block_store::ObjectStore;
use uuid::Uuid;

use super::Peer;
use crate::ClientError;

/// Who signed bytes, as far as this device can tell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Signer {
    /// The account's key, verified through a contact link or a key change sealed with the one
    /// before (or this account's own).
    Verified,
    /// The key the server named the first time this device asked, pinned since and never
    /// verified.
    Unverified,
    /// A key the server names now that is not the one this device pinned: the account took a
    /// new phrase, or the server is lying.
    Changed,
    /// No key this device knows the account by, nor one the server names, made the signature:
    /// a forgery, or bytes changed since they were signed.
    Bad,
}

impl<S: ObjectStore> Peer<S> {
    /// Signs `bytes` the account wrote in `block` with its signing key.
    pub fn sign_authored(&self, block: Uuid, bytes: &[u8]) -> Vec<u8> {
        self.keys
            .identity()
            .sign_authored(block.into_bytes(), bytes)
    }

    /// Who of `account` signed `bytes` in `block`. A key a contact verified is the only one
    /// that answers `Verified`, and an account with one is never taken on the server's word,
    /// so the server cannot sign as a contact. Any other account's key comes from the server and
    /// is pinned the first time (trust on first use).
    pub async fn verify_authored(
        &self,
        account: Uuid,
        block: Uuid,
        bytes: &[u8],
        signature: &[u8],
    ) -> Result<Signer, ClientError> {
        let made = |key: [u8; 32]| {
            SigningPublic(key).verifies_authored(block.into_bytes(), bytes, signature)
        };
        let known = self.refs.signers(account);
        if known.iter().any(|known| known.verified && made(known.key)) {
            return Ok(Signer::Verified);
        }
        if known.iter().any(|known| known.verified) {
            // A contact's new phrase reaches this device with its next look at the contacts.
            self.refresh_contacts().await?;
            let known = self.refs.signers(account);
            return Ok(
                match known.iter().any(|known| known.verified && made(known.key)) {
                    true => Signer::Verified,
                    false => Signer::Bad,
                },
            );
        }
        if known.iter().any(|known| made(known.key)) {
            return Ok(Signer::Unverified);
        }
        let served = client::keys::get_signing_keys(&self.connection(), vec![account])
            .await??
            .into_iter()
            .find(|served| served.account == account)
            .map(|served| served.key);
        let Some(served) = served.filter(|served| !known.iter().any(|known| known.key == *served))
        else {
            return Ok(Signer::Bad);
        };
        if known.is_empty() {
            self.refs.add_signer(account, served, false)?;
            return Ok(match made(served) {
                true => Signer::Unverified,
                false => Signer::Bad,
            });
        }
        Ok(match made(served) {
            true => Signer::Changed,
            false => Signer::Bad,
        })
    }
}
