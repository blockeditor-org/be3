use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicU64, Ordering},
};

use be_protocol::ServerMessage;
use tokio::sync::{Mutex, mpsc::UnboundedSender};

use crate::Identity;
use uuid::Uuid;

#[derive(Default)]
pub struct WatchHub {
    next: AtomicU64,
    clients: Mutex<HashMap<u64, UnboundedSender<ServerMessage>>>,
    watchers: Mutex<HashMap<Uuid, HashSet<u64>>>,
    workspaces: Mutex<HashMap<u64, Identity>>,
}

impl WatchHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register(&self, sender: UnboundedSender<ServerMessage>) -> u64 {
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
        let clients = self.clients.lock().await;
        for target in targets {
            if let Some(sender) = clients.get(&target) {
                let _ = sender.send(message.clone());
            }
        }
    }

    pub async fn remove(&self, client: u64) {
        self.workspaces.lock().await.remove(&client);
        self.clients.lock().await.remove(&client);
        let mut watchers = self.watchers.lock().await;
        watchers.retain(|_, clients| {
            clients.remove(&client);
            !clients.is_empty()
        });
    }

    pub async fn send_to(&self, client: u64, message: ServerMessage) {
        if let Some(sender) = self.clients.lock().await.get(&client) {
            let _ = sender.send(message);
        }
    }

    pub async fn send_all(&self, clients: &[u64], from: u64, message: &ServerMessage) {
        let senders = self.clients.lock().await;
        for client in clients {
            if *client == from {
                continue;
            }
            if let Some(sender) = senders.get(client) {
                let _ = sender.send(message.clone());
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
        let clients = self.clients.lock().await;
        for target in targets {
            if let Some(sender) = clients.get(&target) {
                let _ = sender.send(message.clone());
            }
        }
    }
}
