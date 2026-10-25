use std::collections::{BTreeMap, HashMap};

use super::calls::{EditorAnswer, EditorCall, Heard, PluginCall};
use block_plugin_api::{Encoded, Watch};

use block_plugin_api::{
    AudioStatus, BlockPick, Catalog, ChildStatus, EditorInstanceId, Fonts, HelloAccepted,
    InputBatch, Opening, Preferences, WebViewEvent, WebViewId,
    embed::{
        blocks,
        exports::plugin::{self, Editor},
        host::Call,
        host_services,
        shell::Call as ShellCall,
    },
    ops::{Connection, Incoming, OpError, OpId, Outcome, Producer, Reply, Side, Started, Ticket},
};

#[derive(Default)]
struct Held {
    owners: HashMap<OpId, EditorInstanceId>,
    watches: BTreeMap<OpId, Watch<Encoded>>,
    producers: HashMap<OpId, Producer<Vec<u8>>>,
}

pub(super) struct Service {
    pub(super) instance: EditorInstanceId,
    pub(super) op: Option<OpId>,
    pub(super) call: host_services::Call,
}

#[derive(Default)]
pub(super) struct Services {
    pub(super) asked: Vec<Service>,
    pub(super) withdrawn: Vec<OpId>,
}

pub(super) struct Wire {
    connection: Connection,
    connecting: Option<Reply<block_plugin_api::HelloAccepted>>,
    editors: HashMap<EditorInstanceId, Editor>,
    draws: BTreeMap<OpId, Awaited>,
    answering: BTreeMap<OpId, (EditorInstanceId, Answering)>,
    picks: HashMap<(EditorInstanceId, u64), OpId>,
    painting: HashMap<OpId, Ticket<Vec<u8>>>,
    audio: HashMap<EditorInstanceId, Watch<AudioStatus>>,
    host_values: HashMap<OpId, WatchedHostValue>,
    web_views: HashMap<OpId, ShownWebView>,
    preferences: Watch<Preferences>,
    utc_offset: Watch<i32>,
    fonts: Watch<Fonts>,
    input: Watch<Vec<InputBatch>>,
    services: Services,
    serving: HashMap<OpId, EditorInstanceId>,
    child_statuses: HashMap<EditorInstanceId, Watch<Vec<ChildStatus>>>,
    block_types: Watch<Catalog>,
    held: Held,
}

type WatchedHostValue = ((EditorInstanceId, String), Watch<Vec<u8>>);

type ShownWebView = (EditorInstanceId, WebViewId, Watch<Vec<WebViewEvent>>);

enum Answering {
    Created(Ticket<Result<[u8; 16], String>>),
    Regenerated(Ticket<Result<(), String>>),
    Replaced(u64, Ticket<bool>),
    Picked(u64, Ticket<BlockPick>),
}

impl Answering {
    fn answer(self, outcome: Outcome) -> EditorAnswer {
        let failed = |error: OpError| format!("the editor did not answer: {error:?}");
        match self {
            Self::Created(ticket) => EditorAnswer::Created(
                ticket
                    .decode(outcome)
                    .unwrap_or_else(|error| Err(failed(error))),
            ),
            Self::Regenerated(ticket) => EditorAnswer::Regenerated(
                ticket
                    .decode(outcome)
                    .unwrap_or_else(|error| Err(failed(error))),
            ),
            Self::Picked(pick, ticket) => EditorAnswer::Picked {
                pick,
                answer: ticket.decode(outcome).unwrap_or(BlockPick::Cancelled),
            },
            Self::Replaced(request_id, ticket) => EditorAnswer::Replaced {
                request_id,
                replaced: ticket.decode(outcome).unwrap_or(false),
            },
        }
    }
}

enum Awaited {
    Draw,
    Shutdown,
}

impl Default for Wire {
    fn default() -> Self {
        Self {
            connection: Connection::new(Side::Host),
            connecting: None,
            editors: HashMap::new(),
            draws: BTreeMap::new(),
            answering: BTreeMap::new(),
            picks: HashMap::new(),
            painting: HashMap::new(),
            audio: HashMap::new(),
            host_values: HashMap::new(),
            web_views: HashMap::new(),
            preferences: Watch::default(),
            utc_offset: Watch::default(),
            fonts: Watch::default(),
            input: Watch::default(),
            services: Services::default(),
            serving: HashMap::new(),
            child_statuses: HashMap::new(),
            block_types: Watch::default(),
            held: Held::default(),
        }
    }
}

impl Wire {
    /// Answers the plugin's `connect`.
    pub(super) fn accept(&mut self, accepted: &HelloAccepted) {
        if let Some(reply) = self.connecting.take() {
            reply.send(&mut self.connection, accepted);
        }
    }

    pub(super) fn shutdown(&mut self) {
        let done = plugin::shutdown(&mut self.connection);
        self.draws.insert(done.op(), Awaited::Shutdown);
    }

    pub(super) fn open(&mut self, opening: Opening) {
        let instance = opening.instance();
        let editor = Editor::new(&mut self.connection, opening);
        if let Some(earlier) = self.editors.insert(instance, editor) {
            earlier.resource_drop(&mut self.connection);
        }
    }

    pub(super) fn close(&mut self, instance: EditorInstanceId) {
        if let Some(editor) = self.editors.remove(&instance) {
            editor.resource_drop(&mut self.connection);
        }
        self.child_statuses.remove(&instance);
        self.audio.remove(&instance);
    }

    pub(super) fn plugin_call(&mut self, call: PluginCall) {
        let connection = &mut self.connection;
        match call {
            PluginCall::Preferences(preferences) => {
                self.preferences.change(connection, |_| preferences);
            }
            PluginCall::UtcOffset(seconds) => self.utc_offset.change(connection, |_| seconds),
            PluginCall::Fonts(fonts) => self.fonts.change(connection, |held| merged(held, fonts)),
            PluginCall::Input(batch) => self.input.change(connection, |held| {
                let mut batches = held.unwrap_or_default();
                InputBatch::queue(&mut batches, batch);
                batches
            }),
            PluginCall::Screens(screens) => plugin::screens(connection, screens),
            PluginCall::Configure(serial) => plugin::configure(connection, serial),
            PluginCall::Draw { now_micros, seen } => {
                let drawn = plugin::draw(connection, now_micros, seen);
                self.draws.insert(drawn.op(), Awaited::Draw);
            }
            PluginCall::BlockTypes(catalog) => self.block_types.change(connection, |_| catalog),
            PluginCall::Describe(on) => plugin::describe(connection, on),
            PluginCall::RunAction { screen, id } => plugin::run_action(connection, screen, id),
            PluginCall::NodeAction { screen, request } => {
                plugin::node_action(connection, screen, request)
            }
            PluginCall::Paintings => {
                let painted = plugin::paintings(connection);
                self.painting.insert(painted.op(), painted);
            }
        }
    }

    pub(super) fn call(&mut self, instance: EditorInstanceId, call: EditorCall) {
        let Some(editor) = self.editors.get(&instance).copied() else {
            return;
        };
        let connection = &mut self.connection;
        match call {
            EditorCall::HostValue { key, value } => {
                for (_, watch) in self
                    .host_values
                    .values_mut()
                    .filter(|(watched, _)| *watched == (instance, key.clone()))
                {
                    watch.change(connection, |_| value.clone());
                }
            }
            EditorCall::AudioStatus(status) => {
                self.audio
                    .entry(instance)
                    .or_default()
                    .change(connection, |_| status);
            }
            EditorCall::WebViewEvent { web_view, event } => {
                if let Some((_, _, watch)) = self
                    .web_views
                    .values_mut()
                    .find(|(shown, id, _)| (*shown, *id) == (instance, web_view))
                {
                    watch.change(connection, |held| {
                        let mut events = held.unwrap_or_default();
                        events.push(event);
                        events
                    });
                }
            }
            EditorCall::ChildStatuses(statuses) => {
                self.child_statuses
                    .entry(instance)
                    .or_default()
                    .change(connection, |held| {
                        let mut merged = held.unwrap_or_default();
                        for status in statuses {
                            match merged.iter_mut().find(|held| {
                                (held.region, held.child) == (status.region, status.child)
                            }) {
                                Some(held) => *held = status,
                                None => merged.push(status),
                            }
                        }
                        merged
                    });
            }
            EditorCall::MenuPick(id) => editor.menu_pick(connection, id),
            EditorCall::ChildBar {
                region,
                child,
                action,
            } => editor.child_bar(connection, region, child, action),
            EditorCall::ChildFocused { region, child } => {
                editor.child_focused(connection, region, child)
            }
            EditorCall::SetEditable(editable) => editor.set_editable(connection, editable),
            EditorCall::ViewChanged { view, scale } => editor.view_changed(connection, view, scale),
            EditorCall::PresentingChanged(presenting) => {
                editor.presenting_changed(connection, presenting)
            }
            EditorCall::Presence(visible) => editor.presence(connection, visible),
            EditorCall::FocusChanged(focus) => editor.focus_changed(connection, focus),
            EditorCall::Resized(size) => editor.resized(connection, size),
            EditorCall::DragOver(drag) => editor.drag_over(connection, drag),
            EditorCall::DragLeft => editor.drag_left(connection),
            EditorCall::FileDrop(drop) => editor.file_drop(connection, drop),
            EditorCall::FileDropLeft => editor.file_drop_left(connection),
            EditorCall::ArtifactSettings(data) => editor.artifact_settings(connection, data),
            EditorCall::CommitCreation => {
                let ticket = editor.commit_creation(connection);
                self.answering
                    .insert(ticket.op(), (instance, Answering::Created(ticket)));
            }
            EditorCall::RegenerateArtifact(data) => {
                let ticket = editor.regenerate_artifact(connection, data);
                self.answering
                    .insert(ticket.op(), (instance, Answering::Regenerated(ticket)));
            }
            EditorCall::Pick(request) => {
                let pick = request.pick;
                let ticket = editor.pick(connection, request);
                self.picks.insert((instance, pick), ticket.op());
                self.answering
                    .insert(ticket.op(), (instance, Answering::Picked(pick, ticket)));
            }
            EditorCall::WithdrawPick(pick) => {
                if let Some(op) = self.picks.remove(&(instance, pick)) {
                    connection.cancel(op);
                }
            }
            EditorCall::ReplaceChild {
                request_id,
                old,
                new,
            } => {
                let ticket = editor.replace_child(connection, old, new);
                self.answering.insert(
                    ticket.op(),
                    (instance, Answering::Replaced(request_id, ticket)),
                );
            }
            EditorCall::ShowDialog { block, dialog } => {
                editor.show_dialog(connection, block, dialog);
            }
        }
    }

    pub(super) fn take_services(&mut self) -> Services {
        std::mem::take(&mut self.services)
    }

    pub(super) fn answer(&mut self, op: OpId, outcome: Outcome) {
        if self.serving.remove(&op).is_some() {
            self.connection.complete(op, outcome);
        }
    }

    fn editor_of(&self, parent: Option<OpId>) -> Option<EditorInstanceId> {
        let parent = parent?;
        self.editors
            .iter()
            .find(|(_, editor)| editor.op() == parent)
            .map(|(instance, _)| *instance)
    }

    fn serve(&mut self, started: Started) {
        let op = started.op;
        let Some(instance) = self.editor_of(started.parent) else {
            self.connection.complete(op, Outcome::Err(OpError::Denied));
            return;
        };
        if let Some(call) = host_services::Call::accept(&mut self.connection, started) {
            self.serving.insert(op, instance);
            self.services.asked.push(Service {
                instance,
                op: Some(op),
                call,
            });
        }
    }

    fn watch_call(&mut self, call: ShellCall, heard: &mut Vec<Heard>) {
        let connection = &mut self.connection;
        match call {
            ShellCall::AudioStatusWatchNew { resource, instance } => self
                .audio
                .entry(instance)
                .or_default()
                .opened(resource.op()),
            ShellCall::AudioStatusWatchNext { this, reply } => {
                match self.audio.values_mut().find(|watch| watch.is(this.op())) {
                    Some(watch) => watch.next(connection, reply),
                    None => reply.fail(connection, OpError::Canceled),
                }
            }
            ShellCall::WebViewNew {
                resource,
                instance,
                id,
                url,
            } => {
                let mut watch = Watch::default();
                watch.opened(resource.op());
                self.web_views.insert(resource.op(), (instance, id, watch));
                heard.push(Heard::Shell(ShellCall::WebViewNew {
                    resource,
                    instance,
                    id,
                    url,
                }));
            }
            ShellCall::HostValueWatchNew {
                resource,
                instance,
                key,
            } => {
                let mut watch = Watch::default();
                watch.opened(resource.op());
                self.host_values
                    .insert(resource.op(), ((instance, key.clone()), watch));
                heard.push(Heard::Shell(ShellCall::HostValueWatchNew {
                    resource,
                    instance,
                    key,
                }));
            }
            ShellCall::HostValueWatchNext { this, reply } => {
                match self.host_values.get_mut(&this.op()) {
                    Some((_, watch)) => watch.next(connection, reply),
                    None => reply.fail(connection, OpError::Canceled),
                }
            }
            ShellCall::WebViewNextEvents { this, reply } => {
                match self.web_views.get_mut(&this.op()) {
                    Some((_, _, watch)) => watch.next(connection, reply),
                    None => reply.fail(connection, OpError::Canceled),
                }
            }
            ShellCall::ChildStatusesWatchNew { resource, instance } => self
                .child_statuses
                .entry(instance)
                .or_default()
                .opened(resource.op()),
            ShellCall::ChildStatusesWatchNext { this, reply } => {
                match self
                    .child_statuses
                    .values_mut()
                    .find(|watch| watch.is(this.op()))
                {
                    Some(watch) => watch.next(connection, reply),
                    None => reply.fail(connection, OpError::Canceled),
                }
            }
            ShellCall::BlockTypesWatchNew { resource } => self.block_types.opened(resource.op()),
            ShellCall::BlockTypesWatchNext { reply, .. } => {
                self.block_types.next(connection, reply)
            }
            _ => {}
        }
    }

    pub(super) fn waiting(&self) -> Vec<OpId> {
        self.held
            .watches
            .iter()
            .filter(|(_, watch)| watch.waiting())
            .map(|(op, _)| *op)
            .collect()
    }

    pub(super) fn answer_watch(&mut self, watch: OpId, value: Vec<u8>) {
        if let Some(watch) = self.held.watches.get_mut(&watch) {
            watch.set(&mut self.connection, value);
        }
    }

    pub(super) fn relay(&mut self, replica: OpId, frame: &Vec<u8>) {
        if let Some(producer) = self.held.producers.get(&replica) {
            producer.send(&mut self.connection, frame);
        }
    }

    fn blocks(
        &mut self,
        op: Option<OpId>,
        parent: Option<OpId>,
        call: blocks::Call,
        heard: &mut Vec<Heard>,
    ) -> Result<(), String> {
        use blocks::Call as Blocks;
        call.validate()
            .map_err(|error| format!("the plugin asked for something it may not: {error:?}"))?;
        let owner = |this: OpId| self.held.owners.get(&this).copied();
        let instance = match &call {
            Blocks::BlocksWatchNext { this, .. } => owner(this.op()),
            Blocks::HistoryWatchNext { this, .. } => owner(this.op()),
            Blocks::HeadWatchNext { this, .. } => owner(this.op()),
            Blocks::ArtifactWatchNext { this, .. } => owner(this.op()),
            Blocks::VersionWatchNext { this, .. } => owner(this.op()),
            Blocks::ShowRequestsNext { this, .. } => owner(this.op()),
            Blocks::ReplicaFrames { this, .. } | Blocks::ReplicaSend { this, .. } => {
                owner(this.op())
            }
            _ => self.editor_of(parent),
        };
        let Some(instance) = instance else {
            if let Some(op) = op {
                self.connection.complete(op, Outcome::Err(OpError::Denied));
            }
            return Ok(());
        };
        let next = match &call {
            Blocks::BlocksWatchNext { this, reply } => Some((this.op(), reply.op())),
            Blocks::HistoryWatchNext { this, reply } => Some((this.op(), reply.op())),
            Blocks::HeadWatchNext { this, reply } => Some((this.op(), reply.op())),
            Blocks::ArtifactWatchNext { this, reply } => Some((this.op(), reply.op())),
            Blocks::VersionWatchNext { this, reply } => Some((this.op(), reply.op())),
            Blocks::ShowRequestsNext { this, reply } => Some((this.op(), reply.op())),
            _ => None,
        };
        if let Some((watch, next)) = next {
            self.held
                .watches
                .entry(watch)
                .or_default()
                .next_encoded(&mut self.connection, next);
            return Ok(());
        }
        match call {
            Blocks::ReplicaFrames { this, stream } => {
                self.held.producers.insert(this.op(), stream);
                return Ok(());
            }
            Blocks::BlocksWatchNew { resource, .. } => {
                self.held.owners.insert(resource.op(), instance)
            }
            Blocks::HistoryWatchNew { resource, .. } => {
                self.held.owners.insert(resource.op(), instance)
            }
            Blocks::HeadWatchNew { resource, .. } => {
                self.held.owners.insert(resource.op(), instance)
            }
            Blocks::ArtifactWatchNew { resource, .. } => {
                self.held.owners.insert(resource.op(), instance)
            }
            Blocks::VersionWatchNew { resource } => {
                self.held.owners.insert(resource.op(), instance)
            }
            Blocks::ShowRequestsNew { resource } => {
                self.held.owners.insert(resource.op(), instance)
            }
            Blocks::ReplicaNew { resource, .. } => self.held.owners.insert(resource.op(), instance),
            _ => None,
        };
        heard.push(Heard::Blocks(instance, call));
        Ok(())
    }

    fn ended(&mut self, op: OpId, heard: &mut Vec<Heard>) -> bool {
        let held = &mut self.held;
        if held.owners.remove(&op).is_some() {
            if let Some(producer) = held.producers.remove(&op) {
                producer.fail(&mut self.connection, OpError::Canceled);
            }
            if let Some(mut watch) = held.watches.remove(&op) {
                watch.end(&mut self.connection);
            }
            self.connection
                .complete(op, Outcome::Err(OpError::Canceled));
            heard.push(Heard::Ended(op));
            return true;
        }
        let connection = &mut self.connection;
        if held
            .watches
            .values_mut()
            .any(|watch| watch.withdraw(connection, op))
        {
            return true;
        }
        let streamed = held
            .producers
            .iter()
            .find(|(_, producer)| producer.op() == op)
            .map(|(replica, _)| *replica);
        if let Some(replica) = streamed {
            held.producers.remove(&replica);
            self.connection
                .complete(op, Outcome::Err(OpError::Canceled));
            return true;
        }
        false
    }

    pub(super) fn take_outbound(&mut self) -> Vec<Vec<u8>> {
        self.connection.take_outbound()
    }

    pub(super) fn receive(&mut self, frame: &[u8], heard: &mut Vec<Heard>) -> Result<(), String> {
        self.connection
            .receive(frame)
            .map_err(|error| format!("the plugin sent a frame that did not decode: {error}"))?;
        while let Some(event) = self.connection.next_event() {
            self.heard(event, heard)?;
        }
        Ok(())
    }

    fn heard(&mut self, event: Incoming, heard: &mut Vec<Heard>) -> Result<(), String> {
        let call = match event {
            Incoming::Started(started) if blocks::Call::accepts(&started) => {
                let (op, parent) = (started.op, started.parent);
                return match blocks::Call::accept(&mut self.connection, started) {
                    Some(call) => self.blocks(Some(op), parent, call, heard),
                    None => Ok(()),
                };
            }
            Incoming::Started(started) if host_services::Call::accepts(&started) => {
                self.serve(started);
                return Ok(());
            }
            Incoming::Started(started) if ShellCall::accepts(&started) => {
                if let Some(call) = ShellCall::accept(&mut self.connection, started) {
                    self.watch_call(call, heard);
                }
                return Ok(());
            }
            Incoming::Started(started) => match Call::accept(&mut self.connection, started) {
                Some(call) => call,
                None => return Ok(()),
            },
            Incoming::Called(called) => {
                if let Some(call) = host_services::Call::accept_direct(&called) {
                    if let Some(instance) = self.editor_of(called.parent) {
                        self.services.asked.push(Service {
                            instance,
                            op: None,
                            call,
                        });
                    }
                    return Ok(());
                }
                if let Some(call) = ShellCall::accept_direct(&called) {
                    heard.push(Heard::Shell(call));
                    return Ok(());
                }
                if let Some(call) = blocks::Call::accept_direct(&called) {
                    return self.blocks(None, called.parent, call, heard);
                }
                match Call::accept_direct(&called) {
                    Some(call) => call,
                    None => {
                        return Err(format!(
                            "the plugin called {}, which the host does not answer",
                            called.name
                        ));
                    }
                }
            }
            Incoming::Completed { op, outcome } => {
                if let Some(painted) = self.painting.remove(&op) {
                    heard.push(Heard::Paintings(
                        painted.decode(outcome).unwrap_or_default(),
                    ));
                    return Ok(());
                }
                if let Some((instance, answering)) = self.answering.remove(&op) {
                    self.picks.retain(|_, pick| *pick != op);
                    heard.push(Heard::Answer(instance, answering.answer(outcome)));
                    return Ok(());
                }
                match (self.draws.remove(&op), outcome) {
                    (Some(Awaited::Draw), Outcome::Ok(bytes)) => {
                        let commit: block_plugin_api::FrameCommit =
                            block_plugin_api::ops::decode(&bytes).ok_or(
                                "the plugin answered a draw with a frame that did not decode",
                            )?;
                        commit.validate().map_err(|error| {
                            format!("the plugin drew a frame it may not: {error}")
                        })?;
                        heard.push(Heard::Frame(Box::new(commit)));
                    }
                    (Some(Awaited::Shutdown), Outcome::Ok(_)) => heard.push(Heard::ShutDown),
                    (Some(_), Outcome::Err(error)) => heard.push(Heard::Unanswered(format!(
                        "the plugin did not answer: {error:?}"
                    ))),
                    (None, _) => {}
                }
                return Ok(());
            }
            Incoming::Canceled { op } => {
                if self.ended(op, heard) {
                    return Ok(());
                }
                if self.serving.remove(&op).is_some() {
                    self.connection
                        .complete(op, Outcome::Err(OpError::Canceled));
                    self.services.withdrawn.push(op);
                }
                let watched = self.child_statuses.values().any(|watch| watch.is(op))
                    || self.audio.values().any(|watch| watch.is(op))
                    || self.web_views.contains_key(&op)
                    || self.host_values.contains_key(&op)
                    || self.block_types.is(op);
                self.child_statuses.retain(|_, watch| !watch.is(op));
                self.audio.retain(|_, watch| !watch.is(op));
                self.web_views.remove(&op);
                self.host_values.remove(&op);
                if watched {
                    self.connection
                        .complete(op, Outcome::Err(OpError::Canceled));
                }
                self.block_types.closed(op);
                self.preferences.closed(op);
                self.utc_offset.closed(op);
                self.fonts.closed(op);
                self.input.closed(op);
                return Ok(());
            }
            Incoming::Yielded { .. } => return Ok(()),
        };
        let connection = &mut self.connection;
        match call {
            Call::Connect { reply, hello } => {
                self.connecting = Some(reply);
                heard.push(Heard::Hello(hello));
            }
            Call::Layout { layout } => heard.push(Heard::Layout(layout)),
            Call::FrameNeeded {} => heard.push(Heard::FrameNeeded),
            Call::MissingCharacters { characters } => {
                heard.push(Heard::MissingCharacters(characters));
            }
            Call::PreferencesWatchNew { resource } => self.preferences.opened(resource.op()),
            Call::PreferencesWatchNext { reply, .. } => self.preferences.next(connection, reply),
            Call::UtcOffsetWatchNew { resource } => self.utc_offset.opened(resource.op()),
            Call::UtcOffsetWatchNext { reply, .. } => self.utc_offset.next(connection, reply),
            Call::FontsWatchNew { resource } => self.fonts.opened(resource.op()),
            Call::FontsWatchNext { reply, .. } => self.fonts.next(connection, reply),
            Call::InputNew { resource } => self.input.opened(resource.op()),
            Call::InputNext { reply, .. } => self.input.next(connection, reply),
        }
        Ok(())
    }
}

fn merged(held: Option<Fonts>, fonts: Fonts) -> Fonts {
    match held {
        Some(mut held) if !fonts.replace => {
            held.faces.extend(fonts.faces);
            held
        }
        _ => fonts,
    }
}

#[cfg(test)]
mod tests;
