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
