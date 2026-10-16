use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::atomic::{AtomicU64, Ordering},
};

use be_protocol::{ServerMessage, WorkspaceRole};
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
    workspaces: HashMap<u64, Identity>,
    by_workspace: HashMap<Uuid, HashSet<u64>>,
    by_member: HashMap<(Uuid, Uuid), HashSet<u64>>,
    administrators: HashMap<Uuid, HashSet<u64>>,
    watchers: HashMap<Uuid, HashSet<u64>>,
    watching: HashMap<u64, HashSet<Uuid>>,
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
        self.leave_workspace(client);
    }

    fn leave_workspace(&mut self, client: u64) {
        if let Some(identity) = self.workspaces.remove(&client) {
            unindex(&mut self.by_workspace, identity.workspace, client);
            unindex(
                &mut self.by_member,
                (identity.workspace, identity.account),
                client,
            );
            unindex(&mut self.administrators, identity.workspace, client);
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

    pub async fn join_workspace(&self, client: u64, identity: Identity) {
        let mut joined = self.joined.lock().await;
        joined.leave_workspace(client);
        joined.workspaces.insert(client, identity);
        joined
            .by_workspace
            .entry(identity.workspace)
            .or_default()
            .insert(client);
        joined
            .by_member
            .entry((identity.workspace, identity.account))
            .or_default()
            .insert(client);
        if identity.role == WorkspaceRole::Administrator {
            joined
                .administrators
                .entry(identity.workspace)
                .or_default()
                .insert(client);
        }
    }

    pub async fn audience(
        &self,
        workspace: Uuid,
        accounts: &BTreeSet<Uuid>,
    ) -> Vec<(u64, Identity)> {
        let joined = self.joined.lock().await;
        let mut clients: BTreeSet<u64> = joined
            .administrators
            .get(&workspace)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        for account in accounts {
            clients.extend(
                joined
                    .by_member
                    .get(&(workspace, *account))
                    .into_iter()
                    .flatten()
                    .copied(),
            );
        }
        clients
            .into_iter()
            .filter_map(|client| {
                joined
                    .workspaces
                    .get(&client)
                    .map(|identity| (client, *identity))
            })
            .collect()
    }

    pub async fn announce(&self, workspace: Uuid, message: ServerMessage) {
        let targets: Vec<u64> = self
            .joined
            .lock()
            .await
            .by_workspace
            .get(&workspace)
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
