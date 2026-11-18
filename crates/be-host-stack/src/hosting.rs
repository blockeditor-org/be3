//! The host's side of a hosted block's channel on this device (plans/hosted_blocks.md,
//! "Hosting"): while this device's replica holds a block's lease, the members that speak on its
//! host channel are admitted here (`Peer::admit`, then `Admitting::confirm` once their first
//! message opens), and the host component running on the block hears them come, speak and go
//! as `member-event`s (`take_member_events`), and answers one with `tell_member`.
//!
//! A member's channel goes when it leaves the session, when a message on it does not open (out
//! of place or forged: it handshakes again), when the signed logs no longer let it in, checked
//! again on every access change (`Peer::still_admits`), and every channel goes when this device
//! gives the lease up.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use be_block_client::{Admitted, Admitting, Event, Peer};
use be_block_protocol::{ClientId, JoinerChange, MembershipChange};
use be_sdk_api::blocks::{JoinedMember, MemberEvent, MemberSaid};
use uuid::Uuid;

use crate::worker::{Shared, Store, Tasks};

/// Members a host component has not taken yet: past this, the oldest go.
const EVENTS_KEPT: usize = 4096;

#[derive(Default)]
pub struct Hosting {
    /// The blocks whose session's lease this device's replica holds.
    held: HashSet<Uuid>,
    /// Each held block's members, by the client they speak from.
    channels: HashMap<Uuid, HashMap<ClientId, Member>>,
    /// What each block's host component has yet to hear.
    events: HashMap<Uuid, VecDeque<MemberEvent>>,
}

enum Member {
    /// Its first message is being checked (`Peer::admit`).
    Answering,
    /// Answered: admitted once its first message on the channel opens.
    Admitting(Box<Admitting>),
    Admitted(Box<Admitted>),
}

impl Hosting {
    /// Whether this device's replica holds `block`'s lease.
    pub fn holds(&self, block: Uuid) -> bool {
        self.held.contains(&block)
    }

    /// What `block`'s host component has yet to take.
    #[cfg(test)]
    pub fn queued(&self, block: Uuid) -> Vec<MemberEvent> {
        self.events
            .get(&block)
            .map(|events| events.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn tell(&mut self, block: Uuid, event: MemberEvent) {
        let queue = self.events.entry(block).or_default();
        queue.push_back(event);
        if queue.len() > EVENTS_KEPT {
            queue.pop_front();
        }
        crate::changes::wake();
    }

    /// Drops `member`'s channel on `block`, telling the host component it left once it was in.
    fn drop_member(&mut self, block: Uuid, member: ClientId) {
        let gone = self
            .channels
            .get_mut(&block)
            .and_then(|channels| channels.remove(&member));
        if let Some(Member::Admitted(_)) = gone {
            self.tell(block, MemberEvent::Left(member));
        }
    }
}

fn lock(shared: &Arc<Mutex<Shared>>) -> std::sync::MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// This device's replica holds `block`'s lease (`holder`) or no longer does: every channel goes
/// with the lease.
pub fn holding(shared: &Arc<Mutex<Shared>>, block: Uuid, holder: bool) {
    let mut held = lock(shared);
    let hosting = &mut held.hosting;
    match holder {
        true => {
            hosting.held.insert(block);
        }
        false => {
            if !hosting.held.remove(&block) {
                return;
            }
            let members: Vec<ClientId> = hosting
                .channels
                .get(&block)
                .map(|channels| channels.keys().copied().collect())
                .unwrap_or_default();
            for member in members {
                hosting.drop_member(block, member);
            }
            hosting.channels.remove(&block);
        }
    }
}

/// What the host component of `block` has yet to hear, taken.
pub fn take_events(shared: &Arc<Mutex<Shared>>, block: Uuid) -> Vec<MemberEvent> {
    lock(shared)
        .hosting
        .events
        .remove(&block)
        .map(Vec::from)
        .unwrap_or_default()
}

/// Seals `payload` on `member`'s channel and sends it: nothing when it is no admitted member.
/// Sent as it is sealed, so the channel's messages leave in order.
pub fn tell(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    tasks: &Tasks,
    block: Uuid,
    said: MemberSaid,
) {
    let mut held = lock(shared);
    let sealed = match held
        .hosting
        .channels
        .get_mut(&block)
        .and_then(|channels| channels.get_mut(&said.member))
    {
        Some(Member::Admitted(admitted)) => admitted.channel.seal(&said.payload),
        _ => return,
    };
    drop(held);
    let sent = peer.connection().send_host(block, Some(said.member), sealed);
    tasks.spawn(async move {
        // A refused send is not sent again sealed anew: the member finds the gap and
        // handshakes again.
        let _ = sent.await;
    });
}

/// Takes what the server told of a hosted block's session: a member's message on the channel,
/// a member leaving, or an access change, after which every admitted member is checked again.
pub fn heard(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    tasks: &Tasks,
    event: Option<&Event>,
) {
    match event {
        Some(Event::Hosted {
            block,
            from,
            payload,
            ..
        }) => hosted(peer, shared, tasks, *block, *from, payload),
        Some(Event::Joiner {
            block,
            change: JoinerChange::Left(member),
        }) => lock(shared).hosting.drop_member(*block, *member),
        Some(Event::Membership { block, entry }) => {
            if matches!(entry.change, MembershipChange::Left) {
                lock(shared).hosting.drop_member(*block, entry.client);
            }
        }
        Some(Event::KeysChanged | Event::Roots | Event::TreeAdvanced { .. }) | None => {
            recheck(peer, shared, tasks)
        }
        Some(_) => {}
    }
}

fn hosted(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    tasks: &Tasks,
    block: Uuid,
    from: ClientId,
    payload: &[u8],
) {
    let mut held = lock(shared);
    let hosting = &mut held.hosting;
    if !hosting.holds(block) {
        return;
    }
    let channels = hosting.channels.entry(block).or_default();
    match channels.remove(&from) {
        None => {
            channels.insert(from, Member::Answering);
            drop(held);
            let (peer, shared, payload) = (Arc::clone(peer), Arc::clone(shared), payload.to_vec());
            tasks.spawn(async move {
                let admitted = peer.admit(block, &payload).await;
                let mut held = lock(&shared);
                let hosting = &mut held.hosting;
                let answering = hosting
                    .channels
                    .get(&block)
                    .is_some_and(|channels| matches!(channels.get(&from), Some(Member::Answering)));
                let channels = hosting.channels.entry(block).or_default();
                match admitted {
                    Ok((admitting, answer)) if answering => {
                        channels.insert(from, Member::Admitting(Box::new(admitting)));
                        drop(held);
                        let _ = peer.connection().send_host(block, Some(from), answer).await;
                    }
                    _ => {
                        if answering {
                            channels.remove(&from);
                        }
                    }
                }
            });
        }
        // Still being checked: what it sends before its answer is dropped.
        Some(Member::Answering) => {
            channels.insert(from, Member::Answering);
        }
        Some(Member::Admitting(admitting)) => {
            if let Ok((admitted, said)) = admitting.confirm(payload) {
                let joined = MemberEvent::Joined(JoinedMember {
                    member: from,
                    account: admitted.account.into_bytes(),
                    level: access_of(admitted.level),
                });
                channels.insert(from, Member::Admitted(Box::new(admitted)));
                hosting.tell(block, joined);
                if !said.is_empty() {
                    hosting.tell(block, said_by(from, said));
                }
            }
        }
        Some(Member::Admitted(mut admitted)) => match admitted.channel.open(payload) {
            Ok(said) => {
                channels.insert(from, Member::Admitted(admitted));
                hosting.tell(block, said_by(from, said));
            }
            // Out of place or forged: the channel is broken, and the member handshakes again.
            Err(_) => hosting.tell(block, MemberEvent::Left(from)),
        },
    }
}

fn said_by(member: ClientId, payload: Vec<u8>) -> MemberEvent {
    MemberEvent::Said(MemberSaid { member, payload })
}

/// Checks every admitted member of every block this device hosts against the signed logs again
/// (`Peer::still_admits`), and drops one they no longer let in.
fn recheck(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks) {
    let members: Vec<(Uuid, ClientId, Uuid, be_block_keys::RecoveryPublic)> = lock(shared)
        .hosting
        .channels
        .iter()
        .flat_map(|(block, channels)| {
            channels.iter().filter_map(move |(member, held)| match held {
                Member::Admitted(admitted) => {
                    Some((*block, *member, admitted.account, admitted.public))
                }
                _ => None,
            })
        })
        .collect();
    if members.is_empty() {
        return;
    }
    let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
    tasks.spawn(async move {
        for (block, member, account, public) in members {
            // Unreachable, the member stays until the logs can be read.
            if let Ok(None) = peer.still_admits(block, account, &public).await {
                lock(&shared).hosting.drop_member(block, member);
            }
        }
    });
}

fn access_of(level: be_block_keys::Level) -> be_sdk_api::AccessLevel {
    use be_block_keys::Level;
    use be_sdk_api::AccessLevel;
    match level {
        Level::None => AccessLevel::None,
        Level::KnowExists => AccessLevel::KnowExists,
        Level::Join => AccessLevel::Join,
        Level::View => AccessLevel::View,
        Level::Edit => AccessLevel::Edit,
        Level::Manage => AccessLevel::Manage,
    }
}
