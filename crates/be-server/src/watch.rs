use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::atomic::{AtomicU64, Ordering},
};

use be_block_protocol::{ClientId, OrganizationRole, PairingMessage, SessionState};
use tokio::sync::{
    Mutex,
    mpsc::{Sender, error::TrySendError},
};

use crate::{Identity, ServerError, limits::MAX_FOLLOWS_PER_CONNECTION};
use uuid::Uuid;

/// A block in the organization that holds it. Live sessions are keyed by both: block
/// ids are chosen by clients, so another organization can hold a block with the same id, and the
/// two must never hear of each other.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Place {
    pub organization: Uuid,
    pub block: Uuid,
}

#[derive(Clone, Debug)]
pub enum Push {
    /// A tree the connection follows advanced (`tree-advanced`).
    TreeAdvanced {
        entry: Uuid,
        sequence: u64,
    },
    /// The account's roots may have changed (`tree-change.roots`).
    Roots,
    SessionChanged {
        block: Place,
        state: SessionState,
    },
    Relayed {
        block: Place,
        from: ClientId,
        /// What the server checked the sender may send.
        kind: be_block_protocol::RelayKind,
        /// Shared by every participant's queue, so a relay to many costs one copy.
        payload: std::sync::Arc<[u8]>,
        received: i64,
    },
    /// The order numbered a submission: every participant hears it, its author too.
    Sequenced {
        block: Place,
        entry: std::sync::Arc<be_block_protocol::JournalEntry>,
    },
    Paired(PairingMessage),
    ContactsChanged,
    /// The account's notices changed (`store/account_notices.rs`).
    AccountNoticesChanged,
    KeyWorkChanged,
    /// Keys the account may open were stored (`keys-changed`).
    KeysChanged,
    /// The account's notification modes changed through another of its connections.
    NotifyModesChanged,
}

#[derive(Default)]
struct Joined {
    accounts: HashMap<u64, Uuid>,
    by_account: HashMap<Uuid, HashSet<u64>>,
    organizations: HashMap<u64, Identity>,
    by_organization: HashMap<Uuid, HashSet<u64>>,
    by_member: HashMap<(Uuid, Uuid), HashSet<u64>>,
    administrators: HashMap<Uuid, HashSet<u64>>,
    /// The entry points each connection follows, in the organization it has open.
    follows: HashMap<u64, HashMap<Uuid, Told>>,
    /// The connections following each entry point.
    followers: HashMap<Place, HashSet<u64>>,
}

/// What a connection was told of an entry point it follows. Once it is told the tree advanced,
/// it is told nothing more of it until it reads the tree's changes to their end, so however
/// many changes come in a burst it hears of them once, then once more for those after its read.
#[derive(Clone, Copy, Debug, Default)]
struct Told {
    /// Told, or reading, and not yet read to the end.
    outstanding: bool,
    /// The newest sequence the tree advanced to while outstanding.
    latest: Option<u64>,
}

fn unindex<K: Eq + std::hash::Hash>(index: &mut HashMap<K, HashSet<u64>>, key: K, client: u64) {
    if let Some(clients) = index.get_mut(&key) {
        clients.remove(&client);
        if clients.is_empty() {
            index.remove(&key);
        }
    }
}

impl Joined {
    fn leave_account(&mut self, client: u64) {
        if let Some(account) = self.accounts.remove(&client) {
            unindex(&mut self.by_account, account, client);
        }
        self.leave_organization(client);
    }

    fn leave_organization(&mut self, client: u64) {
        let follows = self.follows.remove(&client).unwrap_or_default();
        if let Some(identity) = self.organizations.remove(&client) {
            for entry in follows.into_keys() {
                let place = Place {
                    organization: identity.organization,
                    block: entry,
                };
                unindex(&mut self.followers, place, client);
            }
            unindex(&mut self.by_organization, identity.organization, client);
            unindex(
                &mut self.by_member,
                (identity.organization, identity.account),
                client,
            );
            unindex(&mut self.administrators, identity.organization, client);
        }
    }
}

#[derive(Default)]
pub struct WatchHub {
    next: AtomicU64,
    clients: Mutex<HashMap<u64, Sender<Push>>>,
    joined: Mutex<Joined>,
    /// The one connection each block's agent signed in on last; any other of its connections
    /// fails its next sign-in check and closes (`Connection::signed_in`).
    live: std::sync::Mutex<HashMap<Uuid, u64>>,
}

impl WatchHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes `client` the live connection of the block's agent `account`; answers the one it
    /// replaced.
    pub fn go_live(&self, account: Uuid, client: u64) -> Option<u64> {
        let mut live = self
            .live
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        live.insert(account, client)
            .filter(|replaced| *replaced != client)
    }

    pub fn is_live(&self, account: Uuid, client: u64) -> bool {
        let live = self
            .live
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        live.get(&account) == Some(&client)
    }

    pub fn leave_live(&self, account: Uuid, client: u64) {
        let mut live = self
            .live
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if live.get(&account) == Some(&client) {
            live.remove(&account);
        }
    }

    pub async fn register(&self, sender: Sender<Push>) -> u64 {
        let client = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.clients.lock().await.insert(client, sender);
        client
    }

    /// Follows `entry` in the organization `client` has open, before the read that starts it,
    /// so nothing changed during the read is missed. Refused once the connection follows
    /// `MAX_FOLLOWS_PER_CONNECTION`.
    pub async fn follow(&self, client: u64, entry: Uuid) -> Result<(), ServerError> {
        let mut joined = self.joined.lock().await;
        let Some(identity) = joined.organizations.get(&client).copied() else {
            return Ok(());
        };
        let follows = joined.follows.entry(client).or_default();
        if !follows.contains_key(&entry) && follows.len() >= MAX_FOLLOWS_PER_CONNECTION {
            return Err(crate::limits::too_many(
                "entry points one connection follows; unfollow some first",
                MAX_FOLLOWS_PER_CONNECTION,
            ));
        }
        follows.entry(entry).or_default().outstanding = true;
        let place = Place {
            organization: identity.organization,
            block: entry,
        };
        joined.followers.entry(place).or_default().insert(client);
        Ok(())
    }

    pub async fn unfollow(&self, client: u64, entry: Uuid) {
        let mut joined = self.joined.lock().await;
        let Some(identity) = joined.organizations.get(&client).copied() else {
            return;
        };
        if let Some(follows) = joined.follows.get_mut(&client) {
            follows.remove(&entry);
        }
        let place = Place {
            organization: identity.organization,
            block: entry,
        };
        unindex(&mut joined.followers, place, client);
    }

    /// The connections following any of `blocks` in `organization`, with the block each
    /// follows: those a change on the path of `blocks` may have to tell.
    pub async fn followers(
        &self,
        organization: Uuid,
        blocks: &[Uuid],
    ) -> Vec<(u64, Identity, Uuid)> {
        let joined = self.joined.lock().await;
        let mut found = Vec::new();
        for block in blocks {
            let place = Place {
                organization,
                block: *block,
            };
            for client in joined.followers.get(&place).into_iter().flatten() {
                if let Some(identity) = joined.organizations.get(client) {
                    found.push((*client, *identity, *block));
                }
            }
        }
        found
    }

    /// Tells `client` that the tree of `entry` advanced to `sequence`, unless it was told
    /// already and has not read it to the end since (`Told`).
    pub async fn advance(&self, client: u64, entry: Uuid, sequence: u64) {
        let send = {
            let mut joined = self.joined.lock().await;
            let Some(told) = joined
                .follows
                .get_mut(&client)
                .and_then(|follows| follows.get_mut(&entry))
            else {
                return;
            };
            match told.outstanding {
                true => {
                    told.latest = Some(told.latest.unwrap_or_default().max(sequence));
                    false
                }
                false => {
                    told.outstanding = true;
                    true
                }
            }
        };
        if send {
            self.send_to(client, Push::TreeAdvanced { entry, sequence })
                .await;
        }
    }

    /// `client` read the tree of `entry` to its end, as of `sequence`: told of anything since,
    /// and otherwise told again of the next change.
    pub async fn read(&self, client: u64, entry: Uuid, sequence: u64) {
        let send = {
            let mut joined = self.joined.lock().await;
            let Some(told) = joined
                .follows
                .get_mut(&client)
                .and_then(|follows| follows.get_mut(&entry))
            else {
                return;
            };
            match told.latest.take().filter(|latest| *latest > sequence) {
                Some(latest) => Some(latest),
                None => {
                    told.outstanding = false;
                    None
                }
            }
        };
        if let Some(sequence) = send {
            self.send_to(client, Push::TreeAdvanced { entry, sequence })
                .await;
        }
    }

    /// Tells the connections of `accounts` in `organization`, and its administrators', that
    /// their roots may have changed.
    pub async fn roots(&self, organization: Uuid, accounts: &BTreeSet<Uuid>) {
        let clients: Vec<u64> = self
            .audience(organization, accounts)
            .await
            .into_iter()
            .map(|(client, _)| client)
            .collect();
        self.send_every(&clients, &Push::Roots).await;
    }

    pub async fn join_account(&self, client: u64, account: Uuid) {
        let mut joined = self.joined.lock().await;
        if let Some(previous) = joined.accounts.insert(client, account) {
            unindex(&mut joined.by_account, previous, client);
        }
        joined.by_account.entry(account).or_default().insert(client);
    }

    pub async fn leave_account(&self, client: u64) {
        self.joined.lock().await.leave_account(client);
    }

    pub async fn send_account(
        &self,
        account: Uuid,
        from: u64,
        to: Option<u64>,
        message: &Push,
    ) -> usize {
        let targets: Vec<u64> = self
            .joined
            .lock()
            .await
            .by_account
            .get(&account)
            .into_iter()
            .flatten()
            .copied()
            .filter(|client| *client != from && to.is_none_or(|to| to == *client))
            .collect();
        let mut clients = self.clients.lock().await;
        let mut sent = 0;
        for target in targets {
            if deliver(&mut clients, target, message.clone()) {
                sent += 1;
            }
        }
        sent
    }

    pub async fn join_organization(&self, client: u64, identity: Identity) {
        let mut joined = self.joined.lock().await;
        joined.leave_organization(client);
        joined.organizations.insert(client, identity);
        joined
            .by_organization
            .entry(identity.organization)
            .or_default()
            .insert(client);
        joined
            .by_member
            .entry((identity.organization, identity.account))
            .or_default()
            .insert(client);
        if identity.role == OrganizationRole::Administrator {
            joined
                .administrators
                .entry(identity.organization)
                .or_default()
                .insert(client);
        }
    }

    pub async fn audience(
        &self,
        organization: Uuid,
        accounts: &BTreeSet<Uuid>,
    ) -> Vec<(u64, Identity)> {
        let joined = self.joined.lock().await;
        let mut clients: BTreeSet<u64> = joined
            .administrators
            .get(&organization)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        for account in accounts {
            clients.extend(
                joined
                    .by_member
                    .get(&(organization, *account))
                    .into_iter()
                    .flatten()
                    .copied(),
            );
        }
        clients
            .into_iter()
            .filter_map(|client| {
                joined
                    .organizations
                    .get(&client)
                    .map(|identity| (client, *identity))
            })
            .collect()
    }

    pub async fn identity(&self, client: u64) -> Option<Identity> {
        self.joined.lock().await.organizations.get(&client).copied()
    }

    pub async fn clients_of(&self, organization: Uuid, account: Uuid) -> Vec<(u64, Identity)> {
        let joined = self.joined.lock().await;
        joined
            .by_member
            .get(&(organization, account))
            .into_iter()
            .flatten()
            .filter_map(|client| {
                joined
                    .organizations
                    .get(client)
                    .map(|identity| (*client, *identity))
            })
            .collect()
    }

    /// Which of `accounts` have a live connection to `organization`: one that has joined it
    /// (`go_live`), not merely an open socket.
    pub async fn connected(&self, organization: Uuid, accounts: &[Uuid]) -> HashSet<Uuid> {
        let joined = self.joined.lock().await;
        accounts
            .iter()
            .filter(|account| {
                joined
                    .by_member
                    .get(&(organization, **account))
                    .is_some_and(|clients| !clients.is_empty())
            })
            .copied()
            .collect()
    }

    pub async fn drop_member(&self, organization: Uuid, account: Uuid) {
        let clients: Vec<u64> = {
            let mut joined = self.joined.lock().await;
            let clients: Vec<u64> = joined
                .by_member
                .get(&(organization, account))
                .into_iter()
                .flatten()
                .copied()
                .collect();
            for client in &clients {
                joined.leave_organization(*client);
            }
            clients
        };
        let mut senders = self.clients.lock().await;
        for client in clients {
            senders.remove(&client);
        }
    }

    pub async fn announce(&self, organization: Uuid, message: Push) {
        let targets: Vec<u64> = self
            .joined
            .lock()
            .await
            .by_organization
            .get(&organization)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        let mut clients = self.clients.lock().await;
        for target in targets {
            deliver(&mut clients, target, message.clone());
        }
    }

    pub async fn remove(&self, client: u64) {
        {
            let mut joined = self.joined.lock().await;
            joined.leave_account(client);
        }
        self.clients.lock().await.remove(&client);
    }

    pub async fn send_to(&self, client: u64, message: Push) {
        deliver(&mut *self.clients.lock().await, client, message);
    }

    /// Sends `message` to each of `clients`, the sender among them.
    pub async fn send_every(&self, clients: &[u64], message: &Push) {
        let mut senders = self.clients.lock().await;
        for client in clients {
            deliver(&mut senders, *client, message.clone());
        }
    }

    pub async fn send_all(&self, clients: &[u64], from: u64, message: &Push) {
        let mut senders = self.clients.lock().await;
        for client in clients {
            if *client != from {
                deliver(&mut senders, *client, message.clone());
            }
        }
    }
}

fn deliver(clients: &mut HashMap<u64, Sender<Push>>, client: u64, message: Push) -> bool {
    let Some(sender) = clients.get(&client) else {
        return false;
    };
    match sender.try_send(message) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) => {
            tracing::warn!(
                client,
                "dropping a connection that is not reading what it is sent"
            );
            clients.remove(&client);
            false
        }
        Err(TrySendError::Closed(_)) => false,
    }
}
