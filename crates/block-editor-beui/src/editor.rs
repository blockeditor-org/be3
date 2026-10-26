use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use be_block::presence::{ChannelKind, PresenceKind};
use beui::reactive::*;
use beui::{Document, Pos2, Rect, Vec2};
use block_plugin_api::{
    BarAction, ChildContent, ChildId, ChildLayer, ChildMode, ChildPreview, CreationProgress,
    DialogFrame, DialogStatus, EditorCapabilities, HostPanel, HostWindowId, InteractionMode,
    MenuEntry, ResizeMode, TopBar, Watched, WebViewId,
};
use block_ui::BlockCatalog;
use uuid::Uuid;
use workspace_ui_types::{EditorViewContent, ViewState};

use crate::{
    BlockFilter, BlockList, BlockParent, BlockQuery, Blocks, ContentProjection, EditorHost,
    OwnedContentProjection, PickedBlock, Task, Waker,
};
use crate::{BlockHistory, FileDrop, FocusedBlock, Pushed};
use block_plugin_api::{AudioStatus, FitArea};

type Regenerate = Rc<dyn Fn(&[u8])>;
type PollArtifact = Rc<dyn Fn() -> Option<Result<(), String>>>;
type ReplaceChild = Rc<dyn Fn(Uuid, Uuid) -> bool>;

pub struct Artifacts(Rc<ArtifactState>);

struct ArtifactState {
    host: EditorHost,
    artifact: crate::Artifact,
    regenerate: RefCell<Option<Regenerate>>,
    poll: RefCell<Option<PollArtifact>>,
    settings: OwnedSignal<Vec<u8>>,
    saved: OwnedSignal<Vec<u8>>,
    edit: RefCell<Option<Vec<u8>>>,
}

impl Clone for Artifacts {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl Artifacts {
    pub fn new(host: EditorHost, artifact: crate::Artifact) -> Self {
        defer_graph_changes(&host);
        Self(Rc::new(ArtifactState {
            host,
            artifact,
            regenerate: RefCell::new(None),
            poll: RefCell::new(None),
            settings: OwnedSignal::new(Vec::new()),
            saved: OwnedSignal::new(Vec::new()),
            edit: RefCell::new(None),
        }))
    }

    pub fn settings(&self) -> ReadSignal<Vec<u8>> {
        self.0.settings.read_only()
    }

    pub fn edit_settings(&self, data: Vec<u8>) {
        self.0.settings.set(data.clone());
        *self.0.edit.borrow_mut() = Some(data);
    }

    pub fn saved_settings(&self) -> ReadSignal<Vec<u8>> {
        self.0.saved.read_only()
    }

    pub fn receive_settings(&self, saved: &[u8], draft: &[u8]) {
        if self.0.saved.get_untracked() != saved {
            self.0.saved.set(saved.to_vec());
        }
        if self.0.settings.get_untracked() != draft {
            self.0.settings.set(draft.to_vec());
        }
    }

    pub fn take_settings_edit(&self) -> Option<Vec<u8>> {
        self.0.edit.borrow_mut().take()
    }

    pub fn host(&self) -> &EditorHost {
        &self.0.host
    }

    pub fn blocks(&self) -> Blocks {
        self.0.host.blocks()
    }

    pub fn block_id(&self) -> Uuid {
        self.0.artifact.block_id
    }

    pub fn block_type(&self) -> Uuid {
        self.0.artifact.block_type
    }

    pub fn on_regenerate(&self, work: impl Fn(&[u8]) + 'static) {
        *self.0.regenerate.borrow_mut() = Some(Rc::new(work));
    }

    pub fn on_poll(&self, work: impl Fn() -> Option<Result<(), String>> + 'static) {
        *self.0.poll.borrow_mut() = Some(Rc::new(work));
    }

    pub fn regenerate(&self, data: &[u8]) {
        let work = self.0.regenerate.borrow().clone();
        if let Some(work) = work {
            work(data);
        }
    }

    pub fn poll(&self) -> Option<Result<(), String>> {
        let work = self.0.poll.borrow().clone();
        work.and_then(|work| work())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    pub position: Pos2,
    pub block_id: Uuid,
    pub block_type: Uuid,
    pub dropped: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChildTarget {
    pub id: Uuid,
    pub block_type: Uuid,
    pub view_block: Option<Uuid>,
    pub editor: Option<Uuid>,
}

impl ChildTarget {
    pub fn new(id: Uuid, block_type: Uuid) -> Self {
        Self {
            id,
            block_type,
            view_block: None,
            editor: None,
        }
    }

    pub fn edited_with(mut self, editor: Option<Uuid>) -> Self {
        self.editor = editor;
        self
    }

    pub fn viewed_by(mut self, view_block: Uuid) -> Self {
        self.view_block = Some(view_block);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubregionContent {
    Block(ChildTarget),
    WebView(WebViewId),
    Host(HostPanel),
    Window(HostWindowId),
    Creation { editor: Uuid, template: String },
    ArtifactSettings { block: Uuid },
}

impl From<ChildTarget> for SubregionContent {
    fn from(target: ChildTarget) -> Self {
        Self::Block(target)
    }
}

impl SubregionContent {
    fn wire(&self) -> ChildContent {
        match self {
            Self::Block(target) => ChildContent::Block {
                block_id: target.id.into_bytes(),
                block_type: target.block_type.into_bytes(),
                view_block: target.view_block.map(Uuid::into_bytes),
                editor: target.editor.map(Uuid::into_bytes),
            },
            Self::WebView(web_view) => ChildContent::WebView(*web_view),
            Self::Host(panel) => ChildContent::Host(*panel),
            Self::Window(window) => ChildContent::Window(*window),
            Self::Creation { editor, template } => ChildContent::Creation {
                editor: editor.into_bytes(),
                template: template.clone(),
            },
            Self::ArtifactSettings { block } => ChildContent::ArtifactSettings {
                block_id: block.into_bytes(),
            },
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChildState {
    pub placed: bool,
    pub available: bool,
    pub active: bool,
    pub intrinsic_size: Option<Vec2>,
    pub aspect_ratio: Option<f32>,
    pub interaction: Option<InteractionMode>,
    pub capabilities: EditorCapabilities,
    pub resize: ResizeMode,
    pub error: Option<String>,
    pub child: Option<ChildId>,
    pub menu: Vec<MenuEntry>,
    pub creation: Option<CreationProgress>,
    pub preview: ChildPreview,
    pub dialog: DialogStatus,
}

impl ChildState {
    fn of(host: &EditorHost, child: Option<ChildId>) -> Self {
        let Some(status) = child.and_then(|child| host.child_status(child)) else {
            return Self {
                placed: child.is_some(),
                child,
                ..Self::default()
            };
        };
        Self {
            placed: true,
            available: status.available,
            active: status.active,
            intrinsic_size: status
                .intrinsic
                .map(|size| Vec2::new(size.width, size.height)),
            aspect_ratio: status.aspect_ratio,
            interaction: Some(status.interaction),
            capabilities: status.capabilities,
            resize: status.resize,
            error: status.error,
            child,
            menu: block_plugin_api::MenuEntry::nest(status.menu).unwrap_or_default(),
            creation: status.creation,
            preview: status.preview,
            dialog: status.dialog,
        }
    }
}

struct ChildRecord {
    slot: EmbedSlot,
    content: Prop<Option<SubregionContent>>,
    mode: Prop<ChildMode>,
    layer: Prop<ChildLayer>,
    own_frame: Prop<bool>,
    top_bar: Prop<TopBar>,
    rotation: Prop<f32>,
    opacity: Prop<f32>,
    intrinsic: Prop<Option<Vec2>>,
    dialog: Prop<Option<DialogFrame>>,
    state: Signal<ChildState>,
    report: Callback<ChildState>,
    bar: Callback<BarAction>,
    focus: Callback<()>,
    child: Cell<Option<ChildId>>,
    zone: u64,
}

type Revision = OwnedSignal<u64>;

type Wake = (Arc<AtomicU64>, Signal<u64>);

fn then<T: 'static>(
    host: &EditorHost,
    asked: impl std::future::Future<Output = Option<T>> + 'static,
    answered: impl FnOnce(T) + 'static,
) -> Task {
    let owner = owner_scope();
    host.then(asked, move |answer| {
        if owner.as_ref().is_none_or(ScopeContext::is_alive) {
            answered(answer);
        }
    })
}

struct Mirror([Revision; Pushed::ALL.len()]);

impl Mirror {
    fn new(host: &EditorHost) -> Self {
        Self(Pushed::ALL.map(|pushed| OwnedSignal::new(host.revision(pushed))))
    }

    fn watch(&self, pushed: Pushed) -> ReadSignal<u64> {
        self.0[pushed as usize].read_only()
    }

    fn track(&self, pushed: Pushed) {
        self.0[pushed as usize].get();
    }

    fn sync(&self, host: &EditorHost) {
        for pushed in Pushed::ALL {
            self.0[pushed as usize].set(host.revision(pushed));
        }
    }
}

pub fn fit_content(available: Rect, content: Vec2) -> Rect {
    if content.x <= 0.0 || content.y <= 0.0 {
        return Rect::ZERO;
    }
    let scale = (available.width() / content.x)
        .min(available.height() / content.y)
        .max(f32::EPSILON);
    let size = Vec2::new(content.x * scale, content.y * scale);
    Rect::from_min_size(
        beui::Pos2::new(
            available.center().x - size.x / 2.0,
            available.center().y - size.y / 2.0,
        ),
        size,
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BeuiScale {
    pub(crate) ratio: f32,
    pub(crate) pixels_per_point: f32,
}

impl Default for BeuiScale {
    fn default() -> Self {
        Self {
            ratio: 1.0,
            pixels_per_point: 1.0,
        }
    }
}

pub(crate) fn defer_graph_changes(host: &EditorHost) {
    host.defer_graph_changes_unless(|| beui::reactive::try_with_document(|_| ()).is_some());
}

type Pump = Rc<dyn Fn()>;

#[derive(Default)]
pub(crate) struct Contents {
    projections: RefCell<std::collections::HashMap<Option<Uuid>, Box<dyn std::any::Any>>>,
    pumps: RefCell<std::collections::HashMap<Option<Uuid>, Pump>>,
    due: Rc<RefCell<Vec<Option<Uuid>>>>,
}

#[derive(Clone, Copy)]
pub struct Editor(Stored<EditorState>);

pub struct OwnedEditor {
    editor: Editor,
    _state: OwnedStored<EditorState>,
}

impl OwnedEditor {
    pub fn new(host: EditorHost, block: Uuid) -> Self {
        Self::scaled(host, block, Rc::default(), Rc::default())
    }

    pub(crate) fn scaled(
        host: EditorHost,
        block: Uuid,
        beui: Rc<Cell<BeuiScale>>,
        contents: Rc<Contents>,
    ) -> Self {
        defer_graph_changes(&host);
        let owner = RootScope::new();
        let canvas = create_signal(None::<CanvasView>);
        let world = create_signal(None::<Vec2>);
        let scale = create_signal(1.0_f32);
        let chrome = create_signal(true);
        let placed_content = create_signal(None::<Rect>);
        let editable = create_signal(host.editable());
        let presenting = create_signal(false);
        let drag = create_signal(None::<Drag>);
        let pixels_per_point = create_signal(1.0_f32);
        let resized = create_signal(None::<Vec2>);
        let presence_visible = create_signal(false);
        let revealed = create_signal(None::<u64>);
        let files = create_signal(None::<FileDrop>);
        let placed = create_signal(Rect::ZERO);
        let pushed = Mirror::new(&host);
        let state = OwnedStored::new(EditorState {
            _scope: owner.into_scope(),
            host,
            beui,
            block,
            roots: RefCell::new(None),
            canvas,
            world,
            scale,
            chrome,
            placed_content,
            editable,
            presenting,
            drag,
            pixels_per_point,
            resized,
            pending_resize: Cell::new(None),
            presence_visible,
            pending_presence: Cell::new(None),
            revealed,
            pending_reveal: Cell::new(None),
            replace: RefCell::new(None),
            content: RefCell::new(None),
            contents,
            content_rect: Cell::new(Rect::ZERO),
            intrinsic: Cell::new(None),
            children: RefCell::new(Vec::new()),
            next_child: Cell::new(0),
            pick: RefCell::new(None),
            seen: Cell::new(None),
            shown_rect: Cell::new(None),
            children_moved: Cell::new(false),
            wakes: Rc::default(),
            pushed,
            watches: RefCell::default(),
            files,
            placed,
        });
        Self {
            editor: Editor(state.handle()),
            _state: state,
        }
    }

    pub fn editor(&self) -> Editor {
        self.editor
    }

    pub(crate) fn contents(&self) -> Rc<Contents> {
        Rc::clone(&self.editor.state().contents)
    }
}

impl std::ops::Deref for OwnedEditor {
    type Target = Editor;

    fn deref(&self) -> &Editor {
        &self.editor
    }
}

struct EditorState {
    _scope: Scope,
    host: EditorHost,
    beui: Rc<Cell<BeuiScale>>,
    block: Uuid,
    roots: RefCell<Option<BlockList>>,
    canvas: Signal<Option<CanvasView>>,
    world: Signal<Option<Vec2>>,
    scale: Signal<f32>,
    chrome: Signal<bool>,
    placed_content: Signal<Option<Rect>>,
    editable: Signal<bool>,
    presenting: Signal<bool>,
    drag: Signal<Option<Drag>>,
    pixels_per_point: Signal<f32>,
    resized: Signal<Option<Vec2>>,
    pending_resize: Cell<Option<Vec2>>,
    presence_visible: Signal<bool>,
    pending_presence: Cell<Option<bool>>,
    revealed: Signal<Option<u64>>,
    pending_reveal: Cell<Option<u64>>,
    replace: RefCell<Option<ReplaceChild>>,
    content: RefCell<Option<(NodeRef, u64)>>,
    contents: Rc<Contents>,
    content_rect: Cell<Rect>,
    intrinsic: Cell<Option<Vec2>>,
    children: RefCell<Vec<(u64, Rc<ChildRecord>)>>,
    next_child: Cell<u64>,
    pick: RefCell<Option<Task>>,
    seen: Cell<Option<(u64, f32)>>,
    shown_rect: Cell<Option<Rect>>,
    children_moved: Cell<bool>,
    wakes: Rc<RefCell<Vec<Wake>>>,
    pushed: Mirror,
    watches: RefCell<std::collections::HashMap<std::any::TypeId, Revision>>,
    files: Signal<Option<FileDrop>>,
    placed: Signal<Rect>,
}

impl Editor {
    fn state(&self) -> Rc<EditorState> {
        self.0.value()
    }

    pub fn host(&self) -> EditorHost {
        self.state().host.clone()
    }

    pub fn blocks(&self) -> Blocks {
        self.state().host.blocks()
    }

    pub fn block_id(&self) -> Uuid {
        self.state().block
    }

    pub fn watch_blocks(&self, query: BlockQuery) -> Memo<Ready<Vec<crate::BlockInfo>>> {
        let list = self.blocks().watch(query);
        create_memo(move || list.read())
    }

    pub fn editable(&self) -> ReadSignal<bool> {
        self.state().editable.read_only()
    }

    pub fn read_only(&self) -> Memo<bool> {
        let editable = self.state().editable;
        create_memo(move || !editable.get())
    }

    pub fn block_content<C>(&self) -> ContentProjection<C>
    where
        C: be_block::LiveEdit + Clone + Default,
    {
        self.projection(None)
    }

    pub fn content_of<C>(&self, block: Uuid) -> ContentProjection<C>
    where
        C: be_block::LiveEdit + Clone + Default,
    {
        if block == self.state().block {
            return self.block_content();
        }
        self.state().host.watch_content(block, C::CONTENT_TYPE);
        let projection = self.projection_of(Some(block));
        if Boundary::current().is_none() {
            return projection;
        }
        let found = self.blocks().watch(BlockQuery::Block(block));
        wait_for(move || {
            if !projection.is_alive() || projection.ready().is_ok() {
                return Ok(());
            }
            let viewable = found
                .read()?
                .first()
                .is_some_and(|info| info.access.can_view());
            if viewable {
                Err(Pending::Loading)
            } else {
                Ok(())
            }
        });
        projection
    }

    pub fn view_block(&self) -> Option<Uuid> {
        self.state().host.view_block()
    }

    pub fn view_content(&self) -> Option<ContentProjection<EditorViewContent>> {
        self.view_block()
            .map(|block| self.content_of::<EditorViewContent>(block))
    }

    pub fn view_state(&self, key: &str) -> Option<ViewState> {
        self.view_content()?
            .read(|view| view.root().state(key).cloned())
            .ok()
            .flatten()
    }

    pub fn set_view_state(&self, key: &str, state: Option<&ViewState>) {
        let Some(view) = self.view_content() else {
            return;
        };
        let client = self.state().host.client_id();
        let now = now_milliseconds();
        let edit = untrack(|| {
            view.read(|held| {
                let root = held.root();
                (root.state(key) != state).then(|| root.set_state(key, state, now, client))
            })
        });
        if let Some(edit) = edit.ok().flatten() {
            view.operate_anyway(edit);
        }
    }

    pub fn related_content<C>(&self, block: Memo<Option<Uuid>>) -> crate::RelatedContent<C>
    where
        C: be_block::LiveEdit + Clone + Default,
    {
        crate::RelatedContent::new(*self, block)
    }

    pub fn seed_content<C: be_block::BlockContent>(&self, block: Uuid, content: &C) {
        self.state().host.seed_content(block, content);
    }

    pub fn replace_content<C: be_block::BlockContent>(&self, block: Uuid, content: &C) {
        self.state().host.replace_content(block, content);
    }

    pub fn create_child<C: be_block::BlockContent>(&self, content: &C) -> Uuid {
        self.blocks()
            .create(content, BlockParent::Block(self.state().block))
    }

    pub fn stored_head(&self, block: Uuid) -> Memo<crate::HeadRead> {
        let host = self.state().host.clone();
        host.watch_head(block);
        let revision = self.pushed(Pushed::Heads);
        let read = create_memo(move || {
            revision.get();
            host.head(block)
        });
        wait_for(move || match read.with(Option::is_some) {
            true => Ok(()),
            false => Err(Pending::Loading),
        });
        create_memo(move || read.get().unwrap_or(Ok(None)))
    }

    fn projection<C>(&self, block: Option<Uuid>) -> ContentProjection<C>
    where
        C: be_block::LiveEdit + Clone + Default,
    {
        let projection = self.projection_of(block);
        wait_for(move || {
            if projection.is_alive() {
                projection.ready()
            } else {
                Ok(())
            }
        });
        projection
    }

    fn projection_of<C>(&self, block: Option<Uuid>) -> ContentProjection<C>
    where
        C: be_block::LiveEdit + Clone + Default,
    {
        let state = self.state();
        let cached = state
            .contents
            .projections
            .borrow()
            .get(&block)
            .and_then(|held| held.downcast_ref::<OwnedContentProjection<C>>())
            .map(OwnedContentProjection::handle);
        if let Some(source) = cached {
            return source;
        }
        let owned = OwnedContentProjection::<C>::new(state.host.clone(), block);
        let source = owned.handle();
        let due = Rc::downgrade(&state.contents.due);
        source.on_due(move || {
            if let Some(due) = due.upgrade() {
                due.borrow_mut().push(block);
            }
        });
        state
            .contents
            .pumps
            .borrow_mut()
            .insert(block, Rc::new(move || source.pump()));
        state.contents.due.borrow_mut().push(block);
        state
            .contents
            .projections
            .borrow_mut()
            .insert(block, Box::new(owned));
        source
    }

    pub fn pushed(&self, pushed: Pushed) -> ReadSignal<u64> {
        self.state().pushed.watch(pushed)
    }

    pub fn then<T: 'static>(
        &self,
        asked: impl std::future::Future<Output = Option<T>> + 'static,
        answered: impl FnOnce(T) + 'static,
    ) -> Task {
        then(&self.state().host, asked, answered)
    }

    pub fn woken(&self) -> (Waker, ReadSignal<u64>) {
        let count = Arc::new(AtomicU64::new(0));
        let woken = create_signal(0);
        self.state()
            .wakes
            .borrow_mut()
            .push((Arc::clone(&count), woken));
        let wakes = Rc::downgrade(&self.state().wakes);
        let counted = Arc::clone(&count);
        on_cleanup(move || {
            if let Some(wakes) = wakes.upgrade() {
                wakes
                    .borrow_mut()
                    .retain(|(held, _)| !Arc::ptr_eq(held, &counted));
            }
        });
        (self.state().host.waker().counting(count), woken.read_only())
    }

    pub fn audio(&self) -> Memo<AudioStatus> {
        let host = self.state().host.clone();
        let revision = self.pushed(Pushed::Audio);
        create_memo(move || {
            revision.get();
            host.audio()
        })
    }

    pub fn history(&self, block: Uuid) -> Memo<BlockHistory> {
        let host = self.state().host.clone();
        host.watch_history([block]);
        let revision = self.pushed(Pushed::Histories);
        create_memo(move || {
            revision.get();
            host.history(block)
        })
    }

    pub fn version_status(&self) -> Memo<block_editor_plugin::VersionStatus> {
        let host = self.state().host.clone();
        host.watch_version();
        let revision = self.pushed(Pushed::Version);
        let known = create_memo({
            let host = host.clone();
            move || {
                revision.get();
                host.version_known()
            }
        });
        wait_for(move || match known.get() {
            true => Ok(()),
            false => Err(Pending::Loading),
        });
        create_memo(move || {
            revision.get();
            host.version_status()
        })
    }

    pub fn version(&self, command: block_editor_plugin::VersionCommand) {
        self.state().host.version(self.state().block, command);
    }

    pub fn histories(&self) -> ReadSignal<u64> {
        self.pushed(Pushed::Histories)
    }

    pub fn artifacts(&self) -> ReadSignal<u64> {
        self.pushed(Pushed::Artifacts)
    }

    pub fn focused_block(&self) -> Memo<FocusedBlock> {
        let host = self.state().host.clone();
        let revision = self.pushed(Pushed::Focus);
        create_memo(move || {
            revision.get();
            host.focused_block()
        })
    }

    pub fn take_panel_requests(&self) -> Vec<HostPanel> {
        self.state().host.take_panel_requests()
    }

    pub fn watch<W: Watched>(&self) -> Memo<W::Value>
    where
        W::Value: Default,
    {
        let host = self.state().host.clone();
        let revision = self.watch_revision::<W>();
        create_memo(move || {
            revision.get();
            host.watched::<W>().unwrap_or_default()
        })
    }

    pub fn offered<W: Watched>(&self) -> Memo<bool> {
        let host = self.state().host.clone();
        let revision = self.watch_revision::<W>();
        create_memo(move || {
            revision.get();
            host.offers::<W>()
        })
    }

    fn watch_revision<W: Watched>(&self) -> ReadSignal<u64> {
        let host = &self.state().host;
        self.state()
            .watches
            .borrow_mut()
            .entry(std::any::TypeId::of::<W>())
            .or_insert_with(|| OwnedSignal::new(host.watch_revision::<W>()))
            .read_only()
    }

    pub fn act(&self, call: impl FnOnce() + 'static) {
        self.state().host.act(call);
    }

    pub fn web_view_events(&self) -> ReadSignal<u64> {
        self.pushed(Pushed::WebView)
    }

    pub fn files(&self) -> ReadSignal<Option<FileDrop>> {
        self.state().files.read_only()
    }

    pub fn canvas(&self) -> ReadSignal<Option<CanvasView>> {
        self.state().canvas.read_only()
    }

    pub fn world(&self) -> ReadSignal<Option<Vec2>> {
        self.state().world.read_only()
    }

    pub fn scale(&self) -> ReadSignal<f32> {
        self.state().scale.read_only()
    }

    pub fn chrome_shown(&self) -> ReadSignal<bool> {
        self.state().chrome.read_only()
    }

    pub fn placed_content(&self) -> ReadSignal<Option<Rect>> {
        self.state().placed_content.read_only()
    }

    pub fn presenting(&self) -> ReadSignal<bool> {
        self.state().presenting.read_only()
    }

    pub fn present(&self, presenting: bool) {
        self.state().host.present(presenting);
    }

    pub fn block_types(&self) -> Rc<BlockCatalog> {
        self.state().pushed.track(Pushed::Catalog);
        self.state().host.block_types()
    }

    pub fn block_types_known(&self) -> Memo<bool> {
        let host = self.state().host.clone();
        let revision = self.pushed(Pushed::Catalog);
        let known = create_memo(move || {
            revision.get();
            host.block_types_known()
        });
        wait_for(move || match known.get() {
            true => Ok(()),
            false => Err(Pending::Loading),
        });
        known
    }

    pub fn set_intrinsic_size(&self, size: Option<Vec2>) {
        self.state().intrinsic.set(size);
    }

    pub fn intrinsic_size(&self) -> Option<Vec2> {
        self.state().intrinsic.get()
    }

    pub fn pick_block(
        &self,
        filter: BlockFilter,
        picked: impl FnOnce(Result<PickedBlock, block_editor_plugin::Failure>) + 'static,
    ) {
        let asked = self.state().host.pick_block(filter);
        let task = self.then(asked, move |answer| match answer {
            Ok(Some(block)) => picked(Ok(block)),
            Ok(None) => {}
            Err(error) => picked(Err(error)),
        });
        if let Some(earlier) = self.state().pick.replace(Some(task)) {
            earlier.cancel();
        }
    }

    pub(crate) fn register_child(
        &self,
        slot: EmbedSlot,
        content: Prop<Option<SubregionContent>>,
        mode: Prop<ChildMode>,
        layer: Prop<ChildLayer>,
        own_frame: Prop<bool>,
        top_bar: Prop<TopBar>,
        rotation: Prop<f32>,
        opacity: Prop<f32>,
        intrinsic: Prop<Option<Vec2>>,
        dialog: Prop<Option<DialogFrame>>,
        report: Callback<ChildState>,
        bar: Callback<BarAction>,
        focus: Callback<()>,
    ) -> ReadSignal<ChildState> {
        let editor_state = self.state();
        let state = create_signal(ChildState::default());
        let key = editor_state.next_child.get();
        editor_state.next_child.set(key + 1);
        editor_state.children.borrow_mut().push((
            key,
            Rc::new(ChildRecord {
                slot,
                content,
                mode,
                layer,
                own_frame,
                top_bar,
                rotation,
                opacity,
                intrinsic,
                dialog,
                state,
                report,
                bar,
                focus,
                child: Cell::new(None),
                zone: document_zone(),
            }),
        ));
        let editor = self.0;
        on_cleanup(move || {
            editor.try_with(|editor| {
                editor
                    .children
                    .borrow_mut()
                    .retain(|(other, _)| *other != key);
            });
        });
        state.read_only()
    }

    fn records(&self) -> Vec<Rc<ChildRecord>> {
        self.state()
            .children
            .borrow()
            .iter()
            .map(|(_, record)| Rc::clone(record))
            .collect()
    }

    pub fn content(&self, node: &NodeRef) {
        *self.state().content.borrow_mut() = Some((*node, document_zone()));
    }

    pub fn content_rect(&self) -> Rect {
        self.state().content_rect.get()
    }

    pub fn placed(&self) -> ReadSignal<Rect> {
        self.state().placed.read_only()
    }

    pub fn pixels_per_point(&self) -> ReadSignal<f32> {
        self.state().pixels_per_point.read_only()
    }

    pub fn resized(&self) -> ReadSignal<Option<Vec2>> {
        self.state().resized.read_only()
    }

    pub fn report_resize(&self, size: Vec2) {
        self.state().pending_resize.set(Some(size));
    }

    pub fn show<P: PresenceKind>(&self, value: Option<&P>) {
        let value = value.and_then(|value| serde_json::to_vec(value).ok());
        self.state().host.show_presence(None, P::ID, value);
    }

    pub fn peers<P>(&self) -> ReadSignal<Vec<(u64, P)>>
    where
        P: PresenceKind + Clone + PartialEq + 'static,
    {
        let peers = create_signal(Vec::new());
        let host = self.state().host.clone();
        host.ask_for_peers(None);
        let revision = self.pushed(Pushed::Peers);
        let seen = Cell::new(0);
        create_effect(move || {
            revision.get();
            let Some((latest, held)) = host.peers_since(None, seen.get()) else {
                return;
            };
            seen.set(latest);
            let decoded: Vec<(u64, P)> = held
                .iter()
                .filter(|peer| peer.kind == P::ID)
                .filter_map(|peer| {
                    serde_json::from_slice(&peer.value)
                        .ok()
                        .map(|value| (peer.client, value))
                })
                .collect();
            peers.set(decoded);
        });
        peers.read_only()
    }

    pub fn send_to_peers<M: ChannelKind>(&self, to: Option<u64>, message: &M) {
        self.state()
            .host
            .send_to_peers(None, to, M::ID, message.encode());
    }

    pub fn on_peer_message<M>(&self, receive: impl Fn(u64, M) + 'static)
    where
        M: ChannelKind + 'static,
    {
        let host = self.state().host.clone();
        host.ask_for_peers(None);
        let revision = self.pushed(Pushed::PeerMessages);
        let seen = Cell::new(host.peer_messages_seen());
        create_effect(move || {
            revision.get();
            let received = host.peer_messages_since(None, seen.get());
            let Some((last, _)) = received.last() else {
                return;
            };
            seen.set(*last);
            untrack(|| {
                for (_, message) in received {
                    if message.channel != M::ID {
                        continue;
                    }
                    if let Some(decoded) = M::decode(&message.payload) {
                        receive(message.from, decoded);
                    }
                }
            });
        });
    }

    pub fn presence_visible(&self) -> ReadSignal<bool> {
        self.state().presence_visible.read_only()
    }

    pub fn report_presence_visible(&self, visible: bool) {
        self.state().pending_presence.set(Some(visible));
    }

    pub fn revealed(&self) -> ReadSignal<Option<u64>> {
        self.state().revealed.read_only()
    }

    pub fn report_reveal(&self, client_id: u64) {
        self.state().pending_reveal.set(Some(client_id));
    }

    pub fn on_replace_child(&self, replace: impl Fn(Uuid, Uuid) -> bool + 'static) {
        *self.state().replace.borrow_mut() = Some(Rc::new(replace));
    }

    pub fn replace_child(&self, old: Uuid, new: Uuid) -> bool {
        let replace = self.state().replace.borrow().clone();
        replace.is_some_and(|replace| replace(old, new))
    }

    pub fn drag(&self) -> ReadSignal<Option<Drag>> {
        self.state().drag.read_only()
    }

    pub fn accept_drag(&self, accepted: bool) {
        self.state().host.accept_drag(accepted);
    }

    fn ratio(&self) -> f32 {
        self.state().beui.get().ratio
    }

    fn view_rect(&self) -> Option<Rect> {
        let ratio = self.ratio();
        self.state().host.view().map(|rect| rect.scaled(ratio))
    }

    fn view_scale(&self) -> f32 {
        self.state().host.view_scale().unwrap_or(1.0)
    }

    fn view_canvas(&self) -> Option<CanvasView> {
        let scale = self.view_scale();
        self.view_rect()
            .map(|rect| CanvasView::new(rect.min, scale))
    }

    pub fn pan(&self, delta: Vec2) {
        self.state().host.pan_view(delta / self.ratio());
    }

    pub fn zoom(&self, factor: f32) {
        self.state().host.zoom_view(factor, None);
    }

    pub fn zoom_at(&self, factor: f32, anchor: Pos2) {
        let ratio = self.ratio();
        self.state()
            .host
            .zoom_view(factor, Some(Pos2::new(anchor.x / ratio, anchor.y / ratio)));
    }

    pub(crate) fn keep_fitted_area(&self, area: Option<FitArea>) {
        self.state().host.keep_fitted(area);
    }

    pub(crate) fn fit_view(&self) {
        self.state().host.fit_view();
    }

    pub(crate) fn host_rect(&self, rect: Rect) -> FitArea {
        let ratio = self.ratio();
        FitArea::Rect(rect.scaled(1.0 / ratio))
    }

    pub fn pan_drag(&self) {
        self.state().host.pan_drag_view();
    }

    pub fn reveal(&self, target: Rect) {
        let Some(placement) = self.view_canvas() else {
            return;
        };
        let shown = placement.rect_to_screen(target).center();
        self.pan(self.state().content_rect.get().center() - shown);
    }

    pub fn begin_frame(&self) {
        let state = self.state();
        let ratio = self.ratio();
        let seen = Some((state.host.changes(), ratio));
        let pushed = state.seen.replace(seen) != seen;
        if pushed {
            state.host.flush_graph();
        }
        let mut due = std::mem::take(&mut *state.contents.due.borrow_mut());
        if pushed {
            due.extend(state.host.updated_content());
        }
        self.pump(due);
        let wakes = state.wakes.borrow().clone();
        for (count, woken) in wakes {
            woken.set(count.load(Ordering::Acquire));
        }
        if pushed {
            state.pushed.sync(&state.host);
            let watches: Vec<_> = state
                .watches
                .borrow()
                .iter()
                .map(|(watch, revision)| (*watch, revision.handle()))
                .collect();
            for (watch, revision) in watches {
                revision.set(state.host.watch_revision_of(watch));
            }
            state
                .files
                .set(state.host.files().map(|files| crate::FileDrop {
                    position: Pos2::new(files.position.x * ratio, files.position.y * ratio),
                    ..files
                }));
            state.chrome.set(state.host.chrome_shown());
            state
                .placed_content
                .set(state.host.placed_content().map(|rect| rect.scaled(ratio)));
            state.editable.set(state.host.editable());
            state.presenting.set(state.host.presenting());
            state.drag.set(state.host.drag().map(|drag| Drag {
                position: Pos2::new(drag.position.x * ratio, drag.position.y * ratio),
                block_id: drag.block_id,
                block_type: drag.block_type,
                dropped: drag.dropped,
            }));
        }
        let rect = state.content_rect.get();
        if pushed || state.shown_rect.replace(Some(rect)) != Some(rect) {
            state.placed.set(rect);
            let scale = self.view_scale();
            let divisor = scale.max(f32::EPSILON);
            state.canvas.set(self.view_canvas());
            state.world.set(
                self.view_rect()
                    .map(|rect| Vec2::new(rect.width() / divisor, rect.height() / divisor)),
            );
            state.scale.set(scale);
        }
        state
            .pixels_per_point
            .set(state.beui.get().pixels_per_point);
        state.resized.set(state.pending_resize.take());
        if let Some(visible) = state.pending_presence.take() {
            state.presence_visible.set(visible);
        }
        state.revealed.set(state.pending_reveal.take());
        if pushed || state.children_moved.take() {
            self.follow_children();
        }
    }

    fn pump(&self, blocks: Vec<Option<Uuid>>) {
        let pumps: Vec<_> = {
            let state = self.state();
            let held = state.contents.pumps.borrow();
            let mut pumped = std::collections::HashSet::new();
            blocks
                .into_iter()
                .filter(|block| pumped.insert(*block))
                .filter_map(|block| held.get(&block).cloned())
                .collect()
        };
        for pump in pumps {
            pump();
        }
    }

    fn follow_children(&self) {
        let state = self.state();
        for record in self.records() {
            if let Some(child) = record.child.get() {
                for action in state.host.take_child_bar_actions(child) {
                    record.bar.call(action);
                }
                if state.host.take_child_focus(child) {
                    record.focus.call(());
                }
            }
            let state = ChildState::of(&state.host, record.child.get());
            if record.state.get_untracked() == state {
                continue;
            }
            record.state.set(state.clone());
            record.report.call(state);
        }
    }

    pub fn end_frame(&self, document: &Document, window: beui::WindowId) {
        let state = self.state();
        let zone = document.zone();
        let mut records: Vec<(Vec<usize>, Rc<ChildRecord>)> = self
            .records()
            .into_iter()
            .filter(|record| record.zone == 0 || record.zone == zone)
            .map(|record| {
                let order = record
                    .slot
                    .node()
                    .map(|node| document.paint_order(node))
                    .unwrap_or_default();
                (order, record)
            })
            .collect();
        records.sort_by(|(left, _), (right, _)| left.cmp(right));
        let unscale = self.ratio().recip();
        let overlays = document.overlay_layers(window);
        let mut overlays = overlays.iter().peekable();
        for (order, record) in records {
            while let Some((_, rect)) = overlays.next_if(|(layer, _)| *layer < order) {
                state.host.occlude(rect.scaled(unscale));
            }
            let child = self.place_child(document, &record);
            if record.child.replace(child) != child {
                state.children_moved.set(true);
            }
        }
        for (_, rect) in overlays {
            state.host.occlude(rect.scaled(unscale));
        }
        let node = state
            .content
            .borrow()
            .as_ref()
            .filter(|(_, owner)| *owner == 0 || *owner == zone)
            .and_then(|(node, _)| node.try_get());
        let Some(rect) = node.and_then(|node| document.node_rect(node)) else {
            return;
        };
        state.content_rect.set(rect);
        if state.placed.get_untracked() != rect {
            state.host.request_frame_in(std::time::Duration::ZERO);
        }
        state.host.report_content(rect.scaled(unscale));
    }
}

fn document_zone() -> u64 {
    beui::reactive::try_with_document(|document| document.zone()).unwrap_or(0)
}

impl Editor {
    fn place_child(&self, document: &Document, record: &ChildRecord) -> Option<ChildId> {
        let state = self.state();
        let node = record.slot.node()?;
        let rect = document.node_rect(node)?;
        let placement = record.slot.placement()?;
        let content = record.content.get_untracked()?;
        let unscale = self.ratio().recip();
        let shown = document
            .window_rect(document.window_of(node))
            .unwrap_or_else(|| document.viewport_rect());
        Some(state.host.place_child(
            content.wire(),
            rect.scaled(unscale),
            placement.clip.scaled(unscale),
            record.mode.get_untracked(),
            record.layer.get_untracked(),
            record.own_frame.get_untracked(),
            record.top_bar.get_untracked(),
            record.rotation.get_untracked(),
            record.opacity.get_untracked(),
            record.intrinsic.get_untracked(),
            document.panned_by_canvas(node),
            record.dialog.get_untracked().map(|dialog| DialogFrame {
                bottom_inset: dialog.bottom_inset.max(rect.max.y - shown.max.y) * unscale,
                ..dialog
            }),
        ))
    }
}

type Maker = Rc<dyn Fn() -> Result<Uuid, String>>;

#[derive(Clone)]
pub struct Creation(Rc<CreationState>);

struct CreationState {
    host: EditorHost,
    template: String,
    maker: RefCell<Option<Maker>>,
    pushed: Mirror,
    seen: Cell<Option<u64>>,
}

impl Creation {
    pub const MAIN: &str = "main";

    pub fn new(host: EditorHost) -> Self {
        Self::for_template(host, Self::MAIN)
    }

    pub fn for_template(host: EditorHost, template: impl Into<String>) -> Self {
        defer_graph_changes(&host);
        Self(Rc::new(CreationState {
            pushed: Mirror::new(&host),
            host,
            template: template.into(),
            maker: RefCell::new(None),
            seen: Cell::new(None),
        }))
    }

    pub fn host(&self) -> &EditorHost {
        &self.0.host
    }

    pub fn template(&self) -> &str {
        &self.0.template
    }

    pub fn blocks(&self) -> Blocks {
        self.0.host.blocks()
    }

    pub fn create<C: be_block::BlockContent>(&self, content: &C) -> Uuid {
        self.blocks().create(content, BlockParent::Detached)
    }

    pub fn seed_content<C: be_block::BlockContent>(&self, block: Uuid, content: &C) {
        self.0.host.seed_content(block, content);
    }

    pub fn accept(&self) {
        self.0
            .host
            .close_dialog(block_plugin_api::DialogAnswer::Accepted);
    }

    pub fn cancel(&self) {
        self.0
            .host
            .close_dialog(block_plugin_api::DialogAnswer::Dismissed);
    }

    pub fn on_create(&self, make: impl Fn() -> Result<Uuid, String> + 'static) {
        *self.0.maker.borrow_mut() = Some(Rc::new(make));
    }

    pub fn then<T: 'static>(
        &self,
        asked: impl std::future::Future<Output = Option<T>> + 'static,
        answered: impl FnOnce(T) + 'static,
    ) -> Task {
        then(&self.0.host, asked, answered)
    }

    pub fn create_block(&self) -> Result<Uuid, String> {
        let maker = self.0.maker.borrow().clone();
        match maker {
            Some(make) => make(),
            None => Err("this editor does not create blocks".to_owned()),
        }
    }

    pub fn begin_frame(&self) {
        let changes = Some(self.0.host.changes());
        if self.0.seen.replace(changes) != changes {
            self.0.pushed.sync(&self.0.host);
        }
    }
}

impl Editor {
    pub fn effective_settings<R>(&self) -> Memo<R>
    where
        R: be_block::Layered + PartialEq + 'static,
        be_block::be_model::Document<R>: be_block::LiveEdit + Clone + Default,
    {
        let editor = *self;
        create_memo(move || {
            let layers = crate::root_settings::RootSettings::default()
                .read(&editor)
                .map(|held| {
                    crate::root_settings::layers(
                        &editor,
                        &held,
                        R::CONTENT_TYPE,
                        editor.host().client_id(),
                    )
                })
                .unwrap_or_default();
            be_block::layered(layers.bottom_up().into_iter().filter_map(|block| {
                editor
                    .content_of::<be_block::be_model::Document<R>>(block)
                    .read(|content| content.root())
                    .ok()
            }))
        })
    }

    pub fn inherited_settings<R>(&self) -> Memo<R>
    where
        R: be_block::Layered + PartialEq + 'static,
        be_block::be_model::Document<R>: be_block::LiveEdit + Clone + Default,
    {
        let editor = *self;
        create_memo(move || {
            let below = crate::root_settings::RootSettings::default()
                .read(&editor)
                .map(|held| {
                    crate::root_settings::layers(
                        &editor,
                        &held,
                        R::CONTENT_TYPE,
                        editor.host().client_id(),
                    )
                    .below(editor.block_id())
                })
                .unwrap_or_default();
            be_block::layered(below.into_iter().filter_map(|block| {
                editor
                    .content_of::<be_block::be_model::Document<R>>(block)
                    .read(|content| content.root())
                    .ok()
            }))
        })
    }
}

impl crate::root_settings::SettingsGraph for Editor {
    fn roots(&self) -> Option<Vec<(Uuid, Uuid, Uuid)>> {
        let state = self.state();
        let mut roots = state.roots.borrow_mut();
        let list = roots.get_or_insert_with(|| self.blocks().watch(BlockQuery::Roots));
        list.read().ok().map(|list| {
            list.into_iter()
                .filter_map(|info| Some((info.id, info.block_type, info.author?)))
                .collect()
        })
    }

    fn account(&self) -> Uuid {
        self.state().host.account_id()
    }

    fn settings(&self, block: Uuid) -> Option<be_block::Settings> {
        self.content_of::<be_block::SettingsContent>(block)
            .read(|settings| settings.root())
            .ok()
    }

    fn create(&self, block_type: Uuid, content: Vec<u8>, parent: BlockParent) -> Uuid {
        self.blocks().create_with(
            block_type,
            Some(content),
            parent,
            be_block::BlockMetadata::default(),
        )
    }

    fn settings_profile(&self, block: Uuid) -> Option<be_block::SettingsProfile> {
        self.content_of::<be_block::SettingsProfileContent>(block)
            .read(|profile| profile.root())
            .ok()
    }

    fn edit_settings(&self, block: Uuid, edit: be_block::Edit) {
        self.content_of::<be_block::SettingsContent>(block)
            .operate(edit);
    }

    fn edit_settings_profile(&self, block: Uuid, edit: be_block::Edit) {
        self.content_of::<be_block::SettingsProfileContent>(block)
            .operate(edit);
    }
}

pub fn node_anchor(node: &NodeRef) -> Option<Rect> {
    let node = node.try_get()?;
    try_with_document(|document| document.node_rect(node)).flatten()
}

#[cfg(test)]
mod tests;

fn now_milliseconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}
