use std::collections::HashMap;

use be_commit::{CommitId, now_milliseconds};
use be_protocol::{ClientId, ServerMessage, SessionLog, SessionSeal, SessionState};
use be_session::{Claim, Lease, Sequencer, Submission};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::watch::WatchHub;

struct Live {
    lease: Lease,
    sequencer: Option<Sequencer>,
}

impl Live {
    fn left(&mut self) {
        if self.lease.is_empty() {
            self.sequencer = None;
        }
    }
}

pub struct SessionRegistry {
    sessions: Mutex<HashMap<Uuid, Live>>,
    lease: i64,
}

pub enum Submitted {
    Sequenced,
    Duplicate,
    Refused,
}

impl SessionRegistry {
    pub fn new(lease: std::time::Duration) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            lease: i64::try_from(lease.as_millis()).unwrap_or(i64::MAX),
        }
    }

    fn live(&self) -> Live {
        Live {
            lease: Lease::lasting(self.lease),
            sequencer: None,
        }
    }

    pub async fn join(
        &self,
        block: Uuid,
        client: ClientId,
        tag: [u8; 32],
        head: Option<CommitId>,
    ) -> SessionState {
        let mut sessions = self.sessions.lock().await;
        let live = sessions.entry(block).or_insert_with(|| self.live());
        live.lease.join(client, tag, now_milliseconds());
        live.sequencer.get_or_insert_with(|| Sequencer::new(head));
        live.lease.state().clone()
    }

    pub async fn leave(&self, block: Uuid, client: ClientId) -> Option<SessionState> {
        let mut sessions = self.sessions.lock().await;
        let live = sessions.get_mut(&block)?;
        live.lease.leave(client, now_milliseconds());
        live.left();
        let state = live.lease.state().clone();
        if forgettable(&live.lease) {
            sessions.remove(&block);
        }
        Some(state)
    }

    pub async fn leave_all(&self, client: ClientId) -> Vec<(Uuid, SessionState)> {
        let mut sessions = self.sessions.lock().await;
        let now = now_milliseconds();
        let mut changed = Vec::new();
        let mut empty = Vec::new();
        for (block, live) in sessions.iter_mut() {
            if !live.lease.state().participants.contains(&client) {
                continue;
            }
            live.lease.leave(client, now);
            live.left();
            changed.push((*block, live.lease.state().clone()));
            if forgettable(&live.lease) {
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
        let live = sessions.entry(block).or_insert_with(|| self.live());
        let claim = live.lease.claim(client, generation, now_milliseconds());
        (claim, live.lease.state().clone())
    }

    pub async fn heartbeat(
        &self,
        block: Uuid,
        client: ClientId,
        generation: u64,
        seal: SessionSeal,
    ) -> Option<SessionState> {
        let mut sessions = self.sessions.lock().await;
        let live = sessions.get_mut(&block)?;
        if !live
            .lease
            .heartbeat(client, generation, seal.head, now_milliseconds())
        {
            return None;
        }
        if let Some(sequencer) = &mut live.sequencer {
            sequencer.sealed(seal);
        }
        Some(live.lease.state().clone())
    }

    pub async fn submit(
        &self,
        hub: &WatchHub,
        block: Uuid,
        client: ClientId,
        submission: Submission,
        barrier: bool,
    ) -> Submitted {
        let mut sessions = self.sessions.lock().await;
        let Some(live) = sessions.get_mut(&block) else {
            return Submitted::Refused;
        };
        if !live.lease.speaks(client) {
            return Submitted::Refused;
        }
        let Some(sequencer) = &mut live.sequencer else {
            return Submitted::Refused;
        };
        let Some(op) =
            sequencer.submit(submission.id, submission.base, barrier, submission.payload)
        else {
            return Submitted::Duplicate;
        };
        let participants = live.lease.state().participants.clone();
        hub.send_all(&participants, 0, &ServerMessage::Sequenced { block, op })
            .await;
        Submitted::Sequenced
    }

    pub async fn log(&self, block: Uuid, client: ClientId, origin: u64) -> Option<SessionLog> {
        let sessions = self.sessions.lock().await;
        let live = sessions.get(&block)?;
        if !live.lease.state().participants.contains(&client) {
            return None;
        }
        live.sequencer
            .as_ref()
            .map(|sequencer| sequencer.log(origin))
    }

    pub async fn blocks_of(&self, client: ClientId) -> Vec<Uuid> {
        self.sessions
            .lock()
            .await
            .iter()
            .filter(|(_, live)| live.lease.state().participants.contains(&client))
            .map(|(block, _)| *block)
            .collect()
    }

    pub async fn state(&self, block: Uuid) -> SessionState {
        self.sessions
            .lock()
            .await
            .get(&block)
            .map(|live| live.lease.state().clone())
            .unwrap_or_default()
    }

    pub async fn participants(&self, block: Uuid) -> Vec<ClientId> {
        self.sessions
            .lock()
            .await
            .get(&block)
            .map(|live| live.lease.state().participants.clone())
            .unwrap_or_default()
    }
}

fn forgettable(lease: &Lease) -> bool {
    lease.is_empty() && lease.clean_at().is_none()
}
