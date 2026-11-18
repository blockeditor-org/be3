//! A Join member's side of a hosted block's channel on this device (plans/hosted_blocks.md,
//! "Hosting"): while an editor is open on a block its account may only join
//! (`opening-kind.join`), this device joins the block's session as a Join member, handshakes
//! with the account the server names as holding the lease (`Peer::join_host`), and keeps the
//! channel the host answers with. The editor hears it open, close and carry what the host says
//! (`channel-event`), and tells the host something with `tell_host`.
//!
//! The channel is made again, after `closed`, whenever the host changes (`session-event.host`),
//! the session is joined again (a reconnect or a resynchronized connection), or a message on it
//! does not open. What the editor tells the host leaves one message at a time, in order: one the
//! server holds back (`slow-down`, the host channel's rate limit) is sent again, the same bytes,
//! before any after it, so none arrives out of place.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use be_block_client::{ClientError, Event, Peer};
use be_block_keys::noise::Channel;
use be_block_protocol::{ErrorCode, SessionHost};
use be_sdk_api::blocks::ChannelEvent;
use uuid::Uuid;

use crate::seen::ChannelNews;
use crate::worker::{Shared, Store, Tasks};

/// How long a message the server held back waits before it is sent again.
const SLOWED: Duration = Duration::from_millis(250);

/// How many times a handshake that could not read the logs tries again.
const JOIN_HOST_TRIES: u32 = 3;

#[derive(Default)]
pub struct Joining {
    /// Each block an editor here joins, and its channel.
    blocks: HashMap<Uuid, Joined>,
    /// Numbers each start of a channel, so what answers an earlier one is dropped.
    attempts: u64,
}

/// The channel event the app hears for `news`.
pub fn event_of(news: &ChannelNews) -> ChannelEvent {
    match news {
        ChannelNews::Open(account) => ChannelEvent::Open(account.into_bytes()),
        ChannelNews::Closed => ChannelEvent::Closed,
        ChannelNews::Said(payload) => ChannelEvent::Said(payload.clone()),
    }
}

/// Tells the Join editors of `block` `news`, and which account hosts it while its channel is
/// open.
fn queue(shared: &mut Shared, block: Uuid, news: ChannelNews) {
    let seen = &mut shared.seen;
    match news {
        ChannelNews::Open(account) => {
            seen.channel_hosts.insert(block, account);
        }
        ChannelNews::Closed => {
            seen.channel_hosts.remove(&block);
        }
        ChannelNews::Said(_) => {}
    }
    seen.channel_news.push(block, news);
}

/// Starts `block`'s channel over: what was open closes, and what was not sent on it is dropped,
/// since a new channel numbers its messages afresh. Answers the new attempt.
fn restart(shared: &mut Shared, block: Uuid) -> Option<u64> {
    let joining = &mut shared.joining;
    joining.attempts += 1;
    let attempt = joining.attempts;
    let joined = joining.blocks.get_mut(&block)?;
    let was_open = matches!(joined.state, State::Open(_));
    joined.attempt = attempt;
    joined.state = State::Waiting;
    joined.outbox.clear();
    if was_open {
        queue(shared, block, ChannelNews::Closed);
    }
    Some(attempt)
}

struct Joined {
    /// This start of the block's channel.
    attempt: u64,
    /// Who holds the lease, as the server last said.
    host: Option<SessionHost>,
    /// This connection, as the session names it once joined.
    client: Option<u64>,
    state: State,
    /// What the editor told the host, sealed, not yet taken by the server: the first is being
    /// sent.
    outbox: VecDeque<Vec<u8>>,
    sending: bool,
}

enum State {
    /// Not in the session: joined at the next `step` once no replica of the block on this
    /// connection is open or leaving, whose leave would take the Join member out with it.
    Unjoined,
    /// Joining the session, or the session is joined and no host is online.
    Waiting,
    /// The handshake's first message is sent to `host`.
    Handshaking(Box<be_block_client::Joining>),
    Open(Box<Channel>),
}

impl Joining {
    fn current(&self, block: Uuid, attempt: u64) -> Option<&Joined> {
        self.blocks
            .get(&block)
            .filter(|joined| joined.attempt == attempt)
    }
}

fn lock(shared: &Arc<Mutex<Shared>>) -> std::sync::MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The blocks an editor here joins are now `blocks`: each new one joins its session, and each
/// no longer wanted leaves it.
pub fn want(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks, blocks: HashSet<Uuid>) {
    let leaves = {
        let mut held = lock(shared);
        let leaves: Vec<Uuid> = held
            .joining
            .blocks
            .keys()
            .filter(|block| !blocks.contains(block))
            .copied()
            .collect();
        for block in &leaves {
            held.joining.blocks.remove(block);
            held.seen.channel_news.clear(*block);
            held.seen.channel_hosts.remove(block);
        }
        let joining = &mut held.joining;
        let joins: Vec<Uuid> = blocks
            .into_iter()
            .filter(|block| !joining.blocks.contains_key(block))
            .collect();
        for block in joins {
            joining.blocks.insert(
                block,
                Joined {
                    attempt: 0,
                    host: None,
                    client: None,
                    state: State::Unjoined,
                    outbox: VecDeque::new(),
                    sending: false,
                },
            );
        }
        leaves
    };
    for block in leaves {
        let left = peer.connection().leave_session(block);
        tasks.spawn(async move {
            let _ = left.await;
        });
    }
}

/// Joins the session of each block not in it, unless `busy` says this connection's replica of
/// it is open or leaving: the replica's leave would take the Join member out too, since the
/// server knows one member per connection.
pub fn step(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    tasks: &Tasks,
    busy: impl Fn(Uuid) -> bool,
) {
    let unjoined: Vec<Uuid> = lock(shared)
        .joining
        .blocks
        .iter()
        .filter(|(block, joined)| matches!(joined.state, State::Unjoined) && !busy(**block))
        .map(|(block, _)| *block)
        .collect();
    for block in unjoined {
        join(peer, shared, tasks, block);
    }
}

/// Joins `block`'s session afresh, as a Join member, and handshakes with whoever hosts it.
fn join(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks, block: Uuid) {
    let Some(attempt) = restart(&mut lock(shared), block) else {
        return;
    };
    // A Join member runs no build of the type, so it names none: the server keeps the tag only
    // for participants.
    let joined = peer
        .connection()
        .join(peer.organization(), block, be_block_store::Hash::from_bytes([0; 32]));
    let (peer, shared, tasks) = (Arc::clone(peer), Arc::clone(shared), tasks.clone());
    tasks.clone().spawn(async move {
        // Refused or out of reach, it waits for a reconnect or the editor opening again.
        let Ok(joined) = joined.await else {
            return;
        };
        {
            let mut held = lock(&shared);
            let Some(current) = held
                .joining
                .blocks
                .get_mut(&block)
                .filter(|joined| joined.attempt == attempt)
            else {
                return;
            };
            current.host = joined.host;
            current.client = Some(joined.client);
        }
        handshake(&peer, &shared, &tasks, block, attempt);
    });
}

/// Sends the handshake's first message to whoever hosts `block`, when anyone does.
fn handshake(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks, block: Uuid, attempt: u64) {
    // No one hosts, or this connection does (an editor of the block that joined it as a
    // participant may hold its lease): no channel until another does.
    let Some(host) = lock(shared)
        .joining
        .current(block, attempt)
        .and_then(|joined| joined.host.filter(|host| Some(host.client) != joined.client))
    else {
        return;
    };
    let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
    tasks.spawn(async move {
        // The logs do not name the account as one that may host (a server's forgery, or a
        // lease handed on since, which the next `session-event.host` tells): it waits. Any
        // other failure (the logs out of reach) is tried again a few times while it is still
        // this attempt's.
        let mut tries = 0;
        let (joining, first) = loop {
            match peer.join_host(block, host.account).await {
                Ok(started) => break started,
                Err(ClientError::Forged(_)) => return,
                Err(_) if tries < JOIN_HOST_TRIES => {
                    tries += 1;
                    peer.connection().clock().sleep(SLOWED * 4).await;
                    if lock(&shared).joining.current(block, attempt).is_none() {
                        return;
                    }
                }
                Err(_) => return,
            }
        };
        {
            let mut held = lock(&shared);
            let Some(joined) = held
                .joining
                .blocks
                .get_mut(&block)
                .filter(|joined| joined.attempt == attempt)
            else {
                return;
            };
            joined.state = State::Handshaking(Box::new(joining));
        }
        loop {
            match peer.connection().send_host(block, None, first.clone()).await {
                Ok(()) => return,
                // Held back by the rate limit: the same message again, while it is still this
                // attempt's.
                Err(ClientError::Refused(ErrorCode::SlowDown, _)) => {
                    peer.connection().clock().sleep(SLOWED).await;
                    if lock(&shared).joining.current(block, attempt).is_none() {
                        return;
                    }
                }
                // Not taken: the session may have been lost (a replica's leave on this
                // connection, say), so it is joined again.
                Err(_) => return rejoin(&shared, block, attempt),
            }
        }
    });
}

/// Joins `block`'s session again at the next `step`, unless its channel was started over since
/// `attempt`.
fn rejoin(shared: &Arc<Mutex<Shared>>, block: Uuid, attempt: u64) {
    let mut held = lock(shared);
    if held.joining.current(block, attempt).is_none() {
        return;
    }
    restart(&mut held, block);
    if let Some(joined) = held.joining.blocks.get_mut(&block) {
        joined.state = State::Unjoined;
    }
    crate::changes::wake();
}

/// Seals `payload` on `block`'s channel and sends it after anything sent before it: nothing
/// while the channel is not open.
pub fn tell(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks, block: Uuid, payload: Vec<u8>) {
    {
        let mut held = lock(shared);
        let Some(joined) = held.joining.blocks.get_mut(&block) else {
            return;
        };
        let State::Open(channel) = &mut joined.state else {
            return;
        };
        let sealed = channel.seal(&payload);
        joined.outbox.push_back(sealed);
    }
    pump(peer, shared, tasks, block);
}

/// Sends what `block`'s outbox holds, one message at a time, unless that is already happening.
fn pump(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks, block: Uuid) {
    let attempt = {
        let mut held = lock(shared);
        let Some(joined) = held.joining.blocks.get_mut(&block) else {
            return;
        };
        if joined.sending || joined.outbox.is_empty() {
            return;
        }
        joined.sending = true;
        joined.attempt
    };
    let (peer, shared, tasks) = (Arc::clone(peer), Arc::clone(shared), tasks.clone());
    tasks.clone().spawn(async move {
        loop {
            let next = {
                let mut held = lock(&shared);
                let Some(joined) = held.joining.blocks.get_mut(&block) else {
                    return;
                };
                match (joined.attempt == attempt, joined.outbox.front()) {
                    (true, Some(next)) => next.clone(),
                    _ => {
                        joined.sending = false;
                        // Started over meanwhile: the new channel's messages go now.
                        let again = !joined.outbox.is_empty();
                        drop(held);
                        if again {
                            pump(&peer, &shared, &tasks, block);
                        }
                        return;
                    }
                }
            };
            match peer.connection().send_host(block, None, next).await {
                Ok(()) => {
                    let mut held = lock(&shared);
                    if let Some(joined) = held
                        .joining
                        .blocks
                        .get_mut(&block)
                        .filter(|joined| joined.attempt == attempt)
                    {
                        joined.outbox.pop_front();
                    }
                }
                Err(ClientError::Refused(ErrorCode::SlowDown, _)) => {
                    peer.connection().clock().sleep(SLOWED).await;
                }
                // Lost, maybe with the session: the host would break the channel at the next
                // message anyway, so the session is joined again and the channel made anew.
                Err(_) => {
                    {
                        let mut held = lock(&shared);
                        if let Some(joined) = held.joining.blocks.get_mut(&block) {
                            joined.sending = false;
                        }
                    }
                    rejoin(&shared, block, attempt);
                    return;
                }
            }
        }
    });
}

/// Starts `block`'s channel over with the same host, unless it was started over since `attempt`.
fn rehandshake(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks, block: Uuid, attempt: u64) {
    let restarted = {
        let mut held = lock(shared);
        match held.joining.current(block, attempt).is_some() {
            true => restart(&mut held, block),
            false => None,
        }
    };
    if let Some(attempt) = restarted {
        handshake(peer, shared, tasks, block, attempt);
    }
}

/// Takes what the server told of a joined block's session: who hosts it, and what the host
/// said on the channel. A resynchronized connection (`None`) joins every session again.
pub fn heard(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks, event: Option<&Event>) {
    match event {
        Some(Event::Host { block, host, .. }) => {
            let restarted = {
                let mut held = lock(shared);
                match held.joining.blocks.get_mut(block) {
                    // Not in the session yet: it joins at the next step, and handshakes with
                    // whoever hosts then.
                    Some(joined) if matches!(joined.state, State::Unjoined) => {
                        joined.host = *host;
                        None
                    }
                    Some(joined) if joined.host != *host => {
                        let attempt = restart(&mut held, *block);
                        if let Some(joined) = held.joining.blocks.get_mut(block) {
                            joined.host = *host;
                        }
                        attempt
                    }
                    _ => None,
                }
            };
            if let Some(attempt) = restarted {
                handshake(peer, shared, tasks, *block, attempt);
            }
        }
        Some(Event::Hosted {
            block,
            from,
            payload,
            ..
        }) => hosted(peer, shared, tasks, *block, *from, payload),
        None => reconnect(shared),
        Some(_) => {}
    }
}

/// Every joined block joins its session again, at the next `step`: after a reconnect, the
/// server holds none of them.
pub fn reconnect(shared: &Arc<Mutex<Shared>>) {
    let attempts: Vec<(Uuid, u64)> = lock(shared)
        .joining
        .blocks
        .iter()
        .map(|(block, joined)| (*block, joined.attempt))
        .collect();
    for (block, attempt) in attempts {
        rejoin(shared, block, attempt);
    }
}

fn hosted(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks, block: Uuid, from: u64, payload: &[u8]) {
    let mut guard = lock(shared);
    let held = &mut *guard;
    let Some(joined) = held.joining.blocks.get_mut(&block) else {
        return;
    };
    let Some(host) = joined.host.filter(|host| host.client == from) else {
        return;
    };
    let attempt = joined.attempt;
    match std::mem::replace(&mut joined.state, State::Waiting) {
        unanswering @ (State::Unjoined | State::Waiting) => joined.state = unanswering,
        State::Handshaking(handshaking) => match handshaking.finish(payload) {
            Ok(mut channel) => {
                // The first message on the channel shows the host this member is there (its
                // key confirmation), so it goes at once, ahead of anything the editor says.
                joined.outbox.push_front(channel.seal(&[]));
                joined.state = State::Open(Box::new(channel));
                queue(held, block, ChannelNews::Open(host.account));
                drop(guard);
                pump(peer, shared, tasks, block);
            }
            // Not this handshake's answer (a stray message of an earlier channel, say): it goes
            // on waiting for it.
            Err(_) => joined.state = State::Handshaking(handshaking),
        },
        State::Open(mut channel) => match channel.open(payload) {
            Ok(said) => {
                joined.state = State::Open(channel);
                queue(held, block, ChannelNews::Said(said));
            }
            // Out of place or forged: the channel is broken, and is made again.
            Err(_) => {
                queue(held, block, ChannelNews::Closed);
                drop(guard);
                rehandshake(peer, shared, tasks, block, attempt);
            }
        },
    }
}
