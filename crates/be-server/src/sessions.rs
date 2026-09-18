use std::collections::HashMap;

use be_commit::{CommitId, now_milliseconds};
use be_protocol::{ClientId, SessionState};
use be_session::{Claim, Lease};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
pub struct SessionRegistry {
    sessions: Mutex<HashMap<Uuid, Lease>>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn join(&self, block: Uuid, client: ClientId) -> SessionState {
        let mut sessions = self.sessions.lock().await;
        let lease = sessions.entry(block).or_default();
        lease.join(client, now_milliseconds());
        lease.state().clone()
    }

    pub async fn leave(&self, block: Uuid, client: ClientId) -> Option<SessionState> {
        let mut sessions = self.sessions.lock().await;
        let lease = sessions.get_mut(&block)?;
        lease.leave(client, now_milliseconds());
        let state = lease.state().clone();
        if forgettable(lease) {
            sessions.remove(&block);
        }
        Some(state)
    }

    pub async fn leave_all(&self, client: ClientId) -> Vec<(Uuid, SessionState)> {
        let mut sessions = self.sessions.lock().await;
        let now = now_milliseconds();
        let mut changed = Vec::new();
        let mut empty = Vec::new();
        for (block, lease) in sessions.iter_mut() {
            if !lease.state().participants.contains(&client) {
                continue;
            }
            lease.leave(client, now);
            changed.push((*block, lease.state().clone()));
            if forgettable(lease) {
                empty.push(*block);
            }
        }
        for block in empty {
            sessions.remove(&block);
        }
        changed
    }

    pub async fn claim(
        &self,
        block: Uuid,
        client: ClientId,
        generation: u64,
    ) -> (Claim, SessionState) {
        let mut sessions = self.sessions.lock().await;
        let lease = sessions.entry(block).or_default();
        let claim = lease.claim(client, generation, now_milliseconds());
        (claim, lease.state().clone())
    }

    pub async fn heartbeat(
        &self,
        block: Uuid,
        client: ClientId,
        generation: u64,
        clean_at: Option<CommitId>,
    ) -> Option<SessionState> {
        let mut sessions = self.sessions.lock().await;
        let lease = sessions.get_mut(&block)?;
        lease
            .heartbeat(client, generation, clean_at, now_milliseconds())
            .then(|| lease.state().clone())
    }

    pub async fn state(&self, block: Uuid) -> SessionState {
        self.sessions
            .lock()
            .await
            .get(&block)
            .map(|lease| lease.state().clone())
            .unwrap_or_default()
    }

    pub async fn participants(&self, block: Uuid) -> Vec<ClientId> {
        self.sessions
            .lock()
            .await
            .get(&block)
            .map(|lease| lease.state().participants.clone())
            .unwrap_or_default()
    }
}

fn forgettable(lease: &Lease) -> bool {
    lease.is_empty() && lease.clean_at().is_none()
}
