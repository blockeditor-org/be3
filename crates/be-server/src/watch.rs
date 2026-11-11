use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::atomic::{AtomicU64, Ordering},
};

use be_block_commit::CommitId;
use be_block_graph::BlockParent;
use be_block_protocol::{
    BlockSummary, ClientId, Listing, OrganizationRole, PairingMessage, SessionState,
};
use tokio::sync::{
    Mutex,
    mpsc::{Sender, error::TrySendError},
};

use crate::{Identity, ServerError, limits::MAX_LISTINGS_PER_CONNECTION};
use uuid::Uuid;

/// A block in the organization that holds it. Watches and live sessions are keyed by both: block
/// ids are chosen by clients, so another organization can hold a block with the same id, and the
/// two must never hear of each other.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Place {
    pub organization: Uuid,
    pub block: Uuid,
}

/// Listings a connection keeps after they were refused, so a block read before it is made is
/// announced once it is. Past this many, a refused listing is dropped at once.
pub const REFUSED_LISTINGS: usize = 256;

#[derive(Clone, Debug)]
pub enum Push {
    BlockChanged(BlockSummary),
    BlockRemoved {
        block: Uuid,
        revision: u64,
    },
    HeadChanged {
        block: Place,
        head: CommitId,
        author: Uuid,
    },
    BlockDeleted {
        block: Place,
    },
    Unwatched {
        block: Place,
    },
    SessionChanged {
        block: Place,
        state: SessionState,
    },
    Relayed {
        block: Place,
        from: ClientId,
        /// Shared by every participant's queue, so a relay to many costs one copy.
        payload: std::sync::Arc<[u8]>,
    },
    Paired(PairingMessage),
    ContactsChanged,
    KeyWorkChanged,
}

#[derive(Default)]
struct Joined {
    accounts: HashMap<u64, Uuid>,
    by_account: HashMap<Uuid, HashSet<u64>>,
    organizations: HashMap<u64, Identity>,
    by_organization: HashMap<Uuid, HashSet<u64>>,
    by_member: HashMap<(Uuid, Uuid), HashSet<u64>>,
    administrators: HashMap<Uuid, HashSet<u64>>,
    watchers: HashMap<Place, HashSet<u64>>,
    watching: HashMap<u64, HashSet<Place>>,
    interests: HashMap<u64, Interest>,
}

struct Facts {
    parent: Option<BlockParent>,
    held: bool,
    entry: bool,
    references: Vec<Uuid>,
    opens_inside: bool,
}

impl Facts {
    fn of(summary: &BlockSummary, entry: bool) -> Self {
        Self {
            parent: summary.parent,
            held: summary.held,
            entry,
            references: summary.references.clone(),
            opens_inside: summary.opens_inside,
        }
    }
}

#[derive(Default)]
struct Interest {
    listings: HashSet<Listing>,
    refused: HashSet<Listing>,
    /// The `Listing::References` and `Listing::OpenSettings` among `listings`, kept apart so
    /// matching a block costs a few lookups rather than a pass over every listing.
    references: HashSet<Uuid>,
    open_settings: usize,
    /// The listings that count against `MAX_LISTINGS_PER_CONNECTION`: every one but a block's
    /// own (`read-block`), which a client never unlists. Those are bounded instead by the blocks
    /// the caller may know of, and refused ones by `REFUSED_LISTINGS`.
    counted: usize,
    known: HashMap<Uuid, Facts>,
}

fn counts(listing: Listing) -> bool {
    !matches!(listing, Listing::Block(_))
}

impl Interest {
    /// Adds a listing and answers whether it is new, or `None` when the connection already
    /// holds `MAX_LISTINGS_PER_CONNECTION`.
    fn list(&mut self, listing: Listing) -> Option<bool> {
        if self.listings.contains(&listing) {
            return Some(false);
        }
        if counts(listing) {
            if self.counted >= MAX_LISTINGS_PER_CONNECTION {
                return None;
            }
            self.counted += 1;
        }
        self.listings.insert(listing);
        match listing {
            Listing::References(referrer) => {
                self.references.insert(referrer);
            }
            Listing::OpenSettings(_) => self.open_settings += 1,
            _ => {}
        }
        Some(true)
    }

    fn unlist(&mut self, listing: Listing) {
        if !self.listings.remove(&listing) {
            return;
        }
        if counts(listing) {
            self.counted -= 1;
        }
        match listing {
            Listing::References(referrer) => {
                self.references.remove(&referrer);
            }
            Listing::OpenSettings(_) => self.open_settings -= 1,
            _ => {}
        }
    }

    fn matches(&self, block: Uuid, facts: &Facts) -> bool {
        self.listings.contains(&Listing::Block(block))
            || facts
                .parent
                .is_some_and(|parent| self.listings.contains(&Listing::Children(parent)))
            || (facts.parent.is_some() && !facts.held && self.listings.contains(&Listing::Deleted))
            || (facts.entry && self.listings.contains(&Listing::Shared))
            || (facts.opens_inside && self.open_settings > 0)
            || facts
                .references
                .iter()
                .any(|target| self.listings.contains(&Listing::Backrefs(*target)))
            || self.references.iter().any(|referrer| {
                self.known
                    .get(referrer)
                    .is_some_and(|referrer| referrer.references.contains(&block))
            })
    }

    fn told(&mut self, block: Uuid, facts: Facts) -> bool {
        if self.matches(block, &facts) {
            self.known.insert(block, facts);
            return true;
        }
        self.known.remove(&block).is_some()
    }
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
        self.interests.remove(&client);
        if let Some(identity) = self.organizations.remove(&client) {
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

    pub async fn watch(&self, block: Place, client: u64) {
        let mut joined = self.joined.lock().await;
        joined.watchers.entry(block).or_default().insert(client);
        joined.watching.entry(client).or_default().insert(block);
    }

    pub async fn unwatch(&self, block: Place, client: u64) {
        let mut joined = self.joined.lock().await;
        unindex(&mut joined.watchers, block, client);
        if let Some(blocks) = joined.watching.get_mut(&client) {
            blocks.remove(&block);
            if blocks.is_empty() {
                joined.watching.remove(&client);
            }
        }
    }

    /// Registers a listing before it is read, so nothing changed during the read is missed;
    /// answers whether the connection did not already hold it, and refuses it once the
    /// connection holds `MAX_LISTINGS_PER_CONNECTION`.
    pub async fn list(&self, client: u64, listing: Listing) -> Result<bool, ServerError> {
        let mut joined = self.joined.lock().await;
        let interest = joined.interests.entry(client).or_default();
        interest.list(listing).ok_or_else(|| {
            crate::limits::too_many(
                "listings one connection holds; unlist some first",
                MAX_LISTINGS_PER_CONNECTION,
            )
        })
    }

    /// A listing the store refused. One the connection held before stays as it was; a new one is
    /// kept, and counted, up to `REFUSED_LISTINGS`.
    pub async fn refused(&self, client: u64, listing: Listing, new: bool) {
        let mut joined = self.joined.lock().await;
        let interest = joined.interests.entry(client).or_default();
        if !new {
            return;
        }
        match interest.refused.len() < REFUSED_LISTINGS {
            true => {
                interest.refused.insert(listing);
            }
            false => {
                interest.unlist(listing);
            }
        }
    }

    pub async fn listed(&self, client: u64, listing: Listing, blocks: &[BlockSummary]) {
        let mut joined = self.joined.lock().await;
        let interest = joined.interests.entry(client).or_default();
        interest.refused.remove(&listing);
        for summary in blocks {
            interest.told(summary.id, Facts::of(summary, listing == Listing::Shared));
        }
    }

    pub async fn unlist(&self, client: u64, listing: Listing) {
        let mut joined = self.joined.lock().await;
        let Some(interest) = joined.interests.get_mut(&client) else {
            return;
        };
        interest.unlist(listing);
        interest.refused.remove(&listing);
        let known = std::mem::take(&mut interest.known);
        interest.known = known
            .into_iter()
            .filter(|(block, facts)| interest.matches(*block, facts))
            .collect();
    }

    pub async fn tell(&self, client: u64, message: Push, entry: bool) {
        let told = {
            let mut joined = self.joined.lock().await;
            let interest = joined.interests.entry(client).or_default();
            match &message {
                Push::BlockChanged(block) => interest.told(block.id, Facts::of(block, entry)),
                Push::BlockRemoved { block, .. } => interest.known.remove(block).is_some(),
                _ => true,
            }
        };
        if told {
            self.send_to(client, message).await;
        }
    }

    pub async fn removed(&self, organization: Uuid, block: Uuid, revision: u64) {
        let targets: Vec<u64> = {
            let mut joined = self.joined.lock().await;
            let clients: Vec<u64> = joined
                .by_organization
                .get(&organization)
                .into_iter()
                .flatten()
                .copied()
                .collect();
            clients
                .into_iter()
                .filter(|client| {
                    joined
                        .interests
                        .get_mut(client)
                        .is_some_and(|interest| interest.known.remove(&block).is_some())
                })
                .collect()
        };
        let mut clients = self.clients.lock().await;
        for target in targets {
            deliver(&mut clients, target, Push::BlockRemoved { block, revision });
        }
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
            joined.interests.remove(&client);
            for block in joined.watching.remove(&client).unwrap_or_default() {
                unindex(&mut joined.watchers, block, client);
            }
        }
        self.clients.lock().await.remove(&client);
    }

    pub async fn send_to(&self, client: u64, message: Push) {
        deliver(&mut *self.clients.lock().await, client, message);
    }

    pub async fn send_all(&self, clients: &[u64], from: u64, message: &Push) {
        let mut senders = self.clients.lock().await;
        for client in clients {
            if *client != from {
                deliver(&mut senders, *client, message.clone());
            }
        }
    }

    pub async fn broadcast(&self, block: Place, from: u64, message: Push) {
        let targets: Vec<u64> = self
            .joined
            .lock()
            .await
            .watchers
            .get(&block)
            .into_iter()
            .flatten()
            .copied()
            .filter(|client| *client != from)
            .collect();
        let mut clients = self.clients.lock().await;
        for target in targets {
            deliver(&mut clients, target, message.clone());
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
