//! The host's side of a hosted block's channel on this device (plans/hosted_blocks.md,
//! "Hosting"): while this device's replica holds a block's lease, the members that speak on its
//! host channel are admitted here (`Peer::admit`, then `Admitting::confirm` once their first
//! message opens), and the host component running on the block hears them come, speak and go
//! as `member-event`s (`take_member_events`), and answers one with `tell_member`.
//!
//! A member's channel goes when it leaves the session (`joiner.left`, or a `left` membership
//! entry), when a message on it does not open (out of place or forged: the same message is then
//! taken as the member's new handshake), when the host component falls too far behind to hear
//! it, and when the signed logs no longer let it in, checked again (`Peer::still_admits`) on
//! `KeysChanged`, `Roots`, `TreeAdvanced` and a resynchronized connection, and after this
//! device's own change of the block's access or a ban. Every channel goes when this device gives
//! the lease up. A component that starts hears every member already admitted join again
//! (`restart`), and nothing a component before it left untaken.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use be_block_client::{Admitted, Admitting, Event, Peer};
use be_block_protocol::{ClientId, JoinerChange, MembershipChange};
use be_sdk_api::blocks::{JoinedMember, MemberEvent, MemberSaid};
use uuid::Uuid;

use crate::worker::{Shared, Store, Tasks};

/// What a host component may leave untaken: a member that says more while it is this far behind
/// has its channel broken (it handshakes again) rather than an event dropped.
const EVENTS_KEPT: usize = 4096;

/// Counts every event queued for any host component, so the runtime reads them only when there
/// are new ones (`events_revision`).
static EVENTS: AtomicU64 = AtomicU64::new(0);
/// Counts every change of which blocks this device holds the lease of (`lease_revision`).
static LEASES: AtomicU64 = AtomicU64::new(0);

/// How many events were ever queued for host components.
pub fn events_revision() -> u64 {
    EVENTS.load(Ordering::Acquire)
}

/// How many times the blocks whose lease this device holds changed.
pub fn lease_revision() -> u64 {
    LEASES.load(Ordering::Acquire)
}

#[derive(Default)]
pub struct Hosting {
    /// The blocks whose session's lease this device's replica holds.
    held: HashSet<Uuid>,
    /// Each held block's members, by the client they speak from.
    channels: HashMap<Uuid, HashMap<ClientId, Member>>,
    /// What each block's host component has yet to hear.
    events: HashMap<Uuid, VecDeque<MemberEvent>>,
    /// A check of the members against the signed logs is running, and whether another was
    /// asked for meanwhile, which it runs once more rather than start beside it.
    rechecking: bool,
    again: bool,
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
        self.events.entry(block).or_default().push_back(event);
        EVENTS.fetch_add(1, Ordering::AcqRel);
        crate::changes::wake();
    }

    /// Tells the host component what `member` said, unless it is too far behind to: then the
    /// member's channel breaks, and it hears the member go.
    fn said(&mut self, block: Uuid, member: ClientId, payload: Vec<u8>) {
        let behind = self
            .events
            .get(&block)
            .is_some_and(|events| events.len() >= EVENTS_KEPT);
        match behind {
            true => self.drop_member(block, member),
            false => self.tell(block, MemberEvent::Said(MemberSaid { member, payload })),
        }
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
/// with the lease, and what no component took of it.
pub fn holding(shared: &Arc<Mutex<Shared>>, block: Uuid, holder: bool) {
    let mut held = lock(shared);
    let hosting = &mut held.hosting;
    let changed = match holder {
        true => hosting.held.insert(block),
        false => {
            let changed = hosting.held.remove(&block);
            hosting.channels.remove(&block);
            hosting.events.remove(&block);
            changed
        }
    };
    if changed {
        LEASES.fetch_add(1, Ordering::AcqRel);
        crate::changes::wake();
    }
}

/// A host component starts on `block`: it hears every member already admitted join, and none
/// of what an earlier component left untaken.
pub fn restart(shared: &Arc<Mutex<Shared>>, block: Uuid) {
    let mut held = lock(shared);
    let hosting = &mut held.hosting;
    hosting.events.remove(&block);
    let joined: Vec<MemberEvent> = hosting
        .channels
        .get(&block)
        .into_iter()
        .flatten()
        .filter_map(|(member, held)| match held {
            Member::Admitted(admitted) => Some(MemberEvent::Joined(JoinedMember {
                member: *member,
                account: admitted.account.into_bytes(),
                level: access_of(admitted.level),
            })),
            _ => None,
        })
        .collect();
    for event in joined {
        hosting.tell(block, event);
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
/// Sent as it is sealed, so the channel's messages leave in order. The host's sends are never
/// held back by the server, so none needs sending again.
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
        let _ = sent.await;
    });
}

/// Takes what the server told of a hosted block's session: a member's message on the channel,
/// a member leaving, or an access change, after which every member is checked again.
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
        None => {}
        // Still being checked: what it sends before its answer is dropped.
        Some(Member::Answering) => {
            channels.insert(from, Member::Answering);
            return;
        }
        Some(Member::Admitting(admitting)) => match admitting.confirm(payload) {
            Ok((admitted, said)) => {
                let joined = MemberEvent::Joined(JoinedMember {
                    member: from,
                    account: admitted.account.into_bytes(),
                    level: access_of(admitted.level),
                });
                channels.insert(from, Member::Admitted(Box::new(admitted)));
                hosting.tell(block, joined);
                if !said.is_empty() {
                    hosting.said(block, from, said);
                }
                return;
            }
            // Not the member's first message on the channel: a new handshake, taken below.
            Err(_) => {}
        },
        Some(Member::Admitted(mut admitted)) => match admitted.channel.open(payload) {
            Ok(said) => {
                channels.insert(from, Member::Admitted(admitted));
                hosting.said(block, from, said);
                return;
            }
            // Out of place or forged: the channel is broken, and what came is most likely the
            // member's new handshake, taken below.
            Err(_) => hosting.tell(block, MemberEvent::Left(from)),
        },
    }
    held.hosting
        .channels
        .entry(block)
        .or_default()
        .insert(from, Member::Answering);
    drop(held);
    let (peer, shared, payload) = (Arc::clone(peer), Arc::clone(shared), payload.to_vec());
    tasks.spawn(async move {
        let admitted = peer.admit(block, &payload).await;
        let mut held = lock(&shared);
        let channels = held.hosting.channels.entry(block).or_default();
        let answering = matches!(channels.get(&from), Some(Member::Answering));
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

/// Checks every member admitted, or answered and not admitted yet, of every block this device
/// hosts against the signed logs again (`Peer::still_admits`), and drops one they no longer
/// let in. One check runs at a time; asked for while one runs, it runs once more after.
pub fn recheck(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks) {
    {
        let mut held = lock(shared);
        let hosting = &mut held.hosting;
        if hosting.channels.values().all(HashMap::is_empty) {
            return;
        }
        if hosting.rechecking {
            hosting.again = true;
            return;
        }
        hosting.rechecking = true;
    }
    let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
    tasks.spawn(async move {
        loop {
            let members = checked_members(&shared);
            for (block, member, account, public) in members {
                // Unreachable, the member stays until the logs can be read.
                if let Ok(None) = peer.still_admits(block, account, &public).await {
                    lock(&shared).hosting.drop_member(block, member);
                }
            }
            let mut held = lock(&shared);
            let hosting = &mut held.hosting;
            match std::mem::take(&mut hosting.again) {
                true => {}
                false => {
                    hosting.rechecking = false;
                    return;
                }
            }
        }
    });
}

/// The members to check again: each with its account and the key it spoke with.
fn checked_members(
    shared: &Arc<Mutex<Shared>>,
) -> Vec<(Uuid, ClientId, Uuid, be_block_keys::RecoveryPublic)> {
    lock(shared)
        .hosting
        .channels
        .iter()
        .flat_map(|(block, channels)| {
            channels.iter().filter_map(move |(member, held)| match held {
                Member::Admitted(admitted) => {
                    Some((*block, *member, admitted.account, admitted.public))
                }
                Member::Admitting(admitting) => {
                    Some((*block, *member, admitting.account(), admitting.public()))
                }
                Member::Answering => None,
            })
        })
        .collect()
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
