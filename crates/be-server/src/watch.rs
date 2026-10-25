use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::atomic::{AtomicU64, Ordering},
};

use be_graph::BlockParent;
use be_protocol::{BlockSummary, Listing, OrganizationRole, ServerMessage};
use tokio::sync::{
    Mutex,
    mpsc::{Sender, error::TrySendError},
};

use crate::Identity;
use uuid::Uuid;

#[derive(Default)]
struct Joined {
    accounts: HashMap<u64, Uuid>,
    by_account: HashMap<Uuid, HashSet<u64>>,
    organizations: HashMap<u64, Identity>,
    by_organization: HashMap<Uuid, HashSet<u64>>,
    by_member: HashMap<(Uuid, Uuid), HashSet<u64>>,
    administrators: HashMap<Uuid, HashSet<u64>>,
    watchers: HashMap<Uuid, HashSet<u64>>,
    watching: HashMap<u64, HashSet<Uuid>>,
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
    known: HashMap<Uuid, Facts>,
}

impl Interest {
    fn matches(&self, block: Uuid, facts: &Facts) -> bool {
        self.listings.contains(&Listing::Block(block))
            || facts
                .parent
                .is_some_and(|parent| self.listings.contains(&Listing::Children(parent)))
            || (facts.parent.is_some() && !facts.held && self.listings.contains(&Listing::Deleted))
            || (facts.entry && self.listings.contains(&Listing::Shared))
            || (facts.opens_inside
                && self
                    .listings
                    .iter()
                    .any(|listing| matches!(listing, Listing::OpenSettings(_))))
            || facts
                .references
                .iter()
                .any(|target| self.listings.contains(&Listing::Backrefs(*target)))
            || self.listings.iter().any(|listing| {
                matches!(listing, Listing::References(referrer) if self
                    .known
                    .get(referrer)
                    .is_some_and(|referrer| referrer.references.contains(&block)))
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
    clients: Mutex<HashMap<u64, Sender<ServerMessage>>>,
    joined: Mutex<Joined>,
}

impl WatchHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register(&self, sender: Sender<ServerMessage>) -> u64 {
        let client = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.clients.lock().await.insert(client, sender);
        client
    }

    pub async fn watch(&self, block: Uuid, client: u64) {
        let mut joined = self.joined.lock().await;
        joined.watchers.entry(block).or_default().insert(client);
        joined.watching.entry(client).or_default().insert(block);
    }

    pub async fn unwatch(&self, block: Uuid, client: u64) {
        let mut joined = self.joined.lock().await;
        unindex(&mut joined.watchers, block, client);
        if let Some(blocks) = joined.watching.get_mut(&client) {
            blocks.remove(&block);
            if blocks.is_empty() {
                joined.watching.remove(&client);
            }
        }
    }

    pub async fn list(&self, client: u64, listing: Listing) {
        let mut joined = self.joined.lock().await;
        let interest = joined.interests.entry(client).or_default();
        interest.listings.insert(listing);
    }

    pub async fn listed(&self, client: u64, listing: Listing, blocks: &[BlockSummary]) {
        let mut joined = self.joined.lock().await;
        let interest = joined.interests.entry(client).or_default();
        for summary in blocks {
            interest.told(summary.id, Facts::of(summary, listing == Listing::Shared));
        }
    }

    pub async fn unlist(&self, client: u64, listing: Listing) {
        let mut joined = self.joined.lock().await;
        let Some(interest) = joined.interests.get_mut(&client) else {
            return;
        };
        interest.listings.remove(&listing);
        let known = std::mem::take(&mut interest.known);
        interest.known = known
            .into_iter()
            .filter(|(block, facts)| interest.matches(*block, facts))
            .collect();
    }

    pub async fn tell(&self, client: u64, message: ServerMessage, entry: bool) {
        let told = {
            let mut joined = self.joined.lock().await;
            let interest = joined.interests.entry(client).or_default();
            match &message {
                ServerMessage::BlockChanged { block } => {
                    interest.told(block.id, Facts::of(block, entry))
                }
                ServerMessage::BlockRemoved { block, .. } => interest.known.remove(block).is_some(),
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
            deliver(
                &mut clients,
                target,
                ServerMessage::BlockRemoved { block, revision },
            );
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
        message: &ServerMessage,
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

    pub async fn announce(&self, organization: Uuid, message: ServerMessage) {
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

    pub async fn send_to(&self, client: u64, message: ServerMessage) {
        deliver(&mut *self.clients.lock().await, client, message);
    }

    pub async fn send_all(&self, clients: &[u64], from: u64, message: &ServerMessage) {
        let mut senders = self.clients.lock().await;
        for client in clients {
            if *client != from {
                deliver(&mut senders, *client, message.clone());
            }
        }
    }

    pub async fn broadcast(&self, block: Uuid, from: u64, message: ServerMessage) {
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

fn deliver(
    clients: &mut HashMap<u64, Sender<ServerMessage>>,
    client: u64,
    message: ServerMessage,
) -> bool {
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
