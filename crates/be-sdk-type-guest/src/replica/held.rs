//! Tentative layers previewed across the session (`SessionMessage::Held`): this replica tells
//! its peers its own layer as it grows (`Told`), and shows each peer's as a layer of its own.

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
}

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
            || operation.len() > LARGEST_RELAYED_OPERATION
        {
            self.told.changed = true;
            return;
        }
        self.told.edits = Some(edits);
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
    pub(super) fn tell_layer_committed(&mut self, whole: Option<SessionMessage>, through: Option<u64>) {
        let Some(edits) = self.told.edits.take() else {
            return;
        };
        self.told.changed = false;
        let message = match (through, whole) {
            (None, _) => self.dropped_layer(),
            (Some(through), Some(SessionMessage::Held {
                layer,
                origin,
                from,
                to,
                operations,
                ..
            })) => SessionMessage::Held {
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

    /// The whole layer, its edits absorbed: none when it is empty, or too large to relay.
    fn whole_layer(&self) -> Option<SessionMessage> {
        let mut absorbed: Vec<C::Op> = Vec::new();
        for held in &self.tentative {
            let leftover = match absorbed.last_mut() {
                Some(last) => C::absorb_operation(last, held.operation.clone()),
                None => Some(held.operation.clone()),
            };
            absorbed.extend(leftover);
        }
        if absorbed.is_empty() {
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

    /// Sends `client`, which joined or asked again, the layer whole.
    pub(super) fn tell_layer_to(&mut self, client: ClientId) {
        if self.told.edits.is_none() {
            return;
        }
        if let Some(message) = self.whole_layer() {
            self.send(Some(client), &message);
        }
    }

    fn broadcast_held(&mut self, message: SessionMessage) {
        if !self.alone() {
            self.broadcast(&message);
        }
    }
}

/// A peer's tentative layer, previewed here as it is told (`SessionMessage::Held`).
pub(super) struct PeerLayer<Op> {
    layer: u64,
    origin: u64,
    /// How many of the peer's held edits it stands for (`Held::to`).
    to: u64,
    operations: Vec<Op>,
    /// The peer's last local edit it goes out as, once it commits.
    commits_through: Option<u64>,
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

    /// The peers whose layers the order holds now, committed.
    pub(super) fn reached_layers(&self) -> Vec<ClientId> {
        self.peer_layers
            .iter()
            .filter(|(_, layer)| self.reached(layer))
            .map(|(client, _)| *client)
            .collect()
    }

    fn reached(&self, layer: &PeerLayer<C::Op>) -> bool {
        layer.commits_through.is_some_and(|through| {
            self.origins
                .get(&layer.origin)
                .is_some_and(|(held, _)| *held >= through)
        })
    }

    /// `from` told its tentative layer (`SessionMessage::Held`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn heard_held(
        &mut self,
        from: ClientId,
        layer: u64,
        origin: u64,
        start: u64,
        to: u64,
        operations: Vec<serde_bytes::ByteBuf>,
        commits_through: Option<u64>,
    ) {
        if from == self.client || !may_hold(&self.state, from) || !self.loaded {
            return;
        }
        if self.unreadable.is_some() || self.not_yet || self.diverged {
            return;
        }
        if self
            .peer_layers
            .get(&from)
            .is_some_and(|held| held.layer > layer)
        {
            return;
        }
        let Ok(operations) = operations
            .iter()
            .map(|operation| C::decode_operation(operation))
            .collect::<Result<Vec<_>, _>>()
        else {
            self.incompatible = true;
            return self.drop_peer_layer(from);
        };
        let held = self.peer_layers.get(&from);
        let appends = start > 0 && held.is_some_and(|held| held.layer == layer && held.to == start);
        if start > 0 && !appends {
            // It holds another count of the layer's edits (it joined late, reloaded, or held
            // an earlier layer): it shows none of it, and asks for the whole.
            if held.is_some_and(|held| held.layer < layer) {
                self.drop_peer_layer(from);
            }
            if self.asked_layers.get(&from) != Some(&layer) {
                self.asked_layers.insert(from, layer);
                self.send(Some(from), &SessionMessage::HeldAgain);
            }
            return;
        }
        if !appends {
            self.asked_layers.remove(&from);
            // Told whole, or dropped: what it held of the peer's leaves.
            let before = self.unsettled();
            let held_any = self
                .peer_layers
                .remove(&from)
                .is_some_and(|held| !held.operations.is_empty());
            if operations.is_empty() && commits_through.is_none() {
                if held_any {
                    self.relay(before);
                }
                return;
            }
            self.peer_layers.insert(
                from,
                PeerLayer {
                    layer,
                    origin,
                    to,
                    operations: match held_any {
                        true => operations.clone(),
                        false => Vec::new(),
                    },
                    commits_through,
                },
            );
            match held_any {
                true => self.relay(before),
                false => {
                    for operation in operations {
                        self.land_peer_edit(from, operation);
                    }
                }
            }
        } else {
            if let Some(held) = self.peer_layers.get_mut(&from) {
                held.to = to;
                held.commits_through = commits_through.or(held.commits_through);
            }
            for operation in operations {
                self.land_peer_edit(from, operation);
            }
        }
        // Told it commits once the order held it already.
        if self
            .peer_layers
            .get(&from)
            .is_some_and(|held| self.reached(held))
        {
            self.drop_peer_layer(from);
        }
    }

    /// Shows `operation` on top of `client`'s layer, beneath the layers above it.
    fn land_peer_edit(&mut self, client: ClientId, operation: C::Op) {
        let shown_beneath = self.peer_edits() == 0 && self.tentative.is_empty();
        let Some(layer) = self.peer_layers.get_mut(&client) else {
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
            .range(client + 1..)
            .flat_map(|(_, layer)| layer.operations.iter().cloned())
            .chain(self.tentative.iter().map(|held| held.operation.clone()))
            .collect();
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
        for (held, layer) in self.peer_layers.range(..=client) {
            let shown = match *held == client {
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

    /// Drops what this replica shows of `client`'s layer.
    pub(super) fn drop_peer_layer(&mut self, client: ClientId) {
        let before = self.unsettled();
        let held_any = self
            .peer_layers
            .remove(&client)
            .is_some_and(|held| !held.operations.is_empty());
        if held_any {
            self.relay(before);
        }
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
fn may_hold(state: &be_block_protocol::SessionState, client: ClientId) -> bool {
    state
        .participants
        .iter()
        .any(|participant| participant.client == client && participant.may_edit)
}
