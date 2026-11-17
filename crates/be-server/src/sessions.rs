//! Live sessions: who is in each block's session, kept in memory only. The registry writes
//! membership entries (joining, leaving, a change of edit access, a claim) into each session's
//! order between its submissions, stamped with the order's last number, and sends them and the
//! order's entries to the participants under one lock, so each hears them in one order. The
//! holder of the lease follows from them (`be_block_session::Membership`), here as on every
//! replica. After a restart every session is empty and rebuilds from joins.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use be_block_commit::CommitId;
use be_block_protocol::{
    ClientId, JoinedParticipant, JournalEntry, MembershipChange, MembershipEntry, SessionState,
};
use be_block_session::Membership;
use tokio::sync::Mutex;

use crate::watch::{Place, Push, WatchHub};

pub struct SessionRegistry {
    sessions: Mutex<Sessions>,
    hub: Arc<WatchHub>,
}

#[derive(Default)]
struct Session {
    state: SessionState,
    /// The order's last number the participants were sent: what a membership entry is stamped
    /// with.
    tip: u64,
}

#[derive(Default)]
struct Sessions {
    sessions: HashMap<Place, Session>,
    /// The blocks each client is a participant of, so leaving them all costs what the client
    /// joined rather than every session on the server.
    joined: HashMap<ClientId, HashSet<Place>>,
}

impl Sessions {
    /// Writes a membership entry into `block`'s session and sends it to its participants: to
    /// `client` too if it just left, but not if it just joined.
    async fn write(
        &mut self,
        hub: &WatchHub,
        block: Place,
        client: ClientId,
        change: MembershipChange,
    ) -> Option<SessionState> {
        let session = self.sessions.get_mut(&block)?;
        let left = matches!(change, MembershipChange::Left);
        let joined = matches!(change, MembershipChange::Joined(_));
        let entry = session.state.write(session.tip, client, change)?;
        let state = session.state.clone();
        let mut told = state.clients();
        // A joiner has the membership its join answers, its own entry in it.
        if joined {
            told.retain(|told| *told != client);
        }
        if left {
            told.push(client);
            if let Some(blocks) = self.joined.get_mut(&client) {
                blocks.remove(&block);
                if blocks.is_empty() {
                    self.joined.remove(&client);
                }
            }
            if state.participants.is_empty() {
                self.sessions.remove(&block);
            }
        }
        send(hub, block, &told, entry).await;
        Some(state)
    }
}

async fn send(hub: &WatchHub, block: Place, told: &[ClientId], entry: MembershipEntry) {
    hub.send_every(
        told,
        &Push::Membership {
            block,
            entry: Arc::new(entry),
        },
    )
    .await;
}

impl SessionRegistry {
    pub fn new(hub: Arc<WatchHub>) -> Self {
        Self {
            sessions: Mutex::new(Sessions::default()),
            hub,
        }
    }

    /// `client`, running the build `tag`, joins `block`'s session: answers the membership as of
    /// the number it joins at, itself in it, which every other participant hears.
    pub async fn join(
        &self,
        block: Place,
        client: ClientId,
        may_edit: bool,
        tag: be_block_store::Hash,
    ) -> SessionState {
        let mut sessions = self.sessions.lock().await;
        sessions.sessions.entry(block).or_default();
        sessions.joined.entry(client).or_default().insert(block);
        let change = MembershipChange::Joined(JoinedParticipant { tag, may_edit });
        sessions
            .write(&self.hub, block, client, change)
            .await
            .unwrap_or_default()
    }

    /// Writes that `client` left `block`'s session; false when it was not in it.
    pub async fn leave(&self, block: Place, client: ClientId) -> bool {
        let mut sessions = self.sessions.lock().await;
        sessions
            .write(&self.hub, block, client, MembershipChange::Left)
            .await
            .is_some()
    }

    /// Writes that `client` left every session it was in: its connection closed, or went
    /// silent past `IDLE_TIMEOUT`.
    pub async fn leave_all(&self, client: ClientId) {
        let mut sessions = self.sessions.lock().await;
        for block in sessions.joined.get(&client).cloned().unwrap_or_default() {
            sessions
                .write(&self.hub, block, client, MembershipChange::Left)
                .await;
        }
    }

    /// `client`'s edit access to `block` is now `may_edit`: written when it changed.
    pub async fn access(&self, block: Place, client: ClientId, may_edit: bool) {
        let mut sessions = self.sessions.lock().await;
        sessions
            .write(&self.hub, block, client, MembershipChange::Access(may_edit))
            .await;
    }

    /// Writes `client`'s claim of the lease; false when it is not a participant that may edit.
    pub async fn claim(&self, block: Place, client: ClientId) -> bool {
        let mut sessions = self.sessions.lock().await;
        let editor = sessions.sessions.get(&block).is_some_and(|session| {
            session
                .state
                .participants
                .iter()
                .any(|participant| participant.client == client && participant.may_edit)
        });
        editor
            && sessions
                .write(&self.hub, block, client, MembershipChange::Claimed)
                .await
                .is_some()
    }

    /// The order numbered `entry`: every participant hears it, its author too.
    pub async fn sequenced(&self, block: Place, entry: JournalEntry) {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.sessions.get_mut(&block) else {
            return;
        };
        session.tip = session.tip.max(entry.sequence);
        let participants = session.state.clients();
        self.hub
            .send_every(
                &participants,
                &Push::Sequenced {
                    block,
                    entry: Arc::new(entry),
                },
            )
            .await;
    }

    /// A seal moved `block`'s head to `commit`, holding the order through `through`, and the
    /// journal's order goes on from `sequence`: every participant hears it.
    pub async fn sealed(&self, block: Place, commit: CommitId, through: u64, sequence: u64) {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.sessions.get_mut(&block) else {
            return;
        };
        session.tip = sequence;
        let participants = session.state.clients();
        self.hub
            .send_every(&participants, &Push::Sealed {
                block,
                commit,
                through,
            })
            .await;
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
            .sessions
            .get(&block)
            .map(|session| session.state.clone())
            .unwrap_or_default()
    }

    /// Whether `client` is in `block`'s session and runs the build it speaks.
    pub async fn speaks(&self, block: Place, client: ClientId) -> bool {
        self.state(block).await.speaks(client)
    }

    pub async fn participants(&self, block: Place) -> Vec<ClientId> {
        self.state(block).await.clients()
    }

    /// The sessions of blocks in `organization`, each with its participants.
    pub async fn sessions_in(&self, organization: uuid::Uuid) -> Vec<(Place, Vec<ClientId>)> {
        self.sessions
            .lock()
            .await
            .sessions
            .iter()
            .filter(|(place, _)| place.organization == organization)
            .map(|(place, session)| (*place, session.state.clients()))
            .collect()
    }

    /// Whether `client` holds the lease of `block`'s session at the order's tip.
    pub async fn holds(&self, block: Place, client: ClientId) -> bool {
        self.state(block).await.is_holder(client)
    }
}
