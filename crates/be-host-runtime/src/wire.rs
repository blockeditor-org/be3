use be_lib_stable_hash::HashMap;
use std::collections::BTreeMap;

use super::calls::{AppCall, EditorAnswer, EditorCall, Heard};
use be_sdk_api::{Encoded, Watch};

use be_sdk_api::{
    AudioStatus, Catalog, ChildStatus, DebugTemplate, DockUpdate, EditorInstanceId, Failure, Fonts,
    HelloAccepted, InputBatch, Opening, Preferences, WebViewEvent, WebViewId,
    app::EditorEvent,
    embed::{
        self, blocks,
        debug::debug::Call as DebugCall,
        dock::Call as DockCall,
        exports::{
            app::{self, Editor},
            debug::paintings,
        },
        host::Call,
        host_services, network,
        shell::Call as ShellCall,
    },
    ops::{
        Connection, Incoming, MethodId, OpError, OpId, Outcome, Producer, Reply, Side, Started,
        Ticket, wire,
    },
};

#[derive(Default)]
struct Held {
    owners: HashMap<OpId, EditorInstanceId>,
    watches: BTreeMap<OpId, Watch<Encoded>>,
    producers: HashMap<OpId, Producer<Vec<u8>, wire::Bytes>>,
    sends: HashMap<OpId, OpId>,
}

#[derive(Debug)]
pub(super) struct Service {
    pub(super) instance: EditorInstanceId,
    pub(super) call: host_services::Call,
}

pub(super) struct Wire {
    connection: Connection,
    connecting: Option<Reply<be_sdk_api::HelloAccepted>>,
    editors: HashMap<EditorInstanceId, Editor>,
    draws: BTreeMap<OpId, Awaited>,
    answering: BTreeMap<OpId, (EditorInstanceId, Answering)>,
    painting: HashMap<OpId, Ticket<Vec<u8>, wire::Bytes>>,
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
    /// Answers to the calls an app makes without waiting on them (`layout`,
    /// `laid-out`, `frame-needed`, `missing-characters`). They go with the next
    /// thing sent to the app rather than alone, since an answer alone would step
    /// the app for nothing: each frame it draws sends `laid-out`.
    acks: Vec<Reply<()>>,
    hearing: Vec<(EditorInstanceId, EditorEvent)>,
}

type PublishedWatch = (EditorInstanceId, MethodId, Watch<Encoded>);

type ShownWebView = (EditorInstanceId, WebViewId, Watch<Vec<WebViewEvent>>);

enum Answering {
    Created(Ticket<Result<[u8; 16], Failure>>),
    Regenerated(Ticket<Result<(), Failure>>),
    Replaced(u64, Ticket<bool>),
    Rendered(u64, Ticket<Option<Vec<u8>>, wire::Optional<wire::Bytes>>),
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
            Self::Rendered(request_id, ticket) => EditorAnswer::Rendered {
                request_id,
                data: ticket.decode(outcome).unwrap_or(None),
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
            editors: HashMap::default(),
            draws: BTreeMap::new(),
            answering: BTreeMap::new(),
            painting: HashMap::default(),
            audio: HashMap::default(),
            published: HashMap::default(),
            extensions: Vec::new(),
            web_views: HashMap::default(),
            preferences: Watch::default(),
            utc_offset: Watch::default(),
            fonts: Watch::default(),
            input: Watch::default(),
            serving: HashMap::default(),
            child_statuses: HashMap::default(),
            docks: HashMap::default(),
            block_types: Watch::default(),
            debug_templates: Watch::default(),
            held: Held::default(),
            acks: Vec::new(),
            hearing: Vec::new(),
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
        self.flush_hearing();
        let done = app::shutdown(&mut self.connection);
        self.draws.insert(done.op(), Awaited::Shutdown);
    }

    pub(super) fn open(&mut self, opening: Opening) {
        self.flush_hearing();
        let instance = opening.instance;
        let editor = Editor::new(&mut self.connection, opening);
        if let Some(earlier) = self.editors.insert(instance, editor) {
            earlier.resource_drop(&mut self.connection);
        }
    }

    pub(super) fn close(&mut self, instance: EditorInstanceId) {
        self.flush_hearing();
        if let Some(editor) = self.editors.remove(&instance) {
            editor.resource_drop(&mut self.connection);
        }
        self.child_statuses.remove(&instance);
        self.docks.remove(&instance);
        self.audio.remove(&instance);
    }

    pub(super) fn app_call(&mut self, call: AppCall) {
        self.flush_hearing();
        let connection = &mut self.connection;
        match call {
            AppCall::Preferences(preferences) => {
                self.preferences.change(connection, |_| preferences);
            }
            AppCall::UtcOffset(seconds) => self.utc_offset.change(connection, |_| seconds),
            AppCall::Fonts(fonts) => self.fonts.change(connection, |held| merged(held, fonts)),
            AppCall::Input(batch) => self.input.change(connection, |held| {
                let mut batches = held.unwrap_or_default();
                InputBatch::queue(&mut batches, batch);
                batches
            }),
            AppCall::Screens(screens) => {
                app::screens(connection, screens);
            }
            AppCall::Configure(serial) => {
                app::configure(connection, serial);
            }
            AppCall::Draw {
                now_micros,
                wall_micros,
                seen,
            } => {
                let drawn = app::draw(connection, now_micros, wall_micros, seen);
                self.draws.insert(drawn.op(), Awaited::Draw);
            }
            AppCall::BlockTypes(catalog) => self.block_types.change(connection, |_| catalog),
            AppCall::DebugTemplates(templates) => {
                self.debug_templates.change(connection, |_| templates)
            }
            AppCall::Describe(on) => {
                app::describe(connection, on);
            }
            AppCall::RunAction { screen, id } => {
                app::run_action(connection, screen, id);
            }
            AppCall::NodeAction { screen, request } => {
                app::node_action(connection, screen, request);
            }
            AppCall::Paintings => {
                let painted = paintings::paintings(connection);
                self.painting.insert(painted.op(), painted);
            }
        }
    }

    pub(super) fn call(&mut self, instance: EditorInstanceId, call: EditorCall) {
        let Some(editor) = self.editors.get(&instance).copied() else {
            return;
        };
        // Whatever is not an event goes after the events queued before it.
        self.flush_hearing();
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
                            match merged
                                .iter_mut()
                                .find(|held| (held.view, held.child) == (status.view, status.child))
                            {
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
            EditorCall::RenderClip {
                request_id,
                serial,
                format,
            } => {
                let ticket = editor.render_clip(connection, serial, format);
                self.answering.insert(
                    ticket.op(),
                    (instance, Answering::Rendered(request_id, ticket)),
                );
            }
        }
    }

    /// Queues an event for the editor; it goes out with the others in `flush_hearing`.
    pub(super) fn event(&mut self, instance: EditorInstanceId, event: EditorEvent) {
        if self.editors.contains_key(&instance) {
            self.hear(instance, event);
        }
    }

    /// Queues what happened around an editor; `flush_hearing` sends what is
    /// queued, a call for each run of events of one editor, so the order across
    /// editors is the order they came.
    fn hear(&mut self, instance: EditorInstanceId, event: EditorEvent) {
        self.hearing.push((instance, event));
    }

    /// Sends the events queued, in the order they came.
    pub(super) fn flush_hearing(&mut self) {
        let mut queued = std::mem::take(&mut self.hearing).into_iter().peekable();
        while let Some((instance, event)) = queued.next() {
            let mut events = vec![event];
            while let Some((_, event)) = queued.next_if(|(next, _)| *next == instance) {
                events.push(event);
            }
            if let Some(editor) = self.editors.get(&instance).copied() {
                editor.hear(&mut self.connection, events);
            }
        }
    }

    pub(super) fn answer(&mut self, op: OpId, outcome: Outcome) {
        self.serving.remove(&op);
        self.connection.complete(op, outcome);
    }

    fn editor_of(&self, parent: Option<OpId>) -> Option<EditorInstanceId> {
        let parent = parent?;
        self.editors
            .iter()
            .find(|(_, editor)| editor.op() == parent)
            .map(|(instance, _)| *instance)
    }

    fn serve(
        &mut self,
        op: OpId,
        parent: Option<OpId>,
        call: host_services::Call,
        heard: &mut Vec<Heard>,
    ) {
        use host_services::Call as Services;
        let instance = match &call {
            Services::RevisionPreviewOpened { this, .. } => {
                self.held.owners.get(&this.op()).copied()
            }
            _ => self.editor_of(parent),
        };
        let Some(instance) = instance else {
            self.connection.complete(op, Outcome::Err(OpError::Denied));
            return;
        };
        match &call {
            Services::RevisionPreviewNew { resource, .. } => {
                self.held.owners.insert(resource.op(), instance);
            }
            _ => {
                self.serving.insert(op, instance);
            }
        }
        heard.push(Heard::Service(Service { instance, call }));
    }

    /// A fetch belongs to the editor that opened it, and its `response` and `body` to the
    /// fetch; the instance serves them (`Instances::network`), and a cancel of one stops it.
    fn network(
        &mut self,
        op: OpId,
        parent: Option<OpId>,
        call: network::Call,
        heard: &mut Vec<Heard>,
    ) {
        use network::Call as Network;
        let instance = match &call {
            Network::FetchResponse { this, .. } | Network::FetchBody { this, .. } => {
                self.held.owners.get(&this.op()).copied()
            }
            Network::FetchNew { .. } | Network::OpenUrl { .. } => self.editor_of(parent),
        };
        let Some(instance) = instance else {
            self.connection.complete(op, Outcome::Err(OpError::Denied));
            return;
        };
        match &call {
            Network::FetchNew { resource, .. } => {
                self.held.owners.insert(resource.op(), instance);
            }
            _ => {
                self.serving.insert(op, instance);
            }
        }
        heard.push(Heard::Network(instance, call));
    }

    /// Yields one encoded item on a stream the instance serves, unless it has no credit.
    pub(super) fn stream_item(&mut self, op: OpId, item: &[u8]) -> bool {
        self.connection.ready(op) && self.connection.yield_item(op, item).is_ok()
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
        let extension = be_sdk_manifest::extension_of(interface)?;
        if !self.extensions.iter().any(|granted| granted == extension) {
            return Some(OpError::Denied);
        }
        (!super::published::offered(interface)).then_some(OpError::Unsupported)
    }

    fn published_call(&mut self, started: Started, heard: &mut Vec<Heard>) {
        let refused = match super::published::interface_of(&started) {
            Some(interface) => self.refusal(interface),
            None => Some(OpError::Unsupported),
        };
        match (refused, self.editor_of(started.parent)) {
            (None, Some(instance)) => heard.push(Heard::Published(instance, started)),
            (refused, _) => self
                .connection
                .complete(started.op, Outcome::Err(refused.unwrap_or(OpError::Denied))),
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
            call => heard.push(Heard::Shell(call)),
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
        op: OpId,
        parent: Option<OpId>,
        call: blocks::Call,
        heard: &mut Vec<Heard>,
    ) -> Result<(), String> {
        use blocks::Call as Blocks;
        call.validate()
            .map_err(|error| format!("the app asked for something it may not: {error:?}"))?;
        let owner = |this: OpId| self.held.owners.get(&this).copied();
        let instance = match &call {
            Blocks::BlockWatchNext { this, .. } => owner(this.op()),
            Blocks::SearchWatchNext { this, .. } => owner(this.op()),
            Blocks::ShowRequestsNext { this, .. } => owner(this.op()),
            Blocks::ReplicaFrames { this, .. } | Blocks::ReplicaSend { this, .. } => {
                owner(this.op())
            }
            _ => self.editor_of(parent),
        };
        let Some(instance) = instance else {
            self.connection.complete(op, Outcome::Err(OpError::Denied));
            return Ok(());
        };
        let next = match call {
            Blocks::BlockWatchNext { this, reply } => Ok((this.op(), reply.into_op())),
            Blocks::SearchWatchNext { this, reply } => Ok((this.op(), reply.into_op())),
            Blocks::ShowRequestsNext { this, reply } => Ok((this.op(), reply.into_op())),
            call => Err(call),
        };
        let call = match next {
            Ok((watch, next)) => {
                self.held
                    .watches
                    .entry(watch)
                    .or_default()
                    .next_encoded(&mut self.connection, next);
                return Ok(());
            }
            Err(call) => call,
        };
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
            Blocks::BlockWatchNew { resource, .. } => {
                self.held.owners.insert(resource.op(), instance)
            }
            Blocks::SearchWatchNew { resource, .. } => {
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
        // Acks never wait long: every frame the host asks for, and shutting
        // down, sends the app something. Nothing in the app waits on them; they
        // only close the ops it left open.
        if self.connection.has_outbound() {
            for ack in self.acks.drain(..) {
                ack.send(&mut self.connection, &());
            }
        }
        self.connection.take_outbound()
    }

    pub(super) fn holds_input(&self) -> bool {
        self.input.holding()
    }

    pub(super) fn has_outbound(&self) -> bool {
        self.connection.has_outbound()
    }

    pub(super) fn receive(&mut self, frame: &[u8], heard: &mut Vec<Heard>) -> Result<(), String> {
        self.connection
            .receive(frame)
            .map_err(|error| format!("the app sent a frame that did not decode: {error}"))?;
        while let Some(event) = self.connection.next_event() {
            self.heard(event, heard)?;
        }
        Ok(())
    }

    fn heard(&mut self, event: Incoming, heard: &mut Vec<Heard>) -> Result<(), String> {
        let call = match event {
            Incoming::Started(started) if super::published::watching(started.method).is_some() => {
                self.published_watch(started, heard);
                return Ok(());
            }
            Incoming::Started(started) if super::published::interface_of(&started).is_some() => {
                self.published_call(started, heard);
                return Ok(());
            }
            Incoming::Started(started) => {
                let (op, parent) = (started.op, started.parent);
                let instance = self.editor_of(parent);
                let connection = &mut self.connection;
                match embed::Call::accept(connection, started) {
                    None => return Ok(()),
                    Some(embed::Call::Host(call)) => call,
                    Some(embed::Call::Blocks(call)) => {
                        return self.blocks(op, parent, call, heard);
                    }
                    Some(embed::Call::HostServices(call)) => {
                        self.serve(op, parent, call, heard);
                        return Ok(());
                    }
                    Some(embed::Call::Network(call)) => {
                        self.network(op, parent, call, heard);
                        return Ok(());
                    }
                    Some(embed::Call::Debug(call)) => {
                        match call {
                            DebugCall::DebugTemplatesWatchNew { resource } => {
                                self.debug_templates.opened(resource.op())
                            }
                            DebugCall::DebugTemplatesWatchNext { reply, .. } => {
                                self.debug_templates.next(connection, reply)
                            }
                            call => match instance {
                                Some(instance) => heard.push(Heard::Debug(instance, call)),
                                None => {
                                    let (op, outcome) = call.refuse(OpError::Denied);
                                    connection.complete(op, outcome);
                                }
                            },
                        }
                        return Ok(());
                    }
                    Some(embed::Call::Dock(call)) => {
                        match call {
                            DockCall::DockWatchNew { resource, instance } => {
                                if self.editors.contains_key(&instance) {
                                    self.docks
                                        .entry(instance)
                                        .or_default()
                                        .opened(resource.op());
                                }
                            }
                            DockCall::DockWatchNext { this, reply } => {
                                match self.docks.values_mut().find(|watch| watch.is(this.op())) {
                                    Some(watch) => watch.next(connection, reply),
                                    None => reply.fail(connection, OpError::Canceled),
                                }
                            }
                        }
                        return Ok(());
                    }
                    Some(embed::Call::Shell(call)) => {
                        self.watch_call(call, heard);
                        return Ok(());
                    }
                    Some(_) => {
                        connection.complete(op, Outcome::Err(OpError::Unsupported));
                        return Ok(());
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
                        let commit: be_sdk_api::PresentCommit = be_sdk_api::ops::decode(&bytes)
                            .ok_or("the app answered a draw with a present that did not decode")?;
                        heard.push(Heard::Presented(commit));
                    }
                    (Some(Awaited::Shutdown), Outcome::Ok(_)) => heard.push(Heard::ShutDown),
                    (Some(_), Outcome::Err(error)) => heard.push(Heard::Unanswered(format!(
                        "the app did not answer: {error:?}"
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
                if let Some((instance, open, _)) = self.published.remove(&op)
                    && !self
                        .published
                        .values()
                        .any(|(watcher, watched, _)| (*watcher, *watched) == (instance, open))
                {
                    heard.push(Heard::Unwatch(instance, open));
                }
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
            Call::Layout { reply, layout } => {
                self.acks.push(reply);
                heard.push(Heard::Layout(layout));
            }
            Call::LaidOut { reply, commit } => {
                self.acks.push(reply);
                commit
                    .validate()
                    .map_err(|error| format!("the app laid out a frame it may not: {error}"))?;
                heard.push(Heard::LaidOut(Box::new(commit)));
            }
            Call::FrameNeeded { reply } => {
                self.acks.push(reply);
                heard.push(Heard::FrameNeeded);
            }
            Call::MissingCharacters { reply, characters } => {
                self.acks.push(reply);
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
