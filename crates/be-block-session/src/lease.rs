use be_block_protocol::{ClientId, SessionState};

pub const LEASE_MILLISECONDS: i64 = 30_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Claim {
    Granted { generation: u64 },
    HeldByAnother,
    Stale,
}

/// Session ownership. A claim succeeds only against the generation the claimant last saw, and
/// only when the lease expired or the owner left, so a stale claim cannot stomp a newer owner.
/// The owner renews it with `ReportClean` after every seal and every half lease on its
/// replica's timer. `clean_at` outlives the session going empty, so a peer taking over an
/// abandoned session can tell whether anything was unpublished. Only a participant that may
/// edit the block is granted it: what only the owner says, the server relays only from it.
#[derive(Clone, Debug)]
pub struct Lease {
    state: SessionState,
    /// The participants that may own the lease.
    editors: Vec<ClientId>,
    expires: i64,
    length: i64,
}

impl Default for Lease {
    fn default() -> Self {
        Self::lasting(LEASE_MILLISECONDS)
    }
}

impl Lease {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lasting(length: i64) -> Self {
        Self {
            state: SessionState::default(),
            editors: Vec::new(),
            expires: 0,
            length,
        }
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }

    pub fn owner(&self) -> Option<ClientId> {
        self.state.owner
    }

    pub fn generation(&self) -> u64 {
        self.state.generation
    }

    pub fn expires(&self) -> i64 {
        self.expires
    }

    /// `client` joins, and is granted the lease if no one holds it and it `may_edit`.
    pub fn join(&mut self, client: ClientId, may_edit: bool, now: i64) {
        if !self.state.participants.contains(&client) {
            self.state.participants.push(client);
        }
        if may_edit && !self.editors.contains(&client) {
            self.editors.push(client);
        }
        if self.state.owner.is_none() && may_edit {
            self.grant(client, now);
        }
    }

    pub fn leave(&mut self, client: ClientId, now: i64) {
        self.state.participants.retain(|held| *held != client);
        self.demote(client, now);
    }

    /// `client` may no longer edit: it stays a participant, and the lease, if it held it, goes
    /// to the next participant that may. Answers whether it held the lease.
    pub fn demote(&mut self, client: ClientId, now: i64) -> bool {
        self.editors.retain(|held| *held != client);
        if self.state.owner != Some(client) {
            return false;
        }
        self.state.owner = None;
        self.state.generation += 1;
        self.expires = now;
        if let Some(next) = self.editors.first().copied() {
            self.grant(next, now);
        }
        true
    }

    pub fn clean_at(&self) -> Option<be_block_commit::CommitId> {
        self.state.clean_at
    }

    pub fn claim(&mut self, client: ClientId, generation: u64, now: i64) -> Claim {
        if generation != self.state.generation {
            return Claim::Stale;
        }
        if self.state.owner == Some(client) {
            self.expires = now.saturating_add(self.length);
            return Claim::Granted {
                generation: self.state.generation,
            };
        }
        let held = self
            .state
            .owner
            .is_some_and(|owner| self.state.participants.contains(&owner) && now < self.expires);
        if held {
            return Claim::HeldByAnother;
        }
        self.grant(client, now);
        Claim::Granted {
            generation: self.state.generation,
        }
    }

    pub fn heartbeat(
        &mut self,
        client: ClientId,
        generation: u64,
        clean_at: Option<be_block_commit::CommitId>,
        now: i64,
    ) -> bool {
        if self.state.owner != Some(client) || generation != self.state.generation {
            return false;
        }
        self.expires = now.saturating_add(self.length);
        self.state.clean_at = clean_at;
        true
    }

    pub fn is_expired(&self, now: i64) -> bool {
        self.state.owner.is_some() && now >= self.expires
    }

    pub fn is_empty(&self) -> bool {
        self.state.participants.is_empty()
    }

    fn grant(&mut self, client: ClientId, now: i64) {
        if !self.state.participants.contains(&client) {
            self.state.participants.push(client);
        }
        if !self.editors.contains(&client) {
            self.editors.push(client);
        }
        self.state.owner = Some(client);
        self.state.generation += 1;
        self.expires = now.saturating_add(self.length);
    }
}
