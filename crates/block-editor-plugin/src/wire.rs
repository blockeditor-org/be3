use std::{cell::RefCell, collections::VecDeque, future::Future, rc::Rc};

use block_plugin_api::{
    BarAction, ChildId, EditorInstanceId, EditorRegion, Fonts, FrameCommit, Hello, HelloAccepted,
    InputBatch, Opening, Preferences, Rect, ScreenId, ScreenRequest, ShellDialog, Size,
    guest::{
        debug::debug,
        exports::{
            debug::paintings,
            plugin::{Guest, GuestEditor},
        },
        host, shell,
    },
    ops::{
        OpId,
        guest::{Executor, Sender, oneshot, spawn, with_parent},
    },
    shell_types::{DragOver, FileDrop, Focus},
};

use crate::asked::follow;
use crate::editor_session::EditorSession;

pub(crate) type EditorCall = Box<dyn FnOnce(&mut EditorSession)>;

pub(crate) enum Delivery {
    Accepted(HelloAccepted),
    Preferences(Preferences),
    UtcOffset(i32),
    Fonts(Fonts),
    Screens(Vec<ScreenRequest>),
    Configure(u64),
    Input(InputBatch),
    Draw { now_micros: u64, seen: bool },
    Shutdown,
    Open(Opening),
    Close(EditorInstanceId),
    Editor(EditorInstanceId, EditorCall),
    Call(Box<dyn FnOnce(&mut crate::screens::Screens)>),
    Paintings(Sender<Vec<u8>>),
}

impl Delivery {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Accepted(_) => "an acceptance",
            Self::Preferences(_) => "preferences",
            Self::UtcOffset(_) => "a UTC offset",
            Self::Fonts(_) => "fonts",
            Self::Screens(_) => "screens",
            Self::Configure(_) => "a configure",
            Self::Input(_) => "input",
            Self::Draw { .. } => "a draw request",
            Self::Shutdown => "a shutdown",
            Self::Open(_) => "an editor instance",
            Self::Close(_) => "a close",
            Self::Editor(..) => "an editor call",
            Self::Call(_) => "a call",
            Self::Paintings(_) => "a paintings request",
        }
    }
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

    fn push(&self, delivery: Delivery) {
        self.0.borrow_mut().inbox.push(delivery);
    }

    fn call(&self, instance: EditorInstanceId, call: impl FnOnce(&mut EditorSession) + 'static) {
        self.push(Delivery::Editor(instance, Box::new(call)));
    }

    fn screens(&self, call: impl FnOnce(&mut crate::screens::Screens) + 'static) {
        self.push(Delivery::Call(Box::new(call)));
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn drawing(&self) -> bool {
        !self.0.borrow().draws.is_empty()
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn drawn(&self, commit: FrameCommit) {
        let draw = self.0.borrow_mut().draws.pop_front();
        if let Some(draw) = draw {
            draw.send(commit);
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn shut_down(&self) {
        let shutdowns = std::mem::take(&mut self.0.borrow_mut().shutdowns);
        for shutdown in shutdowns {
            shutdown.send(());
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn connect(&self, hello: Hello) {
        let wire = self.clone();
        spawn(async move {
            if let Ok(accepted) = host::connect(hello).await {
                wire.push(Delivery::Accepted(accepted));
            }
        });
    }

    pub(crate) fn watch_shell(&self) {
        let wire = self.clone();
        follow::<shell::BlockTypesWatch>(None, move |catalog| {
            wire.screens(move |screens| screens.set_catalog(catalog));
        });
        let wire = self.clone();
        follow::<debug::DebugTemplatesWatch>(None, move |templates| {
            wire.screens(move |screens| screens.set_debug_templates(templates));
        });
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn watch(&self) {
        let wire = self.clone();
        follow::<host::PreferencesWatch>(None, move |preferences| {
            wire.push(Delivery::Preferences(preferences));
        });
        let wire = self.clone();
        follow::<host::UtcOffsetWatch>(None, move |seconds| {
            wire.push(Delivery::UtcOffset(seconds))
        });
        let wire = self.clone();
        follow::<host::FontsWatch>(None, move |fonts| wire.push(Delivery::Fonts(fonts)));
        let wire = self.clone();
        spawn(async move {
            let input = host::Input::new();
            while let Ok(batches) = input.next().await {
                for batch in batches {
                    wire.push(Delivery::Input(batch));
                }
            }
        });
    }
}

struct Exports(Shared);

pub(crate) struct OpenEditor {
    wire: Shared,
    instance: EditorInstanceId,
}

impl paintings::Guest for Exports {
    fn paintings(&self) -> impl Future<Output = Vec<u8>> + 'static {
        let (sender, painted) = oneshot();
        self.0.push(Delivery::Paintings(sender));
        async move { painted.await.unwrap_or_default() }
    }
}

impl Guest for Exports {
    type Editor = OpenEditor;

    fn new_editor(&self, op: OpId, opening: Opening) -> OpenEditor {
        let instance = opening.instance();
        crate::asked::opened(instance, op);
        self.0.push(Delivery::Open(opening));
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
        self.0.push(Delivery::Screens(screens));
    }

    fn describe(&self, on: bool) {
        self.0.screens(move |screens| screens.describe(on));
    }

    fn run_action(&self, screen: ScreenId, id: String) {
        self.0
            .screens(move |screens| screens.run_action(screen, &id));
    }

    fn node_action(
        &self,
        screen: ScreenId,
        request: block_plugin_api::accessibility::ActionRequest,
    ) {
        self.0
            .screens(move |screens| screens.node_action(screen, request.into()));
    }

    fn configure(&self, serial: u64) {
        self.0.push(Delivery::Configure(serial));
    }

    fn draw(&self, now_micros: u64, seen: bool) -> impl Future<Output = FrameCommit> + 'static {
        let (sender, drawn) = oneshot();
        self.0.0.borrow_mut().draws.push_back(sender);
        self.0.push(Delivery::Draw { now_micros, seen });
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
        self.0.push(Delivery::Shutdown);
        async move {
            done.await;
        }
    }
}

impl GuestEditor for OpenEditor {
    fn menu_pick(&self, id: String) {
        self.wire
            .call(self.instance, move |session| session.pick_menu(id));
    }

    fn dismiss(&self) {
        self.wire.call(self.instance, |session| session.dismiss());
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

    fn commit_creation(
        &self,
    ) -> impl Future<Output = Result<[u8; 16], block_plugin_api::Failure>> + 'static {
        let (sender, created) = oneshot();
        self.wire.call(self.instance, move |session| {
            sender.send(session.commit_creation())
        });
        async move {
            created
                .await
                .unwrap_or(Err(block_plugin_api::Failure::Unavailable))
        }
    }

    fn regenerate_artifact(
        &self,
        data: Vec<u8>,
    ) -> impl Future<Output = Result<(), block_plugin_api::Failure>> + 'static {
        let (sender, regenerated) = oneshot();
        self.wire.call(self.instance, move |session| {
            session.regenerate_artifact(&data, sender)
        });
        async move {
            regenerated
                .await
                .unwrap_or(Err(block_plugin_api::Failure::Unavailable))
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
        self.wire.push(Delivery::Close(self.instance));
        async {}
    }
}
