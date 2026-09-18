use be_protocol::{ClientId, SessionState};

pub const LEASE_MILLISECONDS: i64 = 30_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Claim {
    Granted { generation: u64 },
    HeldByAnother,
    Stale,
}

#[derive(Clone, Debug, Default)]
pub struct Lease {
    state: SessionState,
    expires: i64,
}

impl Lease {
    pub fn new() -> Self {
        Self::default()
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

    pub fn join(&mut self, client: ClientId, now: i64) {
        if !self.state.participants.contains(&client) {
            self.state.participants.push(client);
        }
        if self.state.owner.is_none() {
            self.grant(client, now);
        }
    }

    pub fn leave(&mut self, client: ClientId, now: i64) {
        self.state.participants.retain(|held| *held != client);
        if self.state.owner == Some(client) {
            self.state.owner = None;
            self.state.generation += 1;
            self.expires = now;
            if let Some(next) = self.state.participants.first().copied() {
                self.grant(next, now);
            }
        }
    }

    pub fn clean_at(&self) -> Option<be_commit::CommitId> {
        self.state.clean_at
    }

    pub fn claim(&mut self, client: ClientId, generation: u64, now: i64) -> Claim {
        if generation != self.state.generation {
            return Claim::Stale;
        }
        if self.state.owner == Some(client) {
            self.expires = now + LEASE_MILLISECONDS;
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
        clean_at: Option<be_commit::CommitId>,
        now: i64,
    ) -> bool {
        if self.state.owner != Some(client) || generation != self.state.generation {
            return false;
        }
        self.expires = now + LEASE_MILLISECONDS;
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
        self.state.owner = Some(client);
        self.state.generation += 1;
        self.expires = now + LEASE_MILLISECONDS;
    }
}
