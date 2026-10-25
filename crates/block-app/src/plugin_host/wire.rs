use std::collections::{BTreeMap, HashMap};

use super::calls::{EditorAnswer, EditorCall, Heard};

use block_plugin_api::{
    DecodeError, EditorInstanceId, EditorMessage, ErrorCode, Fonts, InputBatch, Message, Opening,
    Preferences, ProtocolError, decode_frame,
    embed::{
        exports::plugin::{self, Editor},
        host::Call,
        shell::Call as ShellCall,
    },
    encode_frame,
    ops::{Connection, Incoming, OpError, OpId, Outcome, Reply, Side, Ticket},
};

pub(super) struct Wire {
    connection: Connection,
    connecting: Option<Reply<block_plugin_api::HelloAccepted>>,
    editors: HashMap<EditorInstanceId, Editor>,
    draws: BTreeMap<OpId, Awaited>,
    answering: BTreeMap<OpId, (EditorInstanceId, Answering)>,
    preferences: Watch<Preferences>,
    utc_offset: Watch<i32>,
    fonts: Watch<Fonts>,
    input: Watch<Vec<InputBatch>>,
}

enum Answering {
    Created(Ticket<Result<[u8; 16], String>>),
    Regenerated(Ticket<Result<(), String>>),
    Replaced(u64, Ticket<bool>),
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

struct Watch<T> {
    open: Option<OpId>,
    waiting: Option<Reply<T>>,
    held: Option<T>,
}

impl<T> Default for Watch<T> {
    fn default() -> Self {
        Self {
            open: None,
            waiting: None,
            held: None,
        }
    }
}

impl<T: serde::Serialize> Watch<T> {
    fn change(&mut self, connection: &mut Connection, change: impl FnOnce(Option<T>) -> T) {
        let changed = change(self.held.take());
        match self.waiting.take() {
            Some(reply) => reply.send(connection, &changed),
            None => self.held = Some(changed),
        }
    }

    fn next(&mut self, connection: &mut Connection, reply: Reply<T>) {
        match self.held.take() {
            Some(held) => reply.send(connection, &held),
            None => self.waiting = Some(reply),
        }
    }

    fn opened(&mut self, op: OpId) {
        self.open = Some(op);
        self.waiting = None;
    }

    fn closed(&mut self, op: OpId) {
        if self.open == Some(op) {
            *self = Self::default();
        }
    }
}

impl Default for Wire {
    fn default() -> Self {
        Self {
            connection: Connection::new(Side::Host),
            connecting: None,
            editors: HashMap::new(),
            draws: BTreeMap::new(),
            answering: BTreeMap::new(),
            preferences: Watch::default(),
            utc_offset: Watch::default(),
            fonts: Watch::default(),
            input: Watch::default(),
        }
    }
}

impl Wire {
    pub(super) fn send(&mut self, message: Message) -> Result<Vec<Message>, DecodeError> {
        let connection = &mut self.connection;
        match message {
            Message::HelloAccepted(accepted) => {
                if let Some(reply) = self.connecting.take() {
                    reply.send(connection, &accepted);
                }
            }
            Message::Preferences(preferences) => {
                self.preferences.change(connection, |_| preferences);
            }
            Message::UtcOffset(seconds) => self.utc_offset.change(connection, |_| seconds),
            Message::Fonts(fonts) => self.fonts.change(connection, |held| merged(held, fonts)),
            Message::Input(batch) => self.input.change(connection, |held| {
                let mut batches = held.unwrap_or_default();
                match batches.last_mut() {
                    Some(last) if last.screen == batch.screen => last.merge(&batch),
                    _ => batches.push(batch),
                }
                batches
            }),
            Message::Screens(set) => {
                plugin::screens(connection, set.screens);
                return Ok(vec![Message::Acknowledged {
                    request_id: set.request_id,
                }]);
            }
            Message::Configure { serial } => plugin::configure(connection, serial),
            Message::DrawFrame { now_micros, seen } => {
                let drawn = plugin::draw(connection, now_micros, seen);
                self.draws.insert(drawn.op(), Awaited::Draw);
            }
            Message::Shutdown => {
                let done = plugin::shutdown(connection);
                self.draws.insert(done.op(), Awaited::Shutdown);
            }
            Message::Editor(EditorMessage::Close { instance }) => {
                if let Some(editor) = self.editors.remove(&instance) {
                    editor.resource_drop(connection);
                }
            }
            Message::Editor(message) => match Opening::of(message) {
                Ok(opening) => {
                    let instance = opening.instance();
                    let editor = Editor::new(connection, opening);
                    if let Some(earlier) = self.editors.insert(instance, editor) {
                        earlier.resource_drop(connection);
                    }
                }
                Err(message) => {
                    plugin::legacy(connection, encode_frame(&Message::Editor(*message))?)
                }
            },
            message => plugin::legacy(connection, encode_frame(&message)?),
        }
        Ok(Vec::new())
    }

    pub(super) fn call(&mut self, instance: EditorInstanceId, call: EditorCall) {
        let Some(editor) = self.editors.get(&instance).copied() else {
            return;
        };
        let connection = &mut self.connection;
        match call {
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
        }
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
        let connection = &mut self.connection;
        let call = match event {
            Incoming::Started(started) => match Call::accept(connection, started) {
                Some(call) => call,
                None => return Ok(()),
            },
            Incoming::Called(called) => {
                if let Some(call) = ShellCall::accept_direct(&called) {
                    heard.push(Heard::Shell(call));
                    return Ok(());
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
                if let Some((instance, answering)) = self.answering.remove(&op) {
                    heard.push(Heard::Answer(instance, answering.answer(outcome)));
                    return Ok(());
                }
                match (self.draws.remove(&op), outcome) {
                    (Some(Awaited::Draw), Outcome::Ok(bytes)) => {
                        let commit = block_plugin_api::ops::decode(&bytes)
                            .ok_or("the plugin answered a draw with a frame that did not decode")?;
                        heard.push(Heard::Message(Message::Frame(commit)));
                    }
                    (Some(Awaited::Shutdown), Outcome::Ok(_)) => {
                        heard.push(Heard::Message(Message::ShutdownAcknowledged))
                    }
                    (Some(_), Outcome::Err(error)) => {
                        heard.push(Heard::Message(Message::Error(ProtocolError {
                            request_id: None,
                            code: ErrorCode::Internal,
                            message: format!("the plugin did not answer: {error:?}"),
                        })));
                    }
                    (None, _) => {}
                }
                return Ok(());
            }
            Incoming::Canceled { op } => {
                self.preferences.closed(op);
                self.utc_offset.closed(op);
                self.fonts.closed(op);
                self.input.closed(op);
                return Ok(());
            }
            Incoming::Yielded { .. } => return Ok(()),
        };
        match call {
            Call::Connect { reply, hello } => {
                self.connecting = Some(reply);
                heard.push(Heard::Message(Message::Hello(hello)));
            }
            Call::Layout { layout } => heard.push(Heard::Message(Message::Layout(layout))),
            Call::FrameNeeded {} => heard.push(Heard::Message(Message::FrameNeeded)),
            Call::MissingCharacters { characters } => {
                heard.push(Heard::Message(Message::MissingCharacters(characters)));
            }
            Call::PreferencesWatchNew { resource } => self.preferences.opened(resource.op()),
            Call::PreferencesWatchNext { reply, .. } => self.preferences.next(connection, reply),
            Call::UtcOffsetWatchNew { resource } => self.utc_offset.opened(resource.op()),
            Call::UtcOffsetWatchNext { reply, .. } => self.utc_offset.next(connection, reply),
            Call::FontsWatchNew { resource } => self.fonts.opened(resource.op()),
            Call::FontsWatchNext { reply, .. } => self.fonts.next(connection, reply),
            Call::InputNew { resource } => self.input.opened(resource.op()),
            Call::InputNext { reply, .. } => self.input.next(connection, reply),
            Call::Legacy { message } => {
                heard.push(Heard::Message(decode_frame(&message).map_err(|error| {
                    format!("the plugin sent a message that did not decode: {error:?}")
                })?))
            }
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
