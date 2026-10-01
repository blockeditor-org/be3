use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicU64, Ordering},
};

use be_protocol::ServerMessage;
use tokio::sync::{
    Mutex,
    mpsc::{Sender, error::TrySendError},
};

use crate::Identity;
use uuid::Uuid;

#[derive(Default)]
pub struct WatchHub {
    next: AtomicU64,
    clients: Mutex<HashMap<u64, Sender<ServerMessage>>>,
    watchers: Mutex<HashMap<Uuid, HashSet<u64>>>,
    workspaces: Mutex<HashMap<u64, Identity>>,
    accounts: Mutex<HashMap<u64, Uuid>>,
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
        self.watchers
            .lock()
            .await
            .entry(block)
            .or_default()
            .insert(client);
    }

    pub async fn unwatch(&self, block: Uuid, client: u64) {
        let mut watchers = self.watchers.lock().await;
        if let Some(clients) = watchers.get_mut(&block) {
            clients.remove(&client);
            if clients.is_empty() {
                watchers.remove(&block);
            }
        }
    }

    pub async fn join_account(&self, client: u64, account: Uuid) {
        self.accounts.lock().await.insert(client, account);
    }

    pub async fn leave_account(&self, client: u64) {
        self.accounts.lock().await.remove(&client);
        self.workspaces.lock().await.remove(&client);
    }

    pub async fn send_account(
        &self,
        account: Uuid,
        from: u64,
        to: Option<u64>,
        message: &ServerMessage,
    ) -> usize {
        let targets: Vec<u64> = self
            .accounts
            .lock()
            .await
            .iter()
            .filter(|(client, owner)| {
                **owner == account && **client != from && to.is_none_or(|to| to == **client)
            })
            .map(|(client, _)| *client)
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
        self.workspaces.lock().await.insert(client, identity);
    }

    pub async fn members(&self, workspace: Uuid) -> Vec<(u64, Identity)> {
        self.workspaces
            .lock()
            .await
            .iter()
            .filter(|(_, joined)| joined.workspace == workspace)
            .map(|(client, identity)| (*client, *identity))
            .collect()
    }

    pub async fn announce(&self, workspace: Uuid, message: ServerMessage) {
        let targets: Vec<u64> = self
            .workspaces
            .lock()
            .await
            .iter()
            .filter(|(_, joined)| joined.workspace == workspace)
            .map(|(client, _)| *client)
            .collect();
        let mut clients = self.clients.lock().await;
        for target in targets {
            deliver(&mut clients, target, message.clone());
        }
    }

    pub async fn remove(&self, client: u64) {
        self.workspaces.lock().await.remove(&client);
        self.accounts.lock().await.remove(&client);
        self.clients.lock().await.remove(&client);
        let mut watchers = self.watchers.lock().await;
        watchers.retain(|_, clients| {
            clients.remove(&client);
            !clients.is_empty()
        });
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
        let targets: Vec<_> = {
            let watchers = self.watchers.lock().await;
            watchers
                .get(&block)
                .map(|clients| {
                    clients
                        .iter()
                        .copied()
                        .filter(|client| *client != from)
                        .collect()
                })
                .unwrap_or_default()
        };
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
