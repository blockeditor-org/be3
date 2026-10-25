use std::{cell::RefCell, collections::VecDeque, future::Future, rc::Rc};

use block_plugin_api::{
    BarAction, ChildId, EditorInstanceId, EditorMessage, EditorRegion, FrameCommit, Message,
    Opening, Rect, ScreenRequest, ScreenSet, ShellDialog, Size, encode_frame,
    guest::{
        exports::plugin::{Guest, GuestEditor},
        host, shell,
    },
    ops::{
        OpId,
        guest::{Executor, Sender, oneshot, spawn, with_parent},
    },
    shell_types::{DragOver, FileDrop, Focus},
};

use crate::editor_session::EditorSession;

pub(crate) type EditorCall = Box<dyn FnOnce(&mut EditorSession)>;

pub(crate) enum Delivery {
    Message(Message),
    Editor(EditorInstanceId, EditorCall),
    Screens(Box<dyn FnOnce(&mut crate::screens::Screens)>),
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

    fn screens(&self, call: impl FnOnce(&mut crate::screens::Screens) + 'static) {
        self.0
            .borrow_mut()
            .inbox
            .push(Delivery::Screens(Box::new(call)));
    }

    pub(crate) fn watch_shell(&self) {
        let wire = self.clone();
        spawn(async move {
            let watch = shell::BlockTypesWatch::new();
            while let Ok(catalog) = watch.next().await {
                wire.screens(move |screens| screens.set_catalog(&catalog));
            }
        });
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

    fn new_editor(&self, op: OpId, opening: Opening) -> OpenEditor {
        let instance = opening.instance();
        crate::asked::opened(instance, op);
        self.0.push(Message::Editor(opening.into()));
        let watch = with_parent(op, || shell::ChildStatusesWatch::new(instance));
        let wire = self.0.clone();
        spawn(async move {
            while let Ok(statuses) = watch.next().await {
                wire.call(instance, move |session| {
                    session.set_child_statuses(statuses)
                });
            }
        });
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

struct AskedPick {
    wire: Shared,
    instance: EditorInstanceId,
    pick: u64,
    answered: bool,
}

impl Drop for AskedPick {
    fn drop(&mut self) {
        if !self.answered {
            let pick = self.pick;
            self.wire
                .call(self.instance, move |session| session.withdraw_pick(pick));
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

    fn child_focused(&self, _region: EditorRegion, child: ChildId) {
        self.wire
            .call(self.instance, move |session| session.child_focused(child));
    }

    fn set_editable(&self, editable: bool) {
        self.wire
            .call(self.instance, move |session| session.set_editable(editable));
    }

    fn view_changed(&self, view: Rect, scale: f32) {
        self.wire
            .call(self.instance, move |session| session.set_view(view, scale));
    }

    fn presenting_changed(&self, presenting: bool) {
        self.wire.call(self.instance, move |session| {
            session.set_presenting(presenting)
        });
    }

    fn presence(&self, visible: bool) {
        self.wire.call(self.instance, move |session| {
            session.presence_visible(visible)
        });
    }

    fn focus_changed(&self, focus: Focus) {
        self.wire.call(self.instance, move |session| {
            session.set_focused_block(crate::host::FocusedBlock {
                block_id: focus.block_id.map(uuid::Uuid::from_bytes),
                block_type: uuid::Uuid::from_bytes(focus.block_type),
                via: focus.via.into_iter().map(uuid::Uuid::from_bytes).collect(),
            })
        });
    }

    fn resized(&self, size: Size) {
        self.wire.call(self.instance, move |session| {
            session.resized(geometry::vec2(size.width, size.height))
        });
    }

    fn drag_over(&self, drag: DragOver) {
        self.wire.call(self.instance, move |session| {
            session.set_drag(Some((
                drag.region,
                crate::host::BlockDrag {
                    position: drag.pos,
                    block_id: uuid::Uuid::from_bytes(drag.block_id),
                    block_type: uuid::Uuid::from_bytes(drag.block_type),
                    dropped: drag.dropped,
                },
            )))
        });
    }

    fn drag_left(&self) {
        self.wire
            .call(self.instance, move |session| session.set_drag(None));
    }

    fn file_drop(&self, drop: FileDrop) {
        self.wire.call(self.instance, move |session| {
            session.set_files(Some((
                drop.region,
                crate::host::FileDrop {
                    position: drop.pos,
                    files: drop
                        .files
                        .into_iter()
                        .map(|file| crate::PickedFile {
                            name: file.name,
                            data: file.data,
                        })
                        .collect(),
                    dropped: drop.dropped,
                },
            )))
        });
    }

    fn file_drop_left(&self) {
        self.wire
            .call(self.instance, move |session| session.set_files(None));
    }

    fn artifact_settings(&self, data: Vec<u8>) {
        self.wire.call(self.instance, move |session| {
            session.artifact_settings(data)
        });
    }

    fn commit_creation(&self) -> impl Future<Output = Result<[u8; 16], String>> + 'static {
        let (sender, created) = oneshot();
        self.wire.call(self.instance, move |session| {
            sender.send(session.commit_creation())
        });
        async move {
            created
                .await
                .unwrap_or_else(|| Err("the editor closed".to_owned()))
        }
    }

    fn regenerate_artifact(
        &self,
        data: Vec<u8>,
    ) -> impl Future<Output = Result<(), String>> + 'static {
        let (sender, regenerated) = oneshot();
        self.wire.call(self.instance, move |session| {
            session.regenerate_artifact(&data, sender)
        });
        async move {
            regenerated
                .await
                .unwrap_or_else(|| Err("the editor stopped regenerating the artifact".to_owned()))
        }
    }

    fn pick(
        &self,
        request: block_plugin_api::shell_types::PickRequest,
    ) -> impl Future<Output = block_plugin_api::BlockPick> + 'static {
        let (sender, answer) = oneshot();
        let pick = request.pick;
        self.wire.call(self.instance, move |session| {
            session.pick_requested(request, sender)
        });
        let asked = AskedPick {
            wire: self.wire.clone(),
            instance: self.instance,
            pick,
            answered: false,
        };
        async move {
            let mut asked = asked;
            let answer = answer.await;
            asked.answered = true;
            answer.unwrap_or(block_plugin_api::BlockPick::Cancelled)
        }
    }

    fn replace_child(&self, old: [u8; 16], new: [u8; 16]) -> impl Future<Output = bool> + 'static {
        let (sender, replaced) = oneshot();
        self.wire.call(self.instance, move |session| {
            sender.send(
                session.replace_child(uuid::Uuid::from_bytes(old), uuid::Uuid::from_bytes(new)),
            )
        });
        async move { replaced.await.unwrap_or(false) }
    }

    fn show_dialog(&self, block: [u8; 16], dialog: ShellDialog) {
        let block = uuid::Uuid::from_bytes(block);
        self.wire.call(self.instance, move |session| {
            session.show_dialog(block, dialog)
        });
    }

    fn on_drop(&self) -> impl Future<Output = ()> + 'static {
        crate::asked::closed(self.instance);
        self.wire.push(Message::Editor(EditorMessage::Close {
            instance: self.instance,
        }));
        async {}
    }
}
