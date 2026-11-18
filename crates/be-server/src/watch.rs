use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::atomic::{AtomicU64, Ordering},
};

use be_block_protocol::{ClientId, OrganizationRole, PairingMessage};
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
        organization: Uuid,
        entry: Uuid,
        sequence: u64,
    },
    /// The account's roots in the organization may have changed (`topic.roots`).
    Roots {
        organization: Uuid,
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
        run: u64,
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
    /// A message arrived in the account's mailbox, or was deleted (`store/mailbox.rs`).
    MailboxChanged,
    /// The account's key work in the organization may have changed (`topic.key-work`).
    KeyWorkChanged {
        organization: Uuid,
    },
    /// Keys the account may open in the organization were stored (`topic.keys`).
    KeysChanged {
        organization: Uuid,
    },
    /// The account's notification modes in the organization changed through another of its
    /// connections (`topic.notify-modes`).
    NotifyModesChanged {
        organization: Uuid,
    },
}

#[derive(Default)]
struct Joined {
    accounts: HashMap<u64, Uuid>,
    by_account: HashMap<Uuid, HashSet<u64>>,
    /// The organizations each connection works in: those it named in a call it was let make,
    /// with who it is there.
    organizations: HashMap<u64, HashMap<Uuid, Identity>>,
    by_organization: HashMap<Uuid, HashSet<u64>>,
    by_member: HashMap<(Uuid, Uuid), HashSet<u64>>,
    administrators: HashMap<Uuid, HashSet<u64>>,
    /// The entry points each connection follows, in any organization it works in.
    follows: HashMap<u64, HashMap<Place, Told>>,
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
    /// The sequence the connection last read the tree to its end at. A change's write reaches
    /// the hub after it commits, so a read can take the change first: a change at or before
    /// this is one the connection has, and telling it of one would leave it outstanding for a
    /// read the connection, already past it, never makes.
    read: u64,
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
    fn identity(&self, client: u64, organization: Uuid) -> Option<Identity> {
        self.organizations.get(&client)?.get(&organization).copied()
    }

    fn leave_account(&mut self, client: u64) {
        if let Some(account) = self.accounts.remove(&client) {
            unindex(&mut self.by_account, account, client);
        }
        let organizations: Vec<Uuid> = self
            .organizations
            .get(&client)
            .map(|joined| joined.keys().copied().collect())
            .unwrap_or_default();
        for organization in organizations {
            self.leave_organization(client, organization);
        }
        self.follows.remove(&client);
    }

    fn leave_organization(&mut self, client: u64, organization: Uuid) {
        if let Some(follows) = self.follows.get_mut(&client) {
            let left: Vec<Place> = follows
                .keys()
                .filter(|place| place.organization == organization)
                .copied()
                .collect();
            for place in left {
                follows.remove(&place);
                unindex(&mut self.followers, place, client);
            }
        }
        let identity = self
            .organizations
            .get_mut(&client)
            .and_then(|joined| joined.remove(&organization));
        if self
            .organizations
            .get(&client)
            .is_some_and(HashMap::is_empty)
        {
            self.organizations.remove(&client);
        }
        if let Some(identity) = identity {
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

    /// Follows `place`, before the read that starts it, so nothing changed during the read is
    /// missed; answers whether it did not follow it already. Refused once the connection
    /// follows `MAX_FOLLOWS_PER_CONNECTION`, and while it does not work in the organization.
    pub async fn follow(&self, client: u64, place: Place) -> Result<bool, ServerError> {
        let mut joined = self.joined.lock().await;
        if joined
            .organizations
            .get(&client)
            .is_none_or(|organizations| !organizations.contains_key(&place.organization))
        {
            return Ok(false);
        }
        let follows = joined.follows.entry(client).or_default();
        if !follows.contains_key(&place) && follows.len() >= MAX_FOLLOWS_PER_CONNECTION {
            return Err(crate::limits::too_many(
                "entry points one connection follows; unfollow some first",
                MAX_FOLLOWS_PER_CONNECTION,
            ));
        }
        let new = !follows.contains_key(&place);
        follows.entry(place).or_default().outstanding = true;
        joined.followers.entry(place).or_default().insert(client);
        Ok(new)
    }

    /// A read of the tree of `place` failed: the next change is told again, as after a read to
    /// the end.
    pub async fn unread(&self, client: u64, place: Place) {
        let mut joined = self.joined.lock().await;
        if let Some(told) = joined
            .follows
            .get_mut(&client)
            .and_then(|follows| follows.get_mut(&place))
        {
            *told = Told {
                read: told.read,
                ..Told::default()
            };
        }
    }

    /// `client` may no longer view `place`: it is told once more that the tree advanced, so
    /// it reads it and is refused, and it follows the entry no longer.
    pub async fn left(&self, client: u64, place: Place, sequence: u64) {
        self.unfollow(client, place).await;
        self.send_to(
            client,
            Push::TreeAdvanced {
                organization: place.organization,
                entry: place.block,
                sequence,
            },
        )
        .await;
    }

    pub async fn unfollow(&self, client: u64, place: Place) {
        let mut joined = self.joined.lock().await;
        if let Some(follows) = joined.follows.get_mut(&client) {
            follows.remove(&place);
        }
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
                if let Some(identity) = joined
                    .organizations
                    .get(client)
                    .and_then(|organizations| organizations.get(&organization))
                {
                    found.push((*client, *identity, *block));
                }
            }
        }
        found
    }

    /// Tells `client` that the tree of `place` advanced to `sequence`, unless it was told
    /// already and has not read it to the end since (`Told`).
    pub async fn advance(&self, client: u64, place: Place, sequence: u64) {
        let send = {
            let mut joined = self.joined.lock().await;
            let Some(told) = joined
                .follows
                .get_mut(&client)
                .and_then(|follows| follows.get_mut(&place))
            else {
                return;
            };
            if sequence <= told.read {
                return;
            }
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
            self.send_to(
                client,
                Push::TreeAdvanced {
                    organization: place.organization,
                    entry: place.block,
                    sequence,
                },
            )
            .await;
        }
    }

    /// `client` read the tree of `place` to its end, as of `sequence`: told of anything since,
    /// and otherwise told again of the next change. `sequence` and the one `advance` is given
    /// are both the organization's `change_sequence`, read under the store's one database
    /// connection, so a change at or before `sequence` is one the read took (`Told::read`).
    pub async fn read(&self, client: u64, place: Place, sequence: u64) {
        let send = {
            let mut joined = self.joined.lock().await;
            let Some(told) = joined
                .follows
                .get_mut(&client)
                .and_then(|follows| follows.get_mut(&place))
            else {
                return;
            };
            told.read = told.read.max(sequence);
            match told.latest.take().filter(|latest| *latest > sequence) {
                Some(latest) => Some(latest),
                None => {
                    told.outstanding = false;
                    None
                }
            }
        };
        if let Some(sequence) = send {
            self.send_to(
                client,
                Push::TreeAdvanced {
                    organization: place.organization,
                    entry: place.block,
                    sequence,
                },
            )
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
        self.send_every(&clients, &Push::Roots { organization })
            .await;
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

    /// `client` works in `identity.organization` as `identity` from now: it named the
    /// organization in a call it was let make. Joining again takes a changed role.
    pub async fn join_organization(&self, client: u64, identity: Identity) {
        let mut joined = self.joined.lock().await;
        let organization = identity.organization;
        if joined
            .organizations
            .get(&client)
            .and_then(|organizations| organizations.get(&organization))
            .is_some_and(|held| held.role != identity.role)
        {
            unindex(&mut joined.administrators, organization, client);
        }
        joined
            .organizations
            .entry(client)
            .or_default()
            .insert(organization, identity);
        joined
            .by_organization
            .entry(organization)
            .or_default()
            .insert(client);
        joined
            .by_member
            .entry((organization, identity.account))
            .or_default()
            .insert(client);
        if identity.role == OrganizationRole::Administrator {
            joined
                .administrators
                .entry(organization)
                .or_default()
                .insert(client);
        }
    }

    /// `account`'s role in `organization` changed: every connection of it there is who it is
    /// now, so the next call each makes is admitted by the new role.
    pub async fn set_role(&self, organization: Uuid, account: Uuid, role: OrganizationRole) {
        let mut joined = self.joined.lock().await;
        let clients: Vec<u64> = joined
            .by_member
            .get(&(organization, account))
            .into_iter()
            .flatten()
            .copied()
            .collect();
        for client in clients {
            if let Some(identity) = joined
                .organizations
                .get_mut(&client)
                .and_then(|organizations| organizations.get_mut(&organization))
            {
                identity.role = role;
            }
            match role {
                OrganizationRole::Administrator => {
                    joined
                        .administrators
                        .entry(organization)
                        .or_default()
                        .insert(client);
                }
                OrganizationRole::Editor => {
                    unindex(&mut joined.administrators, organization, client);
                }
            }
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
            .filter_map(|client| Some((client, joined.identity(client, organization)?)))
            .collect()
    }

    /// Who `client` is in `organization`, once it works there.
    pub async fn identity(&self, client: u64, organization: Uuid) -> Option<Identity> {
        self.joined.lock().await.identity(client, organization)
    }

    pub async fn clients_of(&self, organization: Uuid, account: Uuid) -> Vec<(u64, Identity)> {
        let joined = self.joined.lock().await;
        joined
            .by_member
            .get(&(organization, account))
            .into_iter()
            .flatten()
            .filter_map(|client| Some((*client, joined.identity(*client, organization)?)))
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
                joined.leave_organization(*client, organization);
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
