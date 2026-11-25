//! Signing what the account writes, and telling who signed what others wrote
//! (guides/blocks/keys.md, "Signing what a person writes").

use std::collections::HashSet;
use std::sync::{Arc, Weak};

use be_block_content::Signer;
use be_block_keys::SigningPublic;
use be_block_store::ObjectStore;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::Peer;
use crate::{ClientError, Connection};

/// Records found forged after asking the server (`Peer::verify_authored`), while the
/// connection that answered lasts: a forged row shown again asks nothing.
#[derive(Default)]
pub(super) struct Forged {
    connection: Weak<Connection>,
    records: HashSet<[u8; 32]>,
}

impl Forged {
    fn holds(&mut self, connection: &Arc<Connection>, record: &[u8; 32]) -> bool {
        if !std::ptr::eq(self.connection.as_ptr(), Arc::as_ptr(connection)) {
            self.connection = Arc::downgrade(connection);
            self.records.clear();
        }
        self.records.contains(record)
    }

    fn insert(&mut self, connection: &Arc<Connection>, record: [u8; 32]) {
        if self.holds(connection, &record) {
            return;
        }
        // Bounded: past the cap it starts over, and a forged row costs one round trip again.
        if self.records.len() >= MAX_FORGED {
            self.records.clear();
        }
        self.records.insert(record);
    }

    /// Forgets every record: a key this device learned since may have made one.
    fn clear(&mut self) {
        self.records.clear();
    }
}

/// The most records `Forged` remembers.
const MAX_FORGED: usize = 4096;

/// How far this device's estimate of the server's clock (`Peer::server_now`), by which it marks
/// when it learned a key was replaced, may be off from the stamps the server gives arrivals.
const SERVER_CLOCK_MARGIN_MILLISECONDS: i64 = 60 * 1000;

/// Who of an account's verified keys made a signature, as `made` tells: the current one, or a
/// replaced one, which answers `Replaced` for a record the server took (`arrived`) after this
/// device learned of the key that replaced it.
fn by_verified(
    verified: &[&crate::refs::KnownSigner],
    made: impl Fn([u8; 32]) -> bool,
    arrived: Option<i64>,
) -> Option<Signer> {
    if verified
        .last()
        .is_some_and(|current| !current.superseded && made(current.key))
    {
        return Some(Signer::Verified);
    }
    let old = verified.iter().find(|old| made(old.key))?;
    let after = matches!(
        (arrived, old.replaced),
        (Some(arrived), Some(replaced))
            if arrived > replaced.saturating_add(SERVER_CLOCK_MARGIN_MILLISECONDS)
    );
    Some(match after {
        true => Signer::Replaced,
        false => Signer::Changed,
    })
}

/// A record's identity for `Forged`: who it names, where, what it says and its signature.
fn record(account: Uuid, block: Uuid, bytes: &[u8], signature: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(account.as_bytes());
    hasher.update(block.as_bytes());
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
    hasher.update(signature);
    hasher.finalize().into()
}

/// `bytes` an app wrote, framed by the app's id ahead of them, as the host signs and verifies
/// what apps write (guides/blocks/keys.md, "Signing what a person writes"): a signature over
/// them verifies only for the same app.
pub fn by_app(app: &str, bytes: &[u8]) -> Vec<u8> {
    let mut bound = (app.len() as u64).to_be_bytes().to_vec();
    bound.extend_from_slice(app.as_bytes());
    bound.extend_from_slice(bytes);
    bound
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
    /// is pinned the first time (trust on first use). `arrived` is when the server took the
    /// record, if it carries that stamp: one signed with a contact's replaced key that arrived
    /// after this device learned of the new key is `Replaced`, not `Changed`.
    pub async fn verify_authored(
        &self,
        account: Uuid,
        block: Uuid,
        bytes: &[u8],
        signature: &[u8],
        arrived: Option<i64>,
    ) -> Result<Signer, ClientError> {
        let made = |key: [u8; 32]| {
            SigningPublic(key).verifies_authored(block.into_bytes(), bytes, signature)
        };
        let known = self.refs.signers(account);
        // The newest verified key is the account's own; one it replaced since still verifies
        // what it signed, but is shown as changed, since someone holding the old phrase can
        // sign with it too.
        let verified: Vec<_> = known.iter().filter(|known| known.verified).collect();
        if let Some(signer) = by_verified(&verified, made, arrived) {
            return Ok(signer);
        }
        let contact = !verified.is_empty();
        if !contact && known.iter().any(|known| made(known.key)) {
            return Ok(Signer::Unverified);
        }
        let connection = self.connection();
        let asked = record(account, block, bytes, signature);
        if self.forged.lock().unwrap().holds(&connection, &asked) {
            return Ok(Signer::Bad);
        }
        if !self.is_attached() {
            return Err(ClientError::Disconnected(
                "who signed this is known once the server is reachable".to_owned(),
            ));
        }
        let found = self
            .ask_about(account, block, bytes, signature, contact, arrived)
            .await?;
        let mut forged = self.forged.lock().unwrap();
        // A key learned while asking (a contact's new phrase, a key pinned) may have made a
        // record found forged before.
        if self.refs.signers(account) != known {
            forged.clear();
        }
        if found == Signer::Bad {
            forged.insert(&connection, asked);
        }
        Ok(found)
    }

    /// `verify_authored` once no key this device holds made the signature: asks the server,
    /// or refreshes the contacts for a contact's new key.
    async fn ask_about(
        &self,
        account: Uuid,
        block: Uuid,
        bytes: &[u8],
        signature: &[u8],
        contact: bool,
        arrived: Option<i64>,
    ) -> Result<Signer, ClientError> {
        let made = |key: [u8; 32]| {
            SigningPublic(key).verifies_authored(block.into_bytes(), bytes, signature)
        };
        let known = self.refs.signers(account);
        if contact {
            // A contact's new phrase reaches this device with its next look at the contacts.
            self.refresh_contacts().await?;
            let known = self.refs.signers(account);
            let verified: Vec<_> = known.iter().filter(|known| known.verified).collect();
            return Ok(by_verified(&verified, made, arrived).unwrap_or(Signer::Bad));
        }
        let served = be_block_protocol::read::<Vec<be_block_protocol::SigningKey>, _>(
            &self.connection(),
            Some(self.organization),
            be_block_protocol::Query::SigningKeys(vec![account]),
        )
        .await??
        .into_iter()
        .find(|served| served.account == account)
        .map(|served| served.key);
        let Some(served) = served.filter(|served| !known.iter().any(|known| known.key == *served))
        else {
            return Ok(Signer::Bad);
        };
        if known.is_empty() {
            self.refs
                .add_signer(account, served, false, self.server_now())?;
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
