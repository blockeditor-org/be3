//! Tentative layers previewed across the session (`SessionMessage::Held`): this replica tells
//! its peers its own layer as it grows (`Told`), and shows each peer's as a layer of its own.

use std::collections::BTreeMap;
use std::ops::Bound;

use be_block_content::{LiveEdit, Merge};
use be_block_protocol::ClientId;
use be_block_session::SessionMessage;

use super::{LARGEST_RELAYED_OPERATION, Replica};
use crate::Recorder;

/// How this replica's own tentative layer reaches its peers.
#[derive(Default)]
pub(super) struct Told {
    /// The number of the layer held last, which each `Held` names.
    layer: u64,
    /// How many of the layer's edits the peers were told, none while they hold none of it.
    edits: Option<u64>,
    /// The layer changed otherwise than by an edit on top, and is told whole once the job
    /// that changed it is done.
    changed: bool,
    /// The layer is too large to relay: peers do not preview it until it ends.
    unpreviewed: bool,
    /// Edits told as appends since the layer was last told whole: past `APPENDS_BETWEEN_WHOLE`
    /// the next goes whole, absorbed, so what a peer holds of a long drag stays a few edits.
    appended: u32,
    /// The layer and how many of its edits each peer that asked again (`HeldAgain`) was last
    /// told whole: it is told again only once the layer changed, whatever it asks.
    answered: BTreeMap<ClientId, (u64, u64)>,
}

/// How many edits a layer is told as appends before it is told whole again.
const APPENDS_BETWEEN_WHOLE: u32 = 16;

/// The most edits a peer's layer shows here: one past it, as one past
/// `LARGEST_RELAYED_OPERATION` bytes, stops its preview until its holder's next layer.
const LARGEST_PEER_LAYER: usize = 256;

impl<C, H> Replica<C, H>
where
    C: LiveEdit + Merge + Clone + Default,
    H: Recorder<C>,
{
    /// A tentative layer starts: its peers drop any earlier one of this replica's when they
    /// hear of it.
    pub(super) fn start_layer(&mut self) {
        self.told = Told {
            layer: self.told.layer + 1,
            edits: Some(0),
            changed: false,
            unpreviewed: false,
            appended: 0,
            answered: std::mem::take(&mut self.told.answered),
        };
    }

    /// Tells the peers the edit just held on top of the layer.
    pub(super) fn tell_held_edit(&mut self) {
        let edits = self.tentative.len() as u64;
        if self.told.unpreviewed {
            return;
        }
        let Some(operation) = self.tentative.last().map(|held| &held.operation) else {
            return;
        };
        let operation = C::encode_operation(operation);
        if self.told.changed
            || self.told.edits != Some(edits - 1)
            || self.told.appended >= APPENDS_BETWEEN_WHOLE
            || operation.len() > LARGEST_RELAYED_OPERATION
        {
            self.told.changed = true;
            return;
        }
        self.told.edits = Some(edits);
        // The first edit of a layer is the layer whole.
        self.told.appended = match edits {
            1 => 0,
            _ => self.told.appended + 1,
        };
        self.broadcast_held(SessionMessage::Held {
            layer: self.told.layer,
            origin: self.origin,
            from: edits - 1,
            to: edits,
            operations: vec![operation.into()],
            commits_through: None,
        });
    }

    /// The layer changed otherwise than by an edit held on top (a gesture of it cancelled,
    /// voided or carried over a restart): it is told whole, or dropped, after the job.
    pub(super) fn layer_changed(&mut self) {
        self.told.changed = true;
    }

    /// Tells the peers the layer whole, or drops it, if it changed during the job just run.
    pub(super) fn tell_layer(&mut self) {
        if !std::mem::take(&mut self.told.changed) {
            return;
        }
        match self.whole_layer() {
            Some(message) => {
                self.told.edits = Some(self.tentative.len() as u64);
                self.told.appended = 0;
                self.broadcast_held(message);
            }
            None => {
                self.told.unpreviewed = !self.tentative.is_empty();
                if self.told.edits.take().is_some() {
                    self.broadcast_held(self.dropped_layer());
                }
            }
        }
    }

    /// The layer is committing as the local edits through `through`, or none of it could go:
    /// the peers drop it once the order holds those edits, or at once. A layer that changed
    /// during the job goes whole with it.
    pub(super) fn tell_layer_committed(
        &mut self,
        whole: Option<SessionMessage>,
        through: Option<u64>,
    ) {
        let Some(edits) = self.told.edits.take() else {
            return;
        };
        self.told.changed = false;
        let message = match (through, whole) {
            (None, _) => self.dropped_layer(),
            (
                Some(through),
                Some(SessionMessage::Held {
                    layer,
                    origin,
                    from,
                    to,
                    operations,
                    ..
                }),
            ) => SessionMessage::Held {
                layer,
                origin,
                from,
                to,
                operations,
                commits_through: Some(through),
            },
            (Some(through), _) => SessionMessage::Held {
                layer: self.told.layer,
                origin: self.origin,
                from: edits,
                to: edits,
                operations: Vec::new(),
                commits_through: Some(through),
            },
        };
        self.broadcast_held(message);
    }

    /// What the layer is to tell if it is told whole now (`commit_layer` takes it before the
    /// layer leaves).
    pub(super) fn layer_to_tell(&self) -> Option<SessionMessage> {
        match self.told.changed && self.told.edits.is_some() {
            true => self.whole_layer(),
            false => None,
        }
    }

    /// The whole layer, its edits absorbed: none when it is empty, or too large for a peer to
    /// show (`LARGEST_PEER_LAYER` edits or `LARGEST_RELAYED_OPERATION` bytes).
    fn whole_layer(&self) -> Option<SessionMessage> {
        let mut absorbed: Vec<C::Op> = Vec::new();
        for held in &self.tentative {
            let leftover = match absorbed.last_mut() {
                Some(last) => C::absorb_operation(last, held.operation.clone()),
                None => Some(held.operation.clone()),
            };
            absorbed.extend(leftover);
        }
        if absorbed.is_empty() || absorbed.len() > LARGEST_PEER_LAYER {
            return None;
        }
        let operations: Vec<Vec<u8>> = absorbed.iter().map(C::encode_operation).collect();
        let size: usize = operations.iter().map(Vec::len).sum();
        if size > LARGEST_RELAYED_OPERATION {
            return None;
        }
        Some(SessionMessage::Held {
            layer: self.told.layer,
            origin: self.origin,
            from: 0,
            to: self.tentative.len() as u64,
            operations: operations.into_iter().map(Into::into).collect(),
            commits_through: None,
        })
    }

    fn dropped_layer(&self) -> SessionMessage {
        SessionMessage::Held {
            layer: self.told.layer,
            origin: self.origin,
            from: 0,
            to: 0,
            operations: Vec::new(),
            commits_through: None,
        }
    }

    /// Sends `client`, which joined, the layer whole.
    pub(super) fn tell_layer_to(&mut self, client: ClientId) {
        if self.told.edits.is_none() {
            return;
        }
        if let Some(message) = self.whole_layer() {
            self.send(Some(client), &message);
        }
    }

    /// `client` asked for the layer whole (`HeldAgain`): it is told once for each state of
    /// the layer, so asking again and again costs nothing.
    pub(super) fn answer_held_again(&mut self, client: ClientId) {
        let now = (self.told.layer, self.tentative.len() as u64);
        if self.told.edits.is_none() || self.told.answered.get(&client) == Some(&now) {
            return;
        }
        self.told.answered.insert(client, now);
        self.tell_layer_to(client);
    }

    /// Forgets what it answered peers that left.
    pub(super) fn forget_answered(&mut self, state: &be_block_protocol::SessionState) {
        self.told
            .answered
            .retain(|client, _| state.participants.iter().any(|held| held.client == *client));
    }

    fn broadcast_held(&mut self, message: SessionMessage) {
        if !self.alone() {
            self.broadcast(&message);
        }
    }
}

/// A peer's tentative layer, previewed here as it is told (`SessionMessage::Held`), under its
/// client and layer number: a layer committing stays beneath its holder's next until the order
/// holds it.
pub(super) struct PeerLayer<Op> {
    origin: u64,
    /// How many of the peer's held edits it stands for (`Held::to`).
    to: u64,
    operations: Vec<Op>,
    /// The encoded size of `operations`.
    bytes: usize,
    /// The peer's last local edit it goes out as, once it commits.
    commits_through: Option<u64>,
}

/// Each peer's layers previewed here, by client and layer number: the order they show in.
pub(super) type PeerLayers<Op> = BTreeMap<(ClientId, u64), PeerLayer<Op>>;

/// A `SessionMessage::Held` as heard (its fields, as the message names them).
pub(super) struct HeardHeld {
    pub(super) layer: u64,
    pub(super) origin: u64,
    pub(super) from: u64,
    pub(super) to: u64,
    pub(super) operations: Vec<serde_bytes::ByteBuf>,
    pub(super) commits_through: Option<u64>,
}

impl<C, H> Replica<C, H>
where
    C: LiveEdit + Merge + Clone + Default,
    H: Recorder<C>,
{
    /// Whether anything shows above this replica's unconfirmed edits: its own tentative layer
    /// or a peer's.
    pub(super) fn layered(&self) -> bool {
        !self.tentative.is_empty() || !self.peer_layers.is_empty()
    }

    /// How many edits the peers' layers show.
    pub(super) fn peer_edits(&self) -> usize {
        self.peer_layers
            .values()
            .map(|layer| layer.operations.len())
            .sum()
    }

    /// The edits shown above this replica's unconfirmed ones, in the order they show: the
    /// peers' layers, then its own.
    pub(super) fn layer_edits(&self) -> Vec<C::Op> {
        self.peer_layers
            .values()
            .flat_map(|layer| layer.operations.iter().cloned())
            .chain(self.tentative.iter().map(|held| held.operation.clone()))
            .collect()
    }

    /// Applies the layers to `content`, which shows what is beneath them.
    pub(super) fn apply_layers(&self, content: &mut C) {
        for layer in self.peer_layers.values() {
            for operation in &layer.operations {
                content.apply(operation);
            }
        }
        for held in &self.tentative {
            content.apply(&held.operation);
        }
    }

    /// What shows, but for the peers' layers: what this replica's own edits are recorded
    /// against for undo.
    pub(super) fn own_visible(&self) -> C {
        if self.peer_layers.is_empty() {
            return self.visible.clone();
        }
        let mut content = self.layer_free();
        for held in &self.tentative {
            content.apply(&held.operation);
        }
        content
    }

    /// What shows, composed afresh: the unconfirmed edits, then the layers.
    pub(super) fn composed(&self) -> C {
        let mut content = self.layer_free();
        self.apply_layers(&mut content);
        content
    }

    /// Every edit shown unsettled, in order, as the editors are told it after an edit
    /// beneath them all: each unsent operation then counts as one.
    pub(super) fn unsettled_edits(&mut self) -> Vec<C::Op> {
        let mut local: Vec<C::Op> = self
            .pending
            .iter()
            .filter_map(|sent| sent.operation.clone())
            .collect();
        self.in_flight = local.len() as u64;
        for (pending, _, weight, ..) in &mut self.unsent {
            local.push(pending.clone());
            *weight = 1;
        }
        local.extend(self.layer_edits());
        local
    }

    /// The peers' layers the order holds now, committed.
    pub(super) fn reached_layers(&self) -> Vec<(ClientId, u64)> {
        self.peer_layers
            .iter()
            .filter(|(_, layer)| self.reached(layer))
            .map(|(key, _)| *key)
            .collect()
    }

    /// Whether the order holds the edits `layer` goes out as, or passed over the entries that
    /// held them (below a barrier), so they never will be.
    fn reached(&self, layer: &PeerLayer<C::Op>) -> bool {
        layer.commits_through.is_some_and(|through| {
            let held = self.origins.get(&layer.origin).map(|(held, _)| *held);
            let passed = self.passed_origins.get(&layer.origin).copied();
            held.max(passed).is_some_and(|reached| reached >= through)
        })
    }

    /// Drops the layers the order holds now, in one landing.
    pub(super) fn drop_reached_layers(&mut self) {
        let reached = self.reached_layers();
        self.drop_layers(|key, _| reached.contains(key));
    }

    /// The order passed over `origin`'s edits through `through` (below a barrier): a layer
    /// that was to go out as them never will, and leaves, now or once it is told it commits.
    pub(super) fn passed_over_origin(&mut self, origin: u64, through: u64) {
        let passed = self.passed_origins.entry(origin).or_default();
        *passed = (*passed).max(through);
        self.drop_reached_layers();
    }

    /// `client`'s newest layer here.
    fn newest_layer(&self, client: ClientId) -> Option<u64> {
        self.peer_layers
            .range((client, 0)..=(client, u64::MAX))
            .next_back()
            .map(|((_, layer), _)| *layer)
    }

    /// `from` told its tentative layer (`SessionMessage::Held`).
    pub(super) fn heard_held(&mut self, from: ClientId, told: HeardHeld) {
        if from == self.client || !may_hold(&self.state, from) || !self.loaded {
            return;
        }
        if self.unreadable.is_some() || self.not_yet || self.diverged {
            return;
        }
        // An earlier layer than one shown, or one too large to show.
        if self
            .newest_layer(from)
            .is_some_and(|layer| layer > told.layer)
            || self
                .unpreviewed
                .get(&from)
                .is_some_and(|layer| *layer >= told.layer)
        {
            return;
        }
        let Ok(operations) = told
            .operations
            .iter()
            .map(|operation| C::decode_operation(operation))
            .collect::<Result<Vec<_>, _>>()
        else {
            self.incompatible = true;
            return self.drop_peer_layer(from);
        };
        let bytes: usize = told
            .operations
            .iter()
            .map(|operation| operation.len())
            .sum();
        let key = (from, told.layer);
        let appends = told.from > 0
            && self
                .peer_layers
                .get(&key)
                .is_some_and(|held| held.to == told.from);
        if told.from > 0 && !appends {
            // It holds another count of the layer's edits (it joined late, reloaded, or missed
            // a frame): it shows none of it, nor what the layer replaces, and asks for the
            // whole, once.
            self.drop_layers(|(client, layer), held| {
                *client == from && (*layer == told.layer || held.commits_through.is_none())
            });
            if self.asked_layers.get(&from) != Some(&told.layer) {
                self.asked_layers.insert(from, told.layer);
                self.send(Some(from), &SessionMessage::HeldAgain);
            }
            return;
        }
        if appends {
            if let Some(held) = self.peer_layers.get_mut(&key) {
                held.to = told.to;
                held.bytes += bytes;
                held.commits_through = told.commits_through.or(held.commits_through);
            }
            for operation in operations {
                self.land_peer_edit(key, operation);
            }
        } else {
            self.asked_layers.remove(&from);
            // Told whole, or dropped: what it showed of this layer leaves, and so does an
            // earlier one of the peer's not committing; one committing stays beneath this
            // until the order holds it.
            let before = self.unsettled();
            let replaced = self.take_layers(|(client, layer), held| {
                *client == from && (*layer == told.layer || held.commits_through.is_none())
            });
            // Dropped, or committing with no edit: nothing to show.
            if operations.is_empty() && (told.commits_through.is_none() || told.to == 0) {
                if replaced {
                    self.relay(before);
                }
                return;
            }
            self.peer_layers.insert(
                key,
                PeerLayer {
                    origin: told.origin,
                    to: told.to,
                    operations: match replaced {
                        true => operations.clone(),
                        false => Vec::new(),
                    },
                    bytes,
                    commits_through: told.commits_through,
                },
            );
            match replaced {
                true => self.relay(before),
                false => {
                    for operation in operations {
                        self.land_peer_edit(key, operation);
                    }
                }
            }
        }
        // Too large to show: shown no more until the peer's next layer.
        if self.peer_layers.get(&key).is_some_and(|held| {
            held.operations.len() > LARGEST_PEER_LAYER || held.bytes > LARGEST_RELAYED_OPERATION
        }) {
            self.unpreviewed.insert(from, told.layer);
            self.drop_layers(|held, _| *held == key);
        }
        // Told it commits once the order held it already.
        self.drop_reached_layers();
    }

    /// Shows `operation` on top of the layer `key`, beneath the layers above it.
    fn land_peer_edit(&mut self, key: (ClientId, u64), operation: C::Op) {
        let shown_beneath = self.peer_edits() == 0 && self.tentative.is_empty();
        let Some(layer) = self.peer_layers.get_mut(&key) else {
            return;
        };
        layer.operations.push(operation.clone());
        if shown_beneath {
            self.beneath = Some(self.visible.clone());
        }
        if !self.covered {
            return self.rebuild_visible();
        }
        let above: Vec<C::Op> = self
            .peer_layers
            .range((Bound::Excluded(key), Bound::Unbounded))
            .flat_map(|(_, layer)| layer.operations.iter().cloned())
            .chain(self.tentative.iter().map(|held| held.operation.clone()))
            .collect();
        // As unsettled after the push, which is what shows unsettled with it.
        let unsettled = self.unsettled();
        if above.is_empty() {
            self.visible.apply(&operation);
            return self.journal(
                super::Journaled::Edited {
                    operation,
                    unsettled,
                },
                None,
            );
        }
        // What shows beneath it: everything under the layers, then the layers up to its own,
        // that one as it was.
        let mut under = self.layer_free();
        for (held, layer) in self.peer_layers.range(..=key) {
            let shown = match *held == key {
                true => layer.operations.len() - 1,
                false => layer.operations.len(),
            };
            for operation in &layer.operations[..shown] {
                under.apply(operation);
            }
        }
        self.visible
            .apply_beneath(&mut under, &operation, &above, &mut Vec::new());
        self.journal(
            super::Journaled::Under {
                operation,
                unsettled,
                above: above.len() as u64,
            },
            None,
        );
    }

    /// Drops what this replica shows of `client`'s layers.
    pub(super) fn drop_peer_layer(&mut self, client: ClientId) {
        self.drop_layers(|(held, _), _| *held == client);
    }

    /// Drops the layers `which` picks, in one landing.
    fn drop_layers(&mut self, which: impl Fn(&(ClientId, u64), &PeerLayer<C::Op>) -> bool) {
        let before = self.unsettled();
        if self.take_layers(which) {
            self.relay(before);
        }
    }

    /// Takes the layers `which` picks away: whether any showed an edit.
    fn take_layers(&mut self, which: impl Fn(&(ClientId, u64), &PeerLayer<C::Op>) -> bool) -> bool {
        let mut shown = false;
        self.peer_layers.retain(|key, layer| {
            let taken = which(key, layer);
            shown |= taken && !layer.operations.is_empty();
            !taken
        });
        shown
    }

    /// The layers changed with no edit beneath them, `before` edits having shown unsettled:
    /// what shows is composed afresh, and the editors told what is unsettled now.
    fn relay(&mut self, before: u64) {
        if !self.covered {
            return self.rebuild_visible();
        }
        self.beneath = None;
        self.visible = self.composed();
        let local = self.unsettled_edits();
        self.journal(
            super::Journaled::Relaid {
                unsettled: before,
                local,
            },
            None,
        );
    }
}

/// Whether `client` may send a `Held`: only a participant that may edit, as the server takes
/// submissions only from them.
pub(super) fn may_hold(state: &be_block_protocol::SessionState, client: ClientId) -> bool {
    state
        .participants
        .iter()
        .any(|participant| participant.client == client && participant.may_edit)
}
