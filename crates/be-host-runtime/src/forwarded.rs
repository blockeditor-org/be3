//! The host's end of an app a root guest embeds (plans/host_as_platform_adapter.md, 4g7b1): the
//! editors the host opens in it and serves as it serves an app of its own, over frames the root
//! forwards (`be3:root`'s `host.forward`) and relays back (`host.forwarded`) unread. The root
//! runs the app itself (its hello, pacing, screens, input and presents); this end hears only what
//! the app sends on the editors' ops and those under them, so it serves block calls, host
//! services, the network, debug and the published interfaces with the app's manifest's rules,
//! as `Runtime` does. Step 5 moves the block stack into the root and deletes this end.

use std::sync::Arc;

use be_sdk_api::{
    DockUpdate, EditorInstanceId, ViewId, app::EditorEvent, embed::shell::Call as ShellCall,
    ops::OpError,
};
use be_sdk_manifest::AppManifest;
use uuid::Uuid;

use super::{
    HostChildStatus, InstanceRole,
    calls::{EditorCall, Heard, Outbound},
    instances::Instances,
    wire::Wire,
};
use be_host_base::host;

/// One app a root guest embeds, as its host serves it.
pub struct Forwarded {
    wire: Wire,
    instances: Instances,
}

impl Forwarded {
    /// Serves the app `app` with its manifest's grants: its extensions, its network, its block
    /// agents and sharing.
    pub fn new(app: &AppManifest) -> Self {
        let mut instances = Instances::default();
        instances.allow_network(app.network.clone());
        instances.allow_block_agents(app.block_agents);
        instances.allow_sharing(app.sharing);
        instances.set_app_id(app.identity.id.clone());
        Self {
            wire: Wire::granting(app.extensions.clone())
                .offering(super::published::offered)
                .relayed(),
            instances,
        }
    }

    /// Opens an editor of `role` as `instance`, for the account's device `client_id`, telling
    /// it the app's block types: the frames that open it, for the root to send the app.
    pub fn open(
        &mut self,
        instance: EditorInstanceId,
        role: InstanceRole,
        client_id: Uuid,
        block_types: &Arc<crate::AppCatalog>,
    ) -> Vec<Vec<u8>> {
        self.instances
            .open_forwarded(instance, role, client_id, block_types);
        self.step()
    }

    /// Tells the editor `instance` the statuses of the children it placed in its view `view`, as
    /// its root shows them: those that changed since the last call for that view, sent with
    /// the next `step`. A child left out is no longer reported.
    pub fn show_children(
        &mut self,
        instance: EditorInstanceId,
        view: ViewId,
        statuses: Vec<HostChildStatus>,
    ) {
        let told = self
            .instances
            .set_forwarded_statuses(instance, view, statuses);
        self.wire.send(told);
    }

    /// Tells the editor `instance` what its root saw of a child it placed (`child-focused`),
    /// sent with the next `step`.
    pub fn tell_placer(&mut self, instance: EditorInstanceId, event: EditorEvent) {
        self.wire.send(vec![Outbound::Event(instance, event)]);
    }

    /// Asks the editor `instance` to show the block `open` names, as be-host asks the editor
    /// that handles show requests (`show-request.block`), sent with the next `step`.
    pub fn show_block(&mut self, instance: EditorInstanceId, open: crate::BlockOpen) {
        self.instances.show_block(instance, open);
    }

    /// Hands the editor `instance` the dock updates its root's docks made for it
    /// (`be3:root`'s `host.dock`), which answer its `dock-watch` with the next `step`.
    pub fn dock(&mut self, instance: EditorInstanceId, updates: Vec<DockUpdate>) {
        self.wire.send(
            updates
                .into_iter()
                .map(|update| Outbound::Editor(instance, EditorCall::Dock(update)))
                .collect(),
        );
    }

    /// Takes the frames the root forwarded, in order, and serves what they ask: the frames that
    /// answer, for the root to relay. Why the app is to stop, if its frames broke the protocol.
    pub fn receive(&mut self, frames: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>, String> {
        let mut heard = Vec::new();
        for frame in frames {
            self.wire.receive(&frame, &mut heard)?;
        }
        for heard in heard {
            self.heard(heard);
        }
        Ok(self.step())
    }

    /// Sends the app what changed for its editors since: their blocks, searches and versions,
    /// what the services it asked for answered, and what it watches. The frames that say it,
    /// for the root to relay.
    pub fn step(&mut self) -> Vec<Vec<u8>> {
        let next = self.instances.next_screens(host::pass());
        let mut messages: Vec<Outbound> = next.opened;
        for message in &mut messages {
            self.instances.translate(message, false);
        }
        self.wire.send(messages);
        for (op, outcome) in self.instances.take_answers() {
            self.wire.answer(op, outcome);
        }
        let waiting = self.wire.waiting();
        let answers = self.instances.answers(&waiting);
        let accepted = self.instances.take_accepted();
        let wire = &mut self.wire;
        self.instances
            .relay(|replica, frame| wire.relay(replica, frame));
        for (op, outcome) in self
            .instances
            .stream_fetches(|body, item| wire.stream_item(body, item))
        {
            wire.answer(op, outcome);
        }
        for (watch, value) in answers {
            self.wire.answer_watch(watch, value);
        }
        for send in accepted {
            self.wire.accepted(send);
        }
        self.wire.take_outbound()
    }

    fn heard(&mut self, heard: Heard) {
        let instances = &mut self.instances;
        match heard {
            Heard::Blocks(instance, call) => {
                instances.blocks_call(instance, call);
            }
            Heard::Watch(instance, open, _) => {
                instances.watch_published(instance, open);
            }
            Heard::Unwatch(instance, open) => instances.unwatch_published(instance, open),
            Heard::Published(instance, started) => {
                instances.published_call(instance, started);
            }
            Heard::Debug(instance, call) => {
                let mut asked = (instance, call);
                instances.translate(&mut asked, true);
                instances.debug_call(asked.0, asked.1);
            }
            Heard::Ended(op) => instances.ended(op),
            Heard::Service(service) => instances.serve(service),
            Heard::Network(instance, call) => instances.network(instance, call),
            Heard::Withdrawn(op) => instances.withdraw(op),
            Heard::Shell(call) => self.shell(call),
            // The root runs the app: nothing else of it reaches here, since the root forwards only
            // the editors' ops and those under them.
            Heard::Hello(_)
            | Heard::Layout(_)
            | Heard::FrameNeeded
            | Heard::MissingCharacters(_)
            | Heard::LaidOut(_)
            | Heard::Presented(_)
            | Heard::ShutDown
            | Heard::Unanswered(_)
            | Heard::Answer(..)
            | Heard::Paintings(_) => {}
        }
    }
}

impl Forwarded {
    /// Answers what an editor tells the shell around it, and refuses what it asks: the root's
    /// views take none of it yet (plans/host_as_platform_adapter.md, 4g7b3f and 4g7c to 4g7e),
    /// so what is told is dropped. Where its children take the keyboard rides on their
    /// placements (`keyboard-ask`), which the root reads itself.
    fn shell(&mut self, call: ShellCall) {
        let ShellCall::Tell { reply, .. } = call else {
            let (op, outcome) = call.refuse(OpError::Unsupported);
            self.wire.answer(op, outcome);
            return;
        };
        let (op, outcome) = reply.answer(&());
        self.wire.answer(op, outcome);
    }
}

#[cfg(test)]
mod tests;
