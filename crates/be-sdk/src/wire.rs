use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use be_sdk_api::{
    EditorInstanceId, Fonts, Hello, HelloAccepted, InputBatch, Opening, Preferences, PresentCommit,
    ScreenId, ScreenRequest,
    app::{ChildBarPress, DialogRequest, EditorEvent, MenuChoice, ViewFrame},
    guest::{
        debug::debug,
        exports::{
            app::{Guest, GuestEditor},
            debug::paintings,
        },
        host, shell,
    },
    ops::{
        OpId,
        guest::{Executor, LocalFuture, Sender, oneshot, spawn, with_parent},
    },
    shell_types::DragOver,
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
    Draw {
        now_micros: u64,
        wall_micros: u64,
        seen: bool,
    },
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
    draws: VecDeque<Sender<PresentCommit>>,
    shutdowns: Vec<Sender<()>>,
}

#[derive(Clone, Default)]
pub(crate) struct Shared(Rc<RefCell<Wire>>);

impl Shared {
    pub(crate) fn executor(&self) -> Executor {
        be_sdk_api::guest::executor(Exports(self.clone()))
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
    pub(crate) fn drawn(&self, commit: PresentCommit) {
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
    fn paintings(&self) -> LocalFuture<Vec<u8>> {
        let (sender, painted) = oneshot();
        self.0.push(Delivery::Paintings(sender));
        Box::pin(async move { painted.await.unwrap_or_default() })
    }
}

impl Guest for Exports {
    type Editor = OpenEditor;

    fn new_editor(&self, op: OpId, opening: Opening) -> OpenEditor {
        let instance = opening.instance;
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

    fn screens(&self, screens: Vec<ScreenRequest>) -> LocalFuture<()> {
        self.0.push(Delivery::Screens(screens));
        Box::pin(std::future::ready(()))
    }

    fn describe(&self, on: bool) -> LocalFuture<()> {
        self.0.screens(move |screens| screens.describe(on));
        Box::pin(std::future::ready(()))
    }

    fn run_action(&self, screen: ScreenId, id: String) -> LocalFuture<()> {
        self.0
            .screens(move |screens| screens.run_action(screen, &id));
        Box::pin(std::future::ready(()))
    }

    fn node_action(
        &self,
        screen: ScreenId,
        request: be_sdk_api::accessibility::ActionRequest,
    ) -> LocalFuture<()> {
        self.0
            .screens(move |screens| screens.node_action(screen, request.into()));
        Box::pin(std::future::ready(()))
    }

    fn configure(&self, serial: u64) -> LocalFuture<()> {
        self.0.push(Delivery::Configure(serial));
        Box::pin(std::future::ready(()))
    }

    fn draw(&self, now_micros: u64, wall_micros: u64, seen: bool) -> LocalFuture<PresentCommit> {
        let (sender, drawn) = oneshot();
        self.0.0.borrow_mut().draws.push_back(sender);
        self.0.push(Delivery::Draw {
            now_micros,
            wall_micros,
            seen,
        });
        Box::pin(async move {
            match drawn.await {
                Some(commit) => commit,
                None => std::future::pending().await,
            }
        })
    }

    fn shutdown(&self) -> LocalFuture<()> {
        let (sender, done) = oneshot();
        self.0.0.borrow_mut().shutdowns.push(sender);
        self.0.push(Delivery::Shutdown);
        Box::pin(async move {
            done.await;
        })
    }
}

impl OpenEditor {
    fn hear_one(&self, event: EditorEvent) {
        let wire = &self.wire;
        let instance = self.instance;
        match event {
            EditorEvent::MenuPick(MenuChoice { view, id, anchor }) => {
                wire.call(instance, move |session| session.pick_menu(view, id, anchor));
            }
            EditorEvent::Dismiss => wire.call(instance, |session| session.dismiss()),
            EditorEvent::ChildBar(ChildBarPress { child, action }) => {
                wire.call(instance, move |session| {
                    session.child_bar_action(child, action)
                });
            }
            EditorEvent::ChildFocused(child) => {
                wire.call(instance, move |session| session.child_focused(child));
            }
            EditorEvent::SetEditable(editable) => {
                wire.call(instance, move |session| session.set_editable(editable));
            }
            EditorEvent::ViewChanged(ViewFrame { view, scale }) => {
                wire.call(instance, move |session| session.set_view(view, scale));
            }
            EditorEvent::PresentingChanged(presenting) => {
                wire.call(instance, move |session| session.set_presenting(presenting));
            }
            EditorEvent::Presence(visible) => {
                wire.call(instance, move |session| session.presence_visible(visible));
            }
            EditorEvent::FocusChanged(focus) => {
                wire.call(instance, move |session| {
                    session.set_focused_block(crate::host::FocusedBlock {
                        block_id: focus.block_id.map(uuid::Uuid::from_bytes),
                        block_type: uuid::Uuid::from_bytes(focus.block_type),
                        editor: focus.editor.map(uuid::Uuid::from_bytes),
                        via: focus.via.into_iter().map(uuid::Uuid::from_bytes).collect(),
                    })
                });
            }
            EditorEvent::Resized(size) => {
                wire.call(instance, move |session| {
                    session.resized(be_lib_geometry::vec2(size.width, size.height))
                });
            }
            EditorEvent::Drag(drag) => {
                let dragged = drag.map(|drag| (drag.view, dragged(&drag)));
                wire.call(instance, move |session| {
                    session.set_drag(dragged.and_then(|(view, dragged)| dragged.map(|d| (view, d))))
                });
            }
            EditorEvent::ArtifactSettings(data) => {
                wire.call(instance, move |session| session.artifact_settings(data));
            }
            EditorEvent::ShowDialog(DialogRequest { block, dialog }) => {
                let block = uuid::Uuid::from_bytes(block);
                wire.call(instance, move |session| session.show_dialog(block, dialog));
            }
        }
    }
}

/// What a drag over the editor carries: a block, or files.
fn dragged(drag: &DragOver) -> Option<crate::host::Dragged> {
    use be_sdk_api::clipboard::{Blocks, Files};
    let blocks = drag.items.iter().find_map(|item| item.decode::<Blocks>());
    let files = drag.items.iter().find_map(|item| item.decode::<Files>());
    match (
        blocks.and_then(|blocks| blocks.blocks.into_iter().next()),
        files,
    ) {
        (Some(block), _) => Some(crate::host::Dragged::Block(crate::host::BlockDrag {
            position: drag.pos,
            block_id: block.id,
            block_type: block.block_type,
            dropped: drag.dropped,
            action: drag.action,
        })),
        (None, Some(files)) => Some(crate::host::Dragged::Files(crate::host::FileDrop {
            position: drag.pos,
            files: files
                .files
                .into_iter()
                .map(|file| crate::PickedFile {
                    name: file.name,
                    data: file.data,
                })
                .collect(),
            dropped: drag.dropped,
        })),
        (None, None) => None,
    }
}

impl GuestEditor for OpenEditor {
    fn hear(&self, events: Vec<EditorEvent>) -> LocalFuture<()> {
        for event in events {
            self.hear_one(event);
        }
        Box::pin(std::future::ready(()))
    }

    fn commit_creation(&self) -> LocalFuture<Result<[u8; 16], be_sdk_api::Failure>> {
        let (sender, created) = oneshot();
        self.wire.call(self.instance, move |session| {
            sender.send(session.commit_creation())
        });
        Box::pin(async move {
            created
                .await
                .unwrap_or(Err(be_sdk_api::Failure::Unavailable))
        })
    }

    fn regenerate_artifact(&self, data: Vec<u8>) -> LocalFuture<Result<(), be_sdk_api::Failure>> {
        let (sender, regenerated) = oneshot();
        self.wire.call(self.instance, move |session| {
            session.regenerate_artifact(&data, sender)
        });
        Box::pin(async move {
            regenerated
                .await
                .unwrap_or(Err(be_sdk_api::Failure::Unavailable))
        })
    }

    fn render_clip(
        &self,
        serial: u64,
        format: String,
    ) -> LocalFuture<Option<be_sdk_api::clipboard::ClipBody>> {
        let (sender, rendered) = oneshot();
        self.wire.call(self.instance, move |session| {
            session.render_clip(serial, &format, sender)
        });
        Box::pin(async move { rendered.await.flatten() })
    }

    fn replace_child(&self, old: [u8; 16], new: [u8; 16]) -> LocalFuture<bool> {
        let (sender, replaced) = oneshot();
        self.wire.call(self.instance, move |session| {
            sender.send(
                session.replace_child(uuid::Uuid::from_bytes(old), uuid::Uuid::from_bytes(new)),
            )
        });
        Box::pin(async move { replaced.await.unwrap_or(false) })
    }

    fn on_drop(&self) -> LocalFuture<()> {
        crate::asked::closed(self.instance);
        self.wire.push(Delivery::Close(self.instance));
        Box::pin(async {})
    }
}
