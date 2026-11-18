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

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use be_block_client::{Admitted, Admitting, Event, Peer};
use be_block_protocol::{ClientId, JoinerChange, MembershipChange};
use be_sdk_api::blocks::{JoinedMember, MemberEvent, MemberSaid};
use uuid::Uuid;

use crate::seen::MemberNews;
use crate::worker::{Shared, Store, Tasks};

/// What a host component may leave untaken: a member that says more while it is this far behind
/// has its channel broken (it handshakes again) rather than an event dropped.
const EVENTS_KEPT: usize = 4096;

#[derive(Default)]
pub struct Hosting {
    /// Each held block's members, by the client they speak from. Which blocks are held is
    /// `Seen::hosted`, which the page reads.
    channels: HashMap<Uuid, HashMap<ClientId, Member>>,
    /// A check of the members against the signed logs is running, and whether another was
    /// asked for meanwhile, which it runs once more rather than start beside it.
    rechecking: bool,
    again: bool,
    /// How many times a check took the members to check (`checked_members`): an admit that saw
    /// it move was missed by a check, and asks for another.
    snapshots: u64,
    /// Numbers each admit, so an answer reaches only the handshake it answers.
    admits: u64,
    /// The blocks whose session this device claimed since its replica last joined it: the claim
    /// stays in the session's membership, so the server names this device host again once a
    /// later claimant goes, until the replica joins again (`release`) or leaves.
    claimed: std::collections::HashSet<Uuid>,
}

enum Member {
    /// Its first message is being checked (`Peer::admit`): this device's `admit`th. What it
    /// sent since (`next`, the newest only) is taken as its new handshake once the check ends:
    /// a member speaks again only after a handshake of its own, so the last thing it sent is
    /// the one it waits on.
    Answering {
        admit: u64,
        next: Option<Vec<u8>>,
    },
    /// Answered: admitted once its first message on the channel opens.
    Admitting(Box<Admitting>),
    Admitted(Box<Admitted>),
}

/// What `block`'s host component has yet to take.
#[cfg(test)]
pub fn queued(shared: &Shared, block: Uuid) -> Vec<MemberEvent> {
    shared
        .member_news
        .of(block)
        .map(|(news, _)| news.into_iter().map(event_of).collect())
        .unwrap_or_default()
}

/// The member event the app hears for `news`.
pub fn event_of(news: &MemberNews) -> MemberEvent {
    match news {
        MemberNews::Joined {
            member,
            account,
            level,
        } => MemberEvent::Joined(JoinedMember {
            member: *member,
            account: account.into_bytes(),
            level: access_of(*level),
        }),
        MemberNews::Said { member, payload } => MemberEvent::Said(MemberSaid {
            member: *member,
            payload: payload.clone(),
        }),
        MemberNews::Left(member) => MemberEvent::Left(*member),
    }
}

fn queue(shared: &mut Shared, block: Uuid, news: MemberNews) {
    shared.seen.member_news.push(block, news);
}

/// Tells the host component what `member` said, unless it is too far behind to: then the
/// member's channel breaks, and it hears the member go.
fn said(shared: &mut Shared, block: Uuid, member: ClientId, payload: Vec<u8>) {
    let behind = shared.member_news.len(block) >= EVENTS_KEPT;
    match behind {
        true => drop_member(shared, block, member),
        false => queue(shared, block, MemberNews::Said { member, payload }),
    }
}

/// Drops `member`'s channel on `block`, telling the host component it left once it was in.
fn drop_member(shared: &mut Shared, block: Uuid, member: ClientId) {
    let gone = shared
        .hosting
        .channels
        .get_mut(&block)
        .and_then(|channels| channels.remove(&member));
    if let Some(Member::Admitted(_)) = gone {
        queue(shared, block, MemberNews::Left(member));
    }
}

/// Drops `member`'s channel on `block` as `drop_member` does, only while it still speaks as
/// `account` with `public`: one admitted again since under the same client stays.
fn drop_checked(
    shared: &mut Shared,
    block: Uuid,
    member: ClientId,
    account: Uuid,
    public: &be_block_keys::RecoveryPublic,
) {
    let same = match shared
        .hosting
        .channels
        .get(&block)
        .and_then(|channels| channels.get(&member))
    {
        Some(Member::Admitted(admitted)) => {
            admitted.account == account && admitted.public == *public
        }
        Some(Member::Admitting(admitting)) => {
            admitting.account() == account && admitting.public() == *public
        }
        Some(Member::Answering { .. }) | None => false,
    };
    if same {
        drop_member(shared, block, member);
    }
}

fn joined(member: ClientId, admitted: &Admitted) -> MemberNews {
    MemberNews::Joined {
        member,
        account: admitted.account,
        level: admitted.level,
    }
}

fn lock(shared: &Arc<Mutex<Shared>>) -> std::sync::MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Who hosts `block`'s session, as the server last said: no one, this device (`Some(true)`),
/// or another. Hosting no longer, every channel goes, and what no component took of it.
pub fn host_heard(shared: &Arc<Mutex<Shared>>, block: Uuid, host: Option<bool>) {
    let mut held = lock(shared);
    let mut changed = held.seen.session_hosts.insert(block, host) != Some(host);
    match host == Some(true) {
        true => changed |= held.seen.hosted.insert(block).is_none(),
        false => changed |= stop_hosting(&mut held, block),
    }
    if changed {
        held.seen.leases_changed += 1;
        crate::changes::wake();
    }
}

/// This device's replica of `block` left its session: it hosts it no longer, and its claim went
/// with it.
pub fn left(shared: &Arc<Mutex<Shared>>, block: Uuid) {
    let mut held = lock(shared);
    held.hosting.claimed.remove(&block);
    let mut changed = held.seen.session_hosts.remove(&block).is_some();
    changed |= stop_hosting(&mut held, block);
    if changed {
        held.seen.leases_changed += 1;
        crate::changes::wake();
    }
}

fn stop_hosting(held: &mut Shared, block: Uuid) -> bool {
    held.hosting.channels.remove(&block);
    held.seen.member_news.clear(block);
    held.seen.hosted.remove(&block).is_some()
}

/// A host component starts on `block`: it claims the session's lease (plans/hosted_blocks.md,
/// step 4d2), so this device hosts it, and hears every member already admitted. That it hosts
/// comes as every change of host does, on the session's events (`heard`), in order with a
/// claim another device makes after it; the answer only records the claim. Refused (it left
/// the session, or may no longer edit), nothing changes.
pub fn start(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks, block: Uuid) {
    restart(shared, block);
    if lock(shared).seen.hosted.contains(&block) {
        return;
    }
    // Recorded as it is sent, so a component that stops before the answer still lets it go,
    // and again once answered, for a join the answer came after. A claim refused needs no
    // release; one more join of the session is all the record costs then.
    lock(shared).hosting.claimed.insert(block);
    let claimed = peer.connection().claim(block);
    let shared = Arc::clone(shared);
    tasks.spawn(async move {
        if claimed.await.is_ok() {
            lock(&shared).hosting.claimed.insert(block);
        }
    });
}

/// No component runs on `block` here any more: whether its replica must join the session again
/// to drop a claim it made since it last joined (`Replicas::rejoin_block`), which it then has
/// no longer.
pub fn release(shared: &Arc<Mutex<Shared>>, block: Uuid) -> bool {
    lock(shared).hosting.claimed.remove(&block)
}

/// A host component starts on `block`: it hears every member already admitted join, and none of
/// what an earlier component left untaken, in one step, so nothing a member says comes before
/// it is heard to join.
fn restart(shared: &Arc<Mutex<Shared>>, block: Uuid) {
    let mut held = lock(shared);
    held.seen.member_news.clear(block);
    let again: Vec<MemberNews> = held
        .hosting
        .channels
        .get(&block)
        .into_iter()
        .flatten()
        .filter_map(|(member, held)| match held {
            Member::Admitted(admitted) => Some(joined(*member, admitted)),
            _ => None,
        })
        .collect();
    for news in again {
        queue(&mut held, block, news);
    }
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
    let sent = peer
        .connection()
        .send_host(block, Some(said.member), sealed);
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
        Some(Event::Host {
            block,
            host,
            ours,
            joined,
        }) => {
            if *joined {
                lock(shared).hosting.claimed.remove(block);
            }
            host_heard(shared, *block, host.map(|_| *ours));
        }
        Some(Event::Joiner {
            block,
            change: JoinerChange::Left(member),
        }) => drop_member(&mut lock(shared), *block, *member),
        Some(Event::Membership { block, entry }) => {
            if matches!(entry.change, MembershipChange::Left) {
                drop_member(&mut lock(shared), *block, entry.client);
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
    let mut guard = lock(shared);
    let held = &mut *guard;
    if !held.seen.hosted.contains(&block) {
        return;
    }
    let channels = held.hosting.channels.entry(block).or_default();
    match channels.remove(&from) {
        None => {}
        // Still being checked: what it sends meanwhile is answered once the check ends.
        Some(Member::Answering { admit, .. }) => {
            channels.insert(
                from,
                Member::Answering {
                    admit,
                    next: Some(payload.to_vec()),
                },
            );
            return;
        }
        Some(Member::Admitting(admitting)) => match admitting.confirm(payload) {
            Ok((admitted, said)) => {
                let news = joined(from, &admitted);
                channels.insert(from, Member::Admitted(Box::new(admitted)));
                queue(held, block, news);
                if !said.is_empty() {
                    self::said(held, block, from, said);
                }
                return;
            }
            // Not the member's first message on the channel: a new handshake, taken below.
            Err(_) => {}
        },
        Some(Member::Admitted(mut admitted)) => match admitted.channel.open(payload) {
            Ok(said) => {
                channels.insert(from, Member::Admitted(admitted));
                self::said(held, block, from, said);
                return;
            }
            // Out of place or forged: the channel is broken, and what came is most likely the
            // member's new handshake, taken below.
            Err(_) => queue(held, block, MemberNews::Left(from)),
        },
    }
    drop(guard);
    answer(peer, shared, tasks, block, from, payload.to_vec());
}

/// Checks `payload` as `from`'s handshake (`Peer::admit`) and answers it when it is let in. What
/// the member sent while it was checked is checked next instead, as its newer handshake.
fn answer(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    tasks: &Tasks,
    block: Uuid,
    from: ClientId,
    payload: Vec<u8>,
) {
    let (admit, snapshot) = {
        let mut held = lock(shared);
        let hosting = &mut held.hosting;
        hosting.admits += 1;
        let admit = hosting.admits;
        hosting
            .channels
            .entry(block)
            .or_default()
            .insert(from, Member::Answering { admit, next: None });
        (admit, hosting.snapshots)
    };
    let (peer, shared, tasks) = (Arc::clone(peer), Arc::clone(shared), tasks.clone());
    tasks.clone().spawn(async move {
        let admitted = peer.admit(block, &payload).await;
        let mut held = lock(&shared);
        let missed = held.hosting.snapshots != snapshot;
        // Gone with the lease, or answering another handshake by now: nothing to do.
        let Some(channels) = held.hosting.channels.get_mut(&block) else {
            return;
        };
        let next = match channels.get_mut(&from) {
            Some(Member::Answering {
                admit: current,
                next,
            }) if *current == admit => next.take(),
            _ => return,
        };
        if let Some(next) = next {
            drop(held);
            return answer(&peer, &shared, &tasks, block, from, next);
        }
        match admitted {
            Ok((admitting, answer)) => {
                channels.insert(from, Member::Admitting(Box::new(admitting)));
                drop(held);
                // A check that ran while this one did skipped the member: it is checked again.
                if missed {
                    recheck(&peer, &shared, &tasks);
                }
                let _ = peer.connection().send_host(block, Some(from), answer).await;
            }
            Err(_) => {
                channels.remove(&from);
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
                    drop_checked(&mut lock(&shared), block, member, account, &public);
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
    let mut held = lock(shared);
    held.hosting.snapshots += 1;
    held.hosting
        .channels
        .iter()
        .flat_map(|(block, channels)| {
            channels
                .iter()
                .filter_map(move |(member, held)| match held {
                    Member::Admitted(admitted) => {
                        Some((*block, *member, admitted.account, admitted.public))
                    }
                    Member::Admitting(admitting) => {
                        Some((*block, *member, admitting.account(), admitting.public()))
                    }
                    Member::Answering { .. } => None,
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
