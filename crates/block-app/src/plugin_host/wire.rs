use std::collections::{BTreeMap, HashMap};

use super::calls::{EditorAnswer, EditorCall, Heard, PluginCall};
use block_plugin_api::{Encoded, Watch};

use block_plugin_api::{
    AudioStatus, Catalog, ChildStatus, DebugTemplate, DockUpdate, EditorInstanceId,
    Failure, Fonts, HelloAccepted, InputBatch, Opening, Preferences, WebViewEvent, WebViewId,
    embed::{
        blocks,
        debug::debug::Call as DebugCall,
        dock::Call as DockCall,
        exports::{
            debug::paintings,
            plugin::{self, Editor},
        },
        host::Call,
        host_services,
        shell::Call as ShellCall,
    },
    ops::{
        Called, Connection, Incoming, MethodId, OpError, OpId, Outcome, Producer, Reply, Side,
        Started, Ticket,
    },
};

#[derive(Default)]
struct Held {
    owners: HashMap<OpId, EditorInstanceId>,
    watches: BTreeMap<OpId, Watch<Encoded>>,
    producers: HashMap<OpId, Producer<Vec<u8>>>,
    sends: HashMap<OpId, OpId>,
}

#[derive(Debug)]
pub(super) struct Service {
    pub(super) instance: EditorInstanceId,
    pub(super) op: Option<OpId>,
    pub(super) call: host_services::Call,
}

pub(super) struct Wire {
    connection: Connection,
    connecting: Option<Reply<block_plugin_api::HelloAccepted>>,
    editors: HashMap<EditorInstanceId, Editor>,
    draws: BTreeMap<OpId, Awaited>,
    answering: BTreeMap<OpId, (EditorInstanceId, Answering)>,
    painting: HashMap<OpId, Ticket<Vec<u8>>>,
    audio: HashMap<EditorInstanceId, Watch<AudioStatus>>,
    published: HashMap<OpId, PublishedWatch>,
    extensions: Vec<String>,
    web_views: HashMap<OpId, ShownWebView>,
    preferences: Watch<Preferences>,
    utc_offset: Watch<i32>,
    fonts: Watch<Fonts>,
    input: Watch<Vec<InputBatch>>,
    serving: HashMap<OpId, EditorInstanceId>,
    child_statuses: HashMap<EditorInstanceId, Watch<Vec<ChildStatus>>>,
    docks: HashMap<EditorInstanceId, Watch<Vec<DockUpdate>>>,
    block_types: Watch<Catalog>,
    debug_templates: Watch<Vec<DebugTemplate>>,
    held: Held,
}

type PublishedWatch = (EditorInstanceId, MethodId, Watch<Encoded>);

type ShownWebView = (EditorInstanceId, WebViewId, Watch<Vec<WebViewEvent>>);

enum Answering {
    Created(Ticket<Result<[u8; 16], Failure>>),
    Regenerated(Ticket<Result<(), Failure>>),
    Replaced(u64, Ticket<bool>),
}

impl Answering {
    fn answer(self, outcome: Outcome) -> EditorAnswer {
        fn failed<T>(error: OpError) -> Result<T, Failure> {
            Err(Failure::from(error))
        }
        match self {
            Self::Created(ticket) => {
                EditorAnswer::Created(ticket.decode(outcome).unwrap_or_else(failed))
            }
            Self::Regenerated(ticket) => {
                EditorAnswer::Regenerated(ticket.decode(outcome).unwrap_or_else(failed))
            }
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
            painting: HashMap::new(),
            audio: HashMap::new(),
            published: HashMap::new(),
            extensions: Vec::new(),
            web_views: HashMap::new(),
            preferences: Watch::default(),
            utc_offset: Watch::default(),
            fonts: Watch::default(),
            input: Watch::default(),
            serving: HashMap::new(),
            child_statuses: HashMap::new(),
            docks: HashMap::new(),
            block_types: Watch::default(),
            debug_templates: Watch::default(),
            held: Held::default(),
        }
    }
}

impl Wire {
    pub(super) fn granting(extensions: Vec<String>) -> Self {
        Self {
            extensions,
            ..Self::default()
        }
    }

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
        self.docks.remove(&instance);
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
            PluginCall::DebugTemplates(templates) => {
                self.debug_templates.change(connection, |_| templates)
            }
            PluginCall::Describe(on) => plugin::describe(connection, on),
            PluginCall::RunAction { screen, id } => plugin::run_action(connection, screen, id),
            PluginCall::NodeAction { screen, request } => {
                plugin::node_action(connection, screen, request)
            }
            PluginCall::Paintings => {
                let painted = paintings::paintings(connection);
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
            EditorCall::Watched { open, value } => {
                for (_, _, watch) in self
                    .published
                    .values_mut()
                    .filter(|(watcher, watched, _)| (*watcher, *watched) == (instance, open))
                {
                    watch.set(connection, value.to_vec());
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
                        WebViewEvent::queue(&mut events, event);
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
            EditorCall::Dock(update) => {
                self.docks
                    .entry(instance)
                    .or_default()
                    .change(connection, |held| {
                        let mut updates = held.unwrap_or_default();
                        DockUpdate::queue(&mut updates, update);
                        updates
                    });
            }
            EditorCall::MenuPick(id) => editor.menu_pick(connection, id),
            EditorCall::Dismiss => editor.dismiss(connection),
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

    fn serve(&mut self, started: Started, heard: &mut Vec<Heard>) {
        let op = started.op;
        let Some(instance) = self.editor_of(started.parent) else {
            self.connection.complete(op, Outcome::Err(OpError::Denied));
            return;
        };
        if let Some(call) = host_services::Call::accept(&mut self.connection, started) {
            self.serving.insert(op, instance);
            heard.push(Heard::Service(Service {
                instance,
                op: Some(op),
                call,
            }));
        }
    }

    fn published_watch(&mut self, started: Started, heard: &mut Vec<Heard>) {
        let Some(watching) = super::published::watching(started.method) else {
            return;
        };
        let connection = &mut self.connection;
        if watching.next == started.method {
            match started
                .parent
                .and_then(|watch| self.published.get_mut(&watch))
            {
                Some((_, _, watch)) => watch.next_encoded(connection, started.op),
                None => connection.complete(started.op, Outcome::Err(OpError::Canceled)),
            }
            return;
        }
        let instance = self.editor_of(started.parent);
        let refused = match instance {
            None => Some(OpError::Denied),
            Some(_) => self.refusal(watching.interface),
        };
        let (Some(instance), None) = (instance, refused) else {
            self.connection
                .complete(started.op, Outcome::Err(refused.unwrap_or(OpError::Denied)));
            return;
        };
        let mut watch = Watch::default();
        watch.opened(started.op);
        self.published
            .insert(started.op, (instance, watching.open, watch));
        heard.push(Heard::Watch(instance, watching.open));
    }

    fn refusal(&self, interface: &str) -> Option<OpError> {
        let extension = block_plugin_api::extension_of(interface)?;
        if !self.extensions.iter().any(|granted| granted == extension) {
            return Some(OpError::Denied);
        }
        (!super::published::offered(extension)).then_some(OpError::Unsupported)
    }

    fn published_call(&mut self, called: Called, heard: &mut Vec<Heard>) {
        let Some(interface) = super::published::interface_of(&called) else {
            return;
        };
        if self.refusal(interface).is_some() {
            return;
        }
        if let Some(instance) = self.editor_of(called.parent) {
            heard.push(Heard::Called(instance, called));
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

    pub(super) fn relay(&mut self, replica: OpId, frame: &Vec<u8>) -> bool {
        self.held
            .producers
            .get(&replica)
            .is_none_or(|producer| producer.send(&mut self.connection, frame).is_ok())
    }

    pub(super) fn accepted(&mut self, send: OpId) {
        if self.held.sends.remove(&send).is_some() {
            self.connection.complete(send, Outcome::returned(&()));
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
            Blocks::ReplicaSend {
                ref this,
                ref reply,
                ..
            } => {
                self.held.sends.insert(reply.op(), this.op());
                None
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
        if held.sends.remove(&op).is_some() {
            self.connection
                .complete(op, Outcome::Err(OpError::Canceled));
            return true;
        }
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

    pub(super) fn has_outbound(&self) -> bool {
        self.connection.has_outbound()
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
                self.serve(started, heard);
                return Ok(());
            }
            Incoming::Started(started) if super::published::watching(started.method).is_some() => {
                self.published_watch(started, heard);
                return Ok(());
            }
            Incoming::Started(started) if DebugCall::accepts(&started) => {
                let connection = &mut self.connection;
                match DebugCall::accept(connection, started) {
                    Some(DebugCall::DebugTemplatesWatchNew { resource }) => {
                        self.debug_templates.opened(resource.op())
                    }
                    Some(DebugCall::DebugTemplatesWatchNext { reply, .. }) => {
                        self.debug_templates.next(connection, reply)
                    }
                    Some(_) | None => {}
                }
                return Ok(());
            }
            Incoming::Started(started) if DockCall::accepts(&started) => {
                let connection = &mut self.connection;
                match DockCall::accept(connection, started) {
                    Some(DockCall::DockWatchNew { resource, instance }) => {
                        if self.editors.contains_key(&instance) {
                            self.docks
                                .entry(instance)
                                .or_default()
                                .opened(resource.op());
                        }
                    }
                    Some(DockCall::DockWatchNext { this, reply }) => {
                        match self.docks.values_mut().find(|watch| watch.is(this.op())) {
                            Some(watch) => watch.next(connection, reply),
                            None => reply.fail(connection, OpError::Canceled),
                        }
                    }
                    None => {}
                }
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
                        heard.push(Heard::Service(Service {
                            instance,
                            op: None,
                            call,
                        }));
                    }
                    return Ok(());
                }
                if super::published::interface_of(&called).is_some() {
                    self.published_call(called, heard);
                    return Ok(());
                }
                if let Some(call) = DebugCall::accept_direct(&called) {
                    if let Some(instance) = self.editor_of(called.parent) {
                        heard.push(Heard::Debug(instance, call));
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
                    heard.push(Heard::Answer(instance, answering.answer(outcome)));
                    return Ok(());
                }
                match (self.draws.remove(&op), outcome) {
                    (Some(Awaited::Draw), Outcome::Ok(bytes)) => {
                        let commit: block_plugin_api::FrameCommit = block_plugin_api::ops::decode(
                            &bytes,
                        )
                        .ok_or("the plugin answered a draw with a frame that did not decode")?;
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
                let connection = &mut self.connection;
                if self
                    .published
                    .values_mut()
                    .any(|(_, _, watch)| watch.withdraw(connection, op))
                {
                    return Ok(());
                }
                if self.serving.remove(&op).is_some() {
                    self.connection
                        .complete(op, Outcome::Err(OpError::Canceled));
                    heard.push(Heard::Withdrawn(op));
                }
                let watched = self.child_statuses.values().any(|watch| watch.is(op))
                    || self.docks.values().any(|watch| watch.is(op))
                    || self.audio.values().any(|watch| watch.is(op))
                    || self.web_views.contains_key(&op)
                    || self.published.contains_key(&op)
                    || self.block_types.is(op)
                    || self.debug_templates.is(op);
                self.child_statuses.retain(|_, watch| !watch.is(op));
                self.docks.retain(|_, watch| !watch.is(op));
                self.audio.retain(|_, watch| !watch.is(op));
                self.web_views.remove(&op);
                self.published.remove(&op);
                if watched {
                    self.connection
                        .complete(op, Outcome::Err(OpError::Canceled));
                }
                self.block_types.closed(op);
                self.debug_templates.closed(op);
                self.preferences.closed(op);
                self.utc_offset.closed(op);
                self.fonts.closed(op);
                self.input.closed(op);
                return Ok(());
            }
            Incoming::Yielded { .. } | Incoming::Credited { .. } => return Ok(()),
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
