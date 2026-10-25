use std::collections::{BTreeMap, HashMap};

use block_plugin_api::{
    DecodeError, EditorInstanceId, EditorMessage, ErrorCode, Fonts, InputBatch, Message, Opening,
    Preferences, ProtocolError, decode_frame,
    embed::{
        exports::plugin::{self, Editor},
        host::Call,
    },
    encode_frame,
    ops::{Connection, Incoming, OpId, Outcome, Reply, Side},
};

pub(super) struct Wire {
    connection: Connection,
    connecting: Option<Reply<block_plugin_api::HelloAccepted>>,
    editors: HashMap<EditorInstanceId, Editor>,
    draws: BTreeMap<OpId, Awaited>,
    preferences: Watch<Preferences>,
    utc_offset: Watch<i32>,
    fonts: Watch<Fonts>,
    input: Watch<Vec<InputBatch>>,
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

    pub(super) fn take_outbound(&mut self) -> Vec<Vec<u8>> {
        self.connection.take_outbound()
    }

    pub(super) fn receive(&mut self, frame: &[u8], heard: &mut Vec<Message>) -> Result<(), String> {
        self.connection
            .receive(frame)
            .map_err(|error| format!("the plugin sent a frame that did not decode: {error}"))?;
        while let Some(event) = self.connection.next_event() {
            self.heard(event, heard)?;
        }
        Ok(())
    }

    fn heard(&mut self, event: Incoming, heard: &mut Vec<Message>) -> Result<(), String> {
        let connection = &mut self.connection;
        let call = match event {
            Incoming::Started(started) => match Call::accept(connection, started) {
                Some(call) => call,
                None => return Ok(()),
            },
            Incoming::Called(called) => match Call::accept_direct(&called) {
                Some(call) => call,
                None => {
                    return Err(format!(
                        "the plugin called {}, which the host does not answer",
                        called.name
                    ));
                }
            },
            Incoming::Completed { op, outcome } => {
                match (self.draws.remove(&op), outcome) {
                    (Some(Awaited::Draw), Outcome::Ok(bytes)) => {
                        let commit = block_plugin_api::ops::decode(&bytes)
                            .ok_or("the plugin answered a draw with a frame that did not decode")?;
                        heard.push(Message::Frame(commit));
                    }
                    (Some(Awaited::Shutdown), Outcome::Ok(_)) => {
                        heard.push(Message::ShutdownAcknowledged)
                    }
                    (Some(_), Outcome::Err(error)) => {
                        heard.push(Message::Error(ProtocolError {
                            request_id: None,
                            code: ErrorCode::Internal,
                            message: format!("the plugin did not answer: {error:?}"),
                        }));
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
                heard.push(Message::Hello(hello));
            }
            Call::Layout { layout } => heard.push(Message::Layout(layout)),
            Call::FrameNeeded {} => heard.push(Message::FrameNeeded),
            Call::MissingCharacters { characters } => {
                heard.push(Message::MissingCharacters(characters));
            }
            Call::PreferencesWatchNew { resource } => self.preferences.opened(resource.op()),
            Call::PreferencesWatchNext { reply, .. } => self.preferences.next(connection, reply),
            Call::UtcOffsetWatchNew { resource } => self.utc_offset.opened(resource.op()),
            Call::UtcOffsetWatchNext { reply, .. } => self.utc_offset.next(connection, reply),
            Call::FontsWatchNew { resource } => self.fonts.opened(resource.op()),
            Call::FontsWatchNext { reply, .. } => self.fonts.next(connection, reply),
            Call::InputNew { resource } => self.input.opened(resource.op()),
            Call::InputNext { reply, .. } => self.input.next(connection, reply),
            Call::Legacy { message } => heard.push(decode_frame(&message).map_err(|error| {
                format!("the plugin sent a message that did not decode: {error:?}")
            })?),
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
