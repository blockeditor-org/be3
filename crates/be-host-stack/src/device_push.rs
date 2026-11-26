//! A push notification as this device shows it (guides/blocks/client.md). The web build's
//! browser decrypts a push and hands the page a `PushMessage` (be-host's `web_push`); a device
//! with no browser to do that (Android's UnifiedPush receiver) is handed the RFC 8291 body
//! itself, decrypts it with the keys it keeps in the host state, and opens it here, whether the
//! stack is running in this process or the app is closed.

use uuid::Uuid;

/// What a notification naming nothing says: the push could not be opened, or named nothing
/// this device knows.
pub const GENERIC_TITLE: &str = "New message";

/// A notification to show, and the block pressing it opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub title: String,
    pub body: String,
    /// The organization and block it is about; `None` for one naming nothing.
    pub opens: Option<(Uuid, Uuid)>,
}

impl Shown {
    pub fn generic() -> Self {
        Self {
            title: GENERIC_TITLE.to_owned(),
            body: String::new(),
            opens: None,
        }
    }

    /// The notification for a push this device opened: titled with the block's name and the
    /// text said by the sender's, both as this device knows them (`sender` from its contacts,
    /// `block` from its graph), never from what the sender wrote.
    pub fn opened(
        opened: be_block_client::OpenedNotification,
        sender: Option<String>,
        block: Option<String>,
    ) -> Self {
        Self {
            title: block.unwrap_or_else(|| GENERIC_TITLE.to_owned()),
            body: match sender {
                Some(sender) => format!("{sender}: {}", opened.text),
                None => opened.text,
            },
            opens: Some((opened.organization, opened.block)),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use receiving::*;

#[cfg(not(target_arch = "wasm32"))]
mod receiving {
    use std::path::Path;
    use std::time::Duration;

    use be_block_client::{Credentials, OpenedNotification, Peer, PeerConfig};
    use be_block_protocol::{PushEndpoint, PushMessage, PushService};
    use be_host_base::host_state::{HostStateStore, ServerLocation};
    use be_lib_web_push::DeviceKeys;

    use super::Shown;

    /// How long a running stack may take to open a push before it is opened from the refs
    /// database instead: well within the seconds a receiver is given.
    const STACK_PATIENCE: Duration = Duration::from_secs(3);

    /// This device's push keys, made and kept the first time.
    pub fn keys(state: &HostStateStore) -> Result<DeviceKeys, String> {
        let failed = |error: be_host_base::host_state::HostStateError| error.to_string();
        if state.push_keys().map_err(failed)?.is_none() {
            let mut made = DeviceKeys::generate().to_bytes();
            let kept = state.keep_push_keys(&made);
            be_lib_web_push::forget(&mut made);
            kept.map_err(failed)?;
        }
        let mut kept = state
            .push_keys()
            .map_err(failed)?
            .ok_or_else(|| "the push keys were not kept".to_owned())?;
        let keys = DeviceKeys::from_bytes(&kept);
        be_lib_web_push::forget(&mut kept);
        keys.ok_or_else(|| "the kept push keys are corrupt".to_owned())
    }

    /// The endpoint to register with the server for each organization the host opens: the one
    /// the distributor gave, while pushes are on here.
    pub fn registration(state: &HostStateStore) -> Result<Option<PushEndpoint>, String> {
        let Some(address) = state.push_endpoint().map_err(|error| error.to_string())? else {
            return Ok(None);
        };
        Ok(Some(endpoint(&keys(state)?, address)))
    }

    /// Turning pushes off here: forgets the endpoint, so no organization registers it again,
    /// and answers its address, for the server to forget where it can now.
    pub fn turn_off(state: &HostStateStore) -> Result<Option<String>, String> {
        let address = state.push_endpoint().map_err(|error| error.to_string())?;
        state
            .set_push_endpoint(None)
            .map_err(|error| error.to_string())?;
        Ok(address)
    }

    /// The endpoint a distributor gave this device, as the server registers it: a Web Push URL
    /// with this device's keys.
    pub fn endpoint(keys: &DeviceKeys, address: String) -> PushEndpoint {
        PushEndpoint {
            service: PushService::WebPush,
            address,
            p256dh: keys.public(),
            auth: keys.auth().to_vec(),
        }
    }

    /// Turns the body of a push the distributor handed this device into the notification to
    /// show, or `None` for a push to drop: one that does not decrypt to a `PushMessage` with
    /// this device's keys, as a browser drops it, or one already shown, which the server can
    /// send again within `MAX_NOTIFICATION_AGE_MILLISECONDS`. A push no saved account here can
    /// open shows `Shown::generic`. `data_dir` holds the refs database
    /// (`refs_in`); `now` is in Unix milliseconds.
    pub fn receive(
        state: &HostStateStore,
        data_dir: &Path,
        body: &[u8],
        now: i64,
    ) -> Option<Shown> {
        let mut kept = state.push_keys().ok().flatten()?;
        let keys = DeviceKeys::from_bytes(&kept);
        be_lib_web_push::forget(&mut kept);
        let message = PushMessage::decode(&keys?.open(body)?)?;
        let digest: [u8; 32] = <sha2::Sha256 as sha2::Digest>::digest(&message.sealed).into();
        let forget_before = now.saturating_sub(be_block_client::MAX_NOTIFICATION_AGE_MILLISECONDS);
        // A store that cannot be read shows it: it may show twice.
        if !state
            .first_showing(&digest, now, forget_before)
            .unwrap_or(true)
        {
            return None;
        }
        Some(
            from_running_stack(&message, now)
                .or_else(|| from_refs(state, data_dir, &message, now))
                .unwrap_or_else(Shown::generic),
        )
    }

    /// Opens the push through the stack when it runs in this process in the push's
    /// organization, naming it from the stack's contacts and graph.
    fn from_running_stack(message: &PushMessage, now: i64) -> Option<Shown> {
        if crate::running_organization()? != message.organization {
            return None;
        }
        let (reply, answer) = std::sync::mpsc::channel();
        crate::open_push(message.clone(), now, move |opened| {
            let _ = reply.send(opened);
        });
        let opened = crate::wait_for_answer(&answer, STACK_PATIENCE)?.ok()?;
        let sender = crate::contacts()
            .into_iter()
            .find(|contact| contact.account == opened.sender)
            .map(|contact| contact.display_name);
        let block = crate::node(opened.block).and_then(|node| node.metadata.name());
        Some(Shown::opened(opened, sender, block))
    }

    /// Opens the push with no stack and no network: a peer built from the refs database for
    /// each account saved here that has kept the push's organization, with an object store of
    /// its own, since opening reads only the keys and summaries the refs database holds. The
    /// database is the one handle the process shares (`refs_in`), so a stack that starts
    /// meanwhile reads and writes through it too.
    fn from_refs(
        state: &HostStateStore,
        data_dir: &Path,
        message: &PushMessage,
        now: i64,
    ) -> Option<Shown> {
        let refs = crate::refs_in(data_dir).ok()?;
        for account in state.accounts().ok()? {
            // An account on the server this device embeds has none to push while it is
            // closed.
            let ServerLocation::Remote(server) = &account.server else {
                continue;
            };
            let url = crate::accounts::socket_url(server);
            if !refs
                .knows(&url, account.id, message.organization)
                .unwrap_or(false)
            {
                continue;
            }
            let Some(identity) = state.identity(&account).ok().flatten() else {
                continue;
            };
            let config = PeerConfig::new(
                url,
                be_block_keys::RecoverySecret::from_bytes(identity),
                Credentials::Token(account.token.clone()),
            )
            .organization(Some(message.organization))
            .refs(refs.clone());
            let Ok(peer) = Peer::cached(config, account.id, be_block_store::MemoryStore::new())
            else {
                continue;
            };
            let Ok(opened) = peer.open_notification(message, now) else {
                continue;
            };
            return Some(named(&peer, opened));
        }
        None
    }

    fn named<S: be_block_store::ObjectStore>(peer: &Peer<S>, opened: OpenedNotification) -> Shown {
        let sender = peer.contact_name(opened.sender);
        let block = peer.kept_name(opened.block);
        Shown::opened(opened, sender, block)
    }
}
