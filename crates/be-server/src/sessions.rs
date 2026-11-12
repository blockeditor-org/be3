use std::collections::{BTreeMap, HashMap, HashSet};

use be_block_commit::{CommitId, now_milliseconds};
use be_block_protocol::{ClientId, SessionState};
use be_block_session::{Claim, Lease};
use tokio::sync::Mutex;

use crate::watch::Place;

/// How long the registry remembers a session nobody is in, for its `clean_at`. A peer taking
/// over a session forgotten after this merges rather than fast-forwards
/// (`be_block_session::takeover_needs_merge`), which is safe, only slower.
pub const EMPTY_SESSION_MEMORY_MILLISECONDS: i64 = 24 * 60 * 60 * 1000;
/// Empty sessions the registry remembers at most; past this it forgets the longest empty first.
pub const MAX_EMPTY_SESSIONS: usize = 65_536;

pub struct SessionRegistry {
    sessions: Mutex<Sessions>,
    lease: i64,
}

#[derive(Default)]
struct Sessions {
    leases: HashMap<Place, Lease>,
    /// The blocks each client is a participant of, so leaving them all costs what the client
    /// joined rather than every session on the server.
    joined: HashMap<ClientId, HashSet<Place>>,
    /// Empty sessions kept for their `clean_at`, in the order they went empty, with when.
    empty: BTreeMap<u64, (Place, i64)>,
    emptied: HashMap<Place, u64>,
    next: u64,
}

impl Sessions {
    fn lease(&mut self, block: Place, length: i64) -> &mut Lease {
        if let Some(order) = self.emptied.remove(&block) {
            self.empty.remove(&order);
        }
        self.leases
            .entry(block)
            .or_insert_with(|| Lease::lasting(length))
    }

    fn joined(&mut self, block: Place, client: ClientId) {
        self.joined.entry(client).or_default().insert(block);
    }

    fn left(&mut self, block: Place, client: ClientId) {
        if let Some(blocks) = self.joined.get_mut(&client) {
            blocks.remove(&block);
            if blocks.is_empty() {
                self.joined.remove(&client);
            }
        }
    }

    /// Forgets `block`'s session once it is empty and has no `clean_at`, or remembers when it
    /// went empty; then forgets whatever has been empty too long or too many.
    fn settle(&mut self, block: Place, now: i64) {
        if let Some(lease) = self.leases.get(&block)
            && lease.is_empty()
            && !self.emptied.contains_key(&block)
        {
            match lease.clean_at() {
                None => {
                    self.leases.remove(&block);
                }
                Some(_) => {
                    self.next += 1;
                    self.empty.insert(self.next, (block, now));
                    self.emptied.insert(block, self.next);
                }
            }
        }
        while let Some((_, &(oldest, at))) = self.empty.first_key_value() {
            if at > now - EMPTY_SESSION_MEMORY_MILLISECONDS
                && self.empty.len() <= MAX_EMPTY_SESSIONS
            {
                break;
            }
            self.empty.pop_first();
            self.emptied.remove(&oldest);
            self.leases.remove(&oldest);
        }
    }
}

impl SessionRegistry {
    pub fn new(lease: std::time::Duration) -> Self {
        Self {
            sessions: Mutex::new(Sessions::default()),
            lease: i64::try_from(lease.as_millis()).unwrap_or(i64::MAX),
        }
    }

    /// `client` joins `block`'s session, taking the lease if no one holds it and it `may_edit`.
    pub async fn join(&self, block: Place, client: ClientId, may_edit: bool) -> SessionState {
        let mut sessions = self.sessions.lock().await;
        let now = now_milliseconds();
        let lease = sessions.lease(block, self.lease);
        lease.join(client, may_edit, now);
        let state = lease.state().clone();
        sessions.joined(block, client);
        sessions.settle(block, now);
        state
    }

    pub async fn leave(&self, block: Place, client: ClientId) -> Option<SessionState> {
        let mut sessions = self.sessions.lock().await;
        let now = now_milliseconds();
        let lease = sessions.leases.get_mut(&block)?;
        lease.leave(client, now);
        let state = lease.state().clone();
        sessions.left(block, client);
        sessions.settle(block, now);
        Some(state)
    }

    pub async fn leave_all(&self, client: ClientId) -> Vec<(Place, SessionState)> {
        let mut sessions = self.sessions.lock().await;
        let now = now_milliseconds();
        let mut changed = Vec::new();
        for block in sessions.joined.remove(&client).unwrap_or_default() {
            let Some(lease) = sessions.leases.get_mut(&block) else {
                continue;
            };
            lease.leave(client, now);
            changed.push((block, lease.state().clone()));
            sessions.settle(block, now);
        }
        changed
    }

    pub async fn claim(
        &self,
        block: Place,
        client: ClientId,
        generation: u64,
    ) -> (Claim, SessionState) {
        let mut sessions = self.sessions.lock().await;
        let now = now_milliseconds();
        let lease = sessions.lease(block, self.lease);
        let claim = lease.claim(client, generation, now);
        let state = lease.state().clone();
        if state.participants.contains(&client) {
            sessions.joined(block, client);
        }
        sessions.settle(block, now);
        (claim, state)
    }

    pub async fn heartbeat(
        &self,
        block: Place,
        client: ClientId,
        generation: u64,
        clean_at: Option<CommitId>,
    ) -> Option<SessionState> {
        let mut sessions = self.sessions.lock().await;
        let lease = sessions.leases.get_mut(&block)?;
        lease
            .heartbeat(client, generation, clean_at, now_milliseconds())
            .then(|| lease.state().clone())
    }

    pub async fn blocks_of(&self, client: ClientId) -> Vec<Place> {
        self.sessions
            .lock()
            .await
            .joined
            .get(&client)
            .map(|blocks| blocks.iter().copied().collect())
            .unwrap_or_default()
    }

    pub async fn state(&self, block: Place) -> SessionState {
        self.sessions
            .lock()
            .await
            .leases
            .get(&block)
            .map(|lease| lease.state().clone())
            .unwrap_or_default()
    }

    pub async fn participants(&self, block: Place) -> Vec<ClientId> {
        self.sessions
            .lock()
            .await
            .leases
            .get(&block)
            .map(|lease| lease.state().participants.clone())
            .unwrap_or_default()
    }

    /// Sessions the registry holds, in use or remembered empty.
    pub async fn held(&self) -> usize {
        self.sessions.lock().await.leases.len()
    }
}
