use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::atomic::{AtomicU64, Ordering},
};

use be_block_graph::{BlockGraph, BlockParent};
use be_block_protocol::{
    BlockSummary, ClientId, Listing, OrganizationRole, PairingMessage,
};
use tokio::sync::{
    Mutex,
    mpsc::{Sender, error::TrySendError},
};

use crate::{Identity, ServerError, limits::MAX_LISTINGS_PER_CONNECTION};
use uuid::Uuid;

/// A block in the organization that holds it. Live sessions are keyed by both: block
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
    BlockChanged {
        block: BlockSummary,
        /// What the change did to its references, when the server knows (`Moves`): told in
        /// their place to a connection already told them (`WatchHub::tell`), which then
        /// empties `block.references`. Without it, `block.references` go whole
        /// (`changed-references`).
        moved: Option<be_block_protocol::MovedReferences>,
        /// The connection whose call changed it.
        from: ClientId,
    },
    BlockRemoved {
        block: Uuid,
        revision: u64,
    },
    /// The server wrote a membership entry into a session's order.
    Membership {
        block: Place,
        entry: std::sync::Arc<be_block_protocol::MembershipEntry>,
    },
    /// A seal moved a session's block's head to `commit`, holding its order through `through`.
    Sealed {
        block: Place,
        commit: be_block_commit::CommitId,
        through: u64,
    },
    Relayed {
        block: Place,
        from: ClientId,
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
    interests: HashMap<u64, Interest>,
}

/// What a connection was told of a block that decides which of its listings hold it, and
/// nothing the graph answers: not its references, which a folder's may run to
/// `MAX_REFERENCES` (`Links` reads them from the graph instead).
pub(crate) struct Facts {
    parent: Option<BlockParent>,
    held: bool,
    entry: bool,
    /// Told with its references, as a summary the connection may view: so a change may be told
    /// as what it did to them (`moved-references`).
    viewed: bool,
    /// Held by one of the connection's listings through references (`Links::hold`) when told:
    /// what keeps it known when another listing that held it is dropped (`WatchHub::unlist`).
    linked: bool,
    opens_inside: bool,
}

impl Facts {
    /// What a block's summary tells, given apart (`blocks::facts_of` reads it from the graph
    /// without making a summary).
    pub(crate) fn new(
        parent: Option<BlockParent>,
        held: bool,
        Seen { entry, linked }: Seen,
        viewed: bool,
        opens_inside: bool,
    ) -> Self {
        Self {
            parent,
            held,
            entry,
            viewed,
            linked,
            opens_inside,
        }
    }

    pub(crate) fn of(summary: &BlockSummary, seen: Seen) -> Self {
        Self::new(
            summary.parent,
            summary.held,
            seen,
            summary.access.can_view(),
            summary.opens_inside,
        )
    }

    /// The bytes the hub keeps for these facts: the same for every block.
    #[cfg(test)]
    pub(crate) const BYTES: usize = std::mem::size_of::<(Uuid, Self)>();
}

/// What a graph job works out of a block for one connection beside its summary.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Seen {
    /// One of the caller's shared entry points (`Listing::Shared`).
    pub(crate) entry: bool,
    /// Held by one of the connection's listings through references (`Links::hold`).
    pub(crate) linked: bool,
}

/// A connection's listings that hold blocks through references, as the hub had them when a
/// graph job began (`WatchHub::links`): the job reads from the graph which of its blocks they
/// hold, so the hub keeps no block's references.
#[derive(Clone, Debug, Default)]
pub(crate) struct Links {
    /// `Listing::References`: blocks whose references the connection listed.
    referrers: Vec<Uuid>,
    /// `Listing::Backrefs`: blocks whose referrers the connection listed.
    targets: Vec<Uuid>,
    /// `Interest::linking` when read, so `tell` knows a listing came since.
    generation: u64,
}

impl Links {
    /// Whether one of these listings holds `block`, by the references of the blocks `can_view`
    /// allows: a summary the caller may not view tells none.
    pub(crate) fn hold(
        &self,
        graph: &BlockGraph,
        block: Uuid,
        can_view: impl Fn(Uuid) -> bool,
    ) -> bool {
        (self
            .targets
            .iter()
            .any(|target| graph.refers(block, *target))
            && can_view(block))
            || self
                .referrers
                .iter()
                .any(|referrer| graph.refers(*referrer, block) && can_view(*referrer))
    }
}

#[derive(Default)]
struct Interest {
    listings: HashSet<Listing>,
    refused: HashSet<Listing>,
    /// The `Listing::References`, `Listing::Backrefs` and `Listing::OpenSettings` among
    /// `listings`, kept apart so matching a block costs a few lookups rather than a pass over
    /// every listing.
    references: HashSet<Uuid>,
    backrefs: HashSet<Uuid>,
    open_settings: usize,
    /// Counts the `Listing::References` and `Listing::Backrefs` listed, so a change matched
    /// against `Links` read before one was is not taken to have left them (`WatchHub::tell`).
    linking: u64,
    /// The listings that count against `MAX_LISTINGS_PER_CONNECTION`: every one but a block's
    /// own (`Listing::Block`), which a client never unlists. Those are bounded instead by the blocks
    /// the caller may know of, and refused ones by `REFUSED_LISTINGS`.
    counted: usize,
    known: HashMap<Uuid, Facts>,
    /// Listings registered and still being read: until `listed` records what they hold, any
    /// block may be among it, so a change to any block is offered (`WatchHub::knowing`).
    reading: usize,
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
                self.linking += 1;
            }
            Listing::Backrefs(target) => {
                self.backrefs.insert(target);
                self.linking += 1;
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
            Listing::Backrefs(target) => {
                self.backrefs.remove(&target);
            }
            Listing::OpenSettings(_) => self.open_settings -= 1,
            _ => {}
        }
    }

    fn links(&self) -> Links {
        Links {
            referrers: self.references.iter().copied().collect(),
            targets: self.backrefs.iter().copied().collect(),
            generation: self.linking,
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
            || facts.linked
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

    /// Registers a listing before it is read, so nothing changed during the read is missed;
    /// answers whether the connection did not already hold it, and refuses it once the
    /// connection holds `MAX_LISTINGS_PER_CONNECTION`.
    pub async fn list(&self, client: u64, listing: Listing) -> Result<bool, ServerError> {
        let mut joined = self.joined.lock().await;
        let interest = joined.interests.entry(client).or_default();
        let new = interest.list(listing).ok_or_else(|| {
            crate::limits::too_many(
                "listings one connection holds; unlist some first",
                MAX_LISTINGS_PER_CONNECTION,
            )
        })?;
        interest.reading += 1;
        Ok(new)
    }

    /// A listing the store refused. One the connection held before stays as it was; a new one is
    /// kept, and counted, up to `REFUSED_LISTINGS`.
    pub async fn refused(&self, client: u64, listing: Listing, new: bool) {
        let mut joined = self.joined.lock().await;
        let interest = joined.interests.entry(client).or_default();
        interest.reading = interest.reading.saturating_sub(1);
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

    /// A page of a listing was read: `told` is what the connection is told of each of its
    /// blocks.
    pub(crate) async fn listed(&self, client: u64, listing: Listing, told: Vec<(Uuid, Facts)>) {
        let mut joined = self.joined.lock().await;
        let interest = joined.interests.entry(client).or_default();
        interest.reading = interest.reading.saturating_sub(1);
        interest.refused.remove(&listing);
        for (block, mut facts) in told {
            // `Links` read before another listing linked the block may miss it, while a listing
            // that links it says so as it is read, so being linked stays until a change
            // matches the block again (`tell`).
            facts.linked |= interest.known.get(&block).is_some_and(|known| known.linked);
            interest.told(block, facts);
        }
    }

    pub async fn unlist(&self, client: u64, listing: Listing) {
        let mut joined = self.joined.lock().await;
        let Some(interest) = joined.interests.get_mut(&client) else {
            return;
        };
        interest.unlist(listing);
        interest.refused.remove(&listing);
        if interest.references.is_empty() && interest.backrefs.is_empty() {
            for facts in interest.known.values_mut() {
                facts.linked = false;
            }
        }
        let known = std::mem::take(&mut interest.known);
        interest.known = known
            .into_iter()
            .filter(|(block, facts)| interest.matches(*block, facts))
            .collect();
    }

    /// The listings each of `clients` holds blocks through references by, for a graph job to
    /// match a change's blocks against (`Links::hold`).
    pub(crate) async fn links(&self, clients: &[u64]) -> Vec<Links> {
        let joined = self.joined.lock().await;
        clients
            .iter()
            .map(|client| {
                joined
                    .interests
                    .get(client)
                    .map(Interest::links)
                    .unwrap_or_default()
            })
            .collect()
    }

    /// Tells `client` of a change if one of its listings holds the block, or it was told of the
    /// block before; `seen` is what the graph job worked out with `links`. A change of a block
    /// it was told with its references tells what the change did to them in their place, so a
    /// large folder's change costs what changed.
    pub(crate) async fn tell(&self, client: u64, mut message: Push, mut seen: Seen, links: &Links) {
        let told = {
            let mut joined = self.joined.lock().await;
            let interest = joined.interests.entry(client).or_default();
            // A listing that links blocks came after `links` was read: whether it holds this
            // one is not known here, so it is taken to.
            seen.linked |= interest.linking != links.generation;
            match &mut message {
                Push::BlockChanged { block, moved, .. } => {
                    let holds = interest
                        .known
                        .get(&block.id)
                        .is_some_and(|facts| facts.viewed);
                    let facts = Facts::of(block, seen);
                    match holds && block.access.can_view() && moved.is_some() {
                        true => block.references = Vec::new(),
                        false => *moved = None,
                    }
                    interest.told(block.id, facts)
                }
                Push::BlockRemoved { block, .. } => interest.known.remove(block).is_some(),
                _ => true,
            }
        };
        if told {
            self.send_to(client, message).await;
        }
    }

    /// The connections to `organization` already told of `block`, or reading a listing that may
    /// hold it: the only ones a change that moves nothing but its head can reach, since it
    /// leaves which listings match the block as they were. `tell` still matches the block
    /// against each one's listings, so one reading something else costs a summary and no
    /// message.
    pub async fn knowing(&self, organization: Uuid, block: Uuid) -> Vec<(u64, Identity)> {
        let joined = self.joined.lock().await;
        joined
            .by_organization
            .get(&organization)
            .into_iter()
            .flatten()
            .filter(|client| {
                joined.interests.get(client).is_some_and(|interest| {
                    interest.reading > 0 || interest.known.contains_key(&block)
                })
            })
            .filter_map(|client| {
                joined
                    .organizations
                    .get(client)
                    .map(|identity| (*client, *identity))
            })
            .collect()
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

    /// What the hub keeps for `client`: its listings and the facts of each block it was told of.
    #[cfg(test)]
    pub(crate) async fn bytes_kept(&self, client: u64) -> usize {
        let joined = self.joined.lock().await;
        joined.interests.get(&client).map_or(0, |interest| {
            let listing = std::mem::size_of::<Listing>();
            let id = std::mem::size_of::<Uuid>();
            (interest.listings.len() + interest.refused.len()) * listing
                + (interest.references.len() + interest.backrefs.len()) * id
                + interest.known.len() * Facts::BYTES
        })
    }

    pub async fn remove(&self, client: u64) {
        {
            let mut joined = self.joined.lock().await;
            joined.leave_account(client);
            joined.interests.remove(&client);
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
