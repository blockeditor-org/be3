use std::{cell::RefCell, collections::VecDeque, future::Future, rc::Rc};

use block_plugin_api::{
    BarAction, ChildId, EditorInstanceId, EditorMessage, EditorRegion, FrameCommit, Message,
    Opening, ScreenRequest, ScreenSet, encode_frame,
    guest::{
        exports::plugin::{Guest, GuestEditor},
        host,
    },
    ops::{
        OpId,
        guest::{Executor, Sender, oneshot, spawn},
    },
};

use crate::editor_session::EditorSession;

pub(crate) type EditorCall = Box<dyn FnOnce(&mut EditorSession)>;

/// What the host's calls leave for the runtime's next step, in the order they came.
pub(crate) enum Delivery {
    Message(Message),
    Editor(EditorInstanceId, EditorCall),
}

#[derive(Default)]
struct Wire {
    inbox: Vec<Delivery>,
    draws: VecDeque<Sender<FrameCommit>>,
    shutdowns: Vec<Sender<()>>,
}

#[derive(Clone, Default)]
pub(crate) struct Shared(Rc<RefCell<Wire>>);

impl Shared {
    pub(crate) fn executor(&self) -> Executor {
        block_plugin_api::guest::executor(Exports(self.clone()))
    }

    pub(crate) fn take_inbox(&self) -> Vec<Delivery> {
        std::mem::take(&mut self.0.borrow_mut().inbox)
    }

    fn push(&self, message: Message) {
        self.0.borrow_mut().inbox.push(Delivery::Message(message));
    }

    fn call(&self, instance: EditorInstanceId, call: impl FnOnce(&mut EditorSession) + 'static) {
        self.0
            .borrow_mut()
            .inbox
            .push(Delivery::Editor(instance, Box::new(call)));
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn watch(&self) {
        let wire = self.clone();
        spawn(async move {
            let watch = host::PreferencesWatch::new();
            while let Ok(preferences) = watch.next().await {
                wire.push(Message::Preferences(preferences));
            }
        });
        let wire = self.clone();
        spawn(async move {
            let watch = host::UtcOffsetWatch::new();
            while let Ok(seconds) = watch.next().await {
                wire.push(Message::UtcOffset(seconds));
            }
        });
        let wire = self.clone();
        spawn(async move {
            let watch = host::FontsWatch::new();
            while let Ok(fonts) = watch.next().await {
                wire.push(Message::Fonts(fonts));
            }
        });
        let wire = self.clone();
        spawn(async move {
            let input = host::Input::new();
            while let Ok(batches) = input.next().await {
                for batch in batches {
                    wire.push(Message::Input(batch));
                }
            }
        });
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn route(&self, messages: Vec<Message>) {
        for message in messages {
            match message {
                Message::Hello(hello) => {
                    let wire = self.clone();
                    spawn(async move {
                        if let Ok(accepted) = host::connect(hello).await {
                            wire.push(Message::HelloAccepted(accepted));
                        }
                    });
                }
                Message::Layout(layout) => host::layout(layout),
                Message::FrameNeeded => host::frame_needed(),
                Message::MissingCharacters(characters) => host::missing_characters(characters),
                Message::Acknowledged { .. } => {}
                Message::Frame(commit) => {
                    let draw = self.0.borrow_mut().draws.pop_front();
                    match draw {
                        Some(draw) => draw.send(commit),
                        None => legacy(&Message::Frame(commit)),
                    }
                }
                Message::ShutdownAcknowledged => {
                    let shutdowns = std::mem::take(&mut self.0.borrow_mut().shutdowns);
                    for shutdown in shutdowns {
                        shutdown.send(());
                    }
                }
                message => legacy(&message),
            }
        }
    }
}

fn legacy(message: &Message) {
    match encode_frame(message) {
        Ok(frame) => host::legacy(frame),
        Err(error) => eprintln!("dropped a message the protocol refused to carry: {error}"),
    }
}

struct Exports(Shared);

pub(crate) struct OpenEditor {
    wire: Shared,
    instance: EditorInstanceId,
}

impl Guest for Exports {
    type Editor = OpenEditor;

    fn new_editor(&self, _: OpId, opening: Opening) -> OpenEditor {
        let instance = opening.instance();
        self.0.push(Message::Editor(opening.into()));
        OpenEditor {
            wire: self.0.clone(),
            instance,
        }
    }

    fn screens(&self, screens: Vec<ScreenRequest>) {
        self.0.push(Message::Screens(ScreenSet {
            request_id: 0,
            screens,
        }));
    }

    fn configure(&self, serial: u64) {
        self.0.push(Message::Configure { serial });
    }

    fn draw(&self, now_micros: u64, seen: bool) -> impl Future<Output = FrameCommit> + 'static {
        let (sender, drawn) = oneshot();
        self.0.0.borrow_mut().draws.push_back(sender);
        self.0.push(Message::DrawFrame { now_micros, seen });
        async move {
            match drawn.await {
                Some(commit) => commit,
                None => std::future::pending().await,
            }
        }
    }

    fn shutdown(&self) -> impl Future<Output = ()> + 'static {
        let (sender, done) = oneshot();
        self.0.0.borrow_mut().shutdowns.push(sender);
        self.0.push(Message::Shutdown);
        async move {
            done.await;
        }
    }

    fn legacy(&self, message: Vec<u8>) {
        match block_plugin_api::decode_frame(&message) {
            Ok(message) => self.0.push(message),
            Err(error) => legacy(&Message::Error(block_plugin_api::ProtocolError {
                request_id: None,
                code: block_plugin_api::ErrorCode::InvalidMessage,
                message: format!("the host sent a message that did not decode: {error:?}"),
            })),
        }
    }
}

impl GuestEditor for OpenEditor {
    fn menu_pick(&self, id: String) {
        self.wire
            .call(self.instance, move |session| session.pick_menu(id));
    }

    fn child_bar(&self, _region: EditorRegion, child: ChildId, action: BarAction) {
        self.wire.call(self.instance, move |session| {
            session.child_bar_action(child, action)
        });
    }

    fn on_drop(&self) -> impl Future<Output = ()> + 'static {
        self.wire.push(Message::Editor(EditorMessage::Close {
            instance: self.instance,
        }));
        async {}
    }
}
