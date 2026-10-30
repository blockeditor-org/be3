use beui::dock::unstyled::DockTabControl;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use be_block::presence::{ChannelKind, PresenceKind};
use be_block::{EditorViewContent, ViewState};
use beui::native::Document;
use beui::native::EmbedSlot;
use beui::native::try_with_document;
use beui::reactive::*;
use beui::{CanvasView, Pos2, Rect, Vec2};
use block_plugin_api::{
    BarAction, BlockPick, ChildId, ChildLayer, ChildMode, DialogFrame, HostPanel, TopBar, Watched,
};
use block_ui::BlockCatalog;
use uuid::Uuid;

use crate::{
    ArtifactsCore, BlockFilter, BlockList, BlockParent, BlockQuery, Blocks, ChildState,
    ContentCache, ContentProjection, CreationCore, Drag, EditorCore, EditorHost,
    OwnedContentProjection, PickedBlock, PlacedChild, PointScale, ShownEditor, SubregionContent,
    Subscription, Task, Waker,
};
use crate::{BlockHistory, FileDrop, FocusedBlock, Pushed};
use block_plugin_api::{AudioStatus, FitArea};

#[derive(Clone)]
pub struct Artifacts(Rc<ArtifactSignals>);

struct ArtifactSignals {
    core: ArtifactsCore,
    settings: OwnedSignal<Vec<u8>>,
    saved: OwnedSignal<Vec<u8>>,
    _changes: [Subscription; 2],
}

impl Artifacts {
    pub fn new(host: EditorHost, artifact: crate::Artifact) -> Self {
        defer_graph_changes(&host);
        let core = ArtifactsCore::new(host, artifact);
        let settings = OwnedSignal::new(core.settings());
        let saved = OwnedSignal::new(core.saved_settings());
        let (drafted, kept) = (settings.handle(), saved.handle());
        let changes = [
            core.on_settings(move |data| drafted.set(data.to_vec())),
            core.on_saved_settings(move |data| kept.set(data.to_vec())),
        ];
        Self(Rc::new(ArtifactSignals {
            core,
            settings,
            saved,
            _changes: changes,
        }))
    }

    pub fn settings(&self) -> ReadSignal<Vec<u8>> {
        self.0.settings.read_only()
    }

    pub fn saved_settings(&self) -> ReadSignal<Vec<u8>> {
        self.0.saved.read_only()
    }

    pub fn blocks(&self) -> Blocks {
        Blocks::of(self.0.core.host())
    }
}

impl std::ops::Deref for Artifacts {
    type Target = ArtifactsCore;

    fn deref(&self) -> &ArtifactsCore {
        &self.0.core
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
    placed: PlacedChild,
    zone: u64,
    dock_tab: Option<u64>,
    docks: bool,
    tab: Option<DockTabControl>,
}

type Revision = OwnedSignal<u64>;

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

#[derive(Clone, Copy)]
enum Changed {
    Pushed(Pushed),
    Watch(std::any::TypeId),
}

struct Mirror {
    host: EditorHost,
    pushed: [Revision; Pushed::ALL.len()],
    watches: RefCell<std::collections::HashMap<std::any::TypeId, (Subscription, Revision)>>,
    changed: Rc<RefCell<Vec<Changed>>>,
    _pushes: Vec<Subscription>,
}

impl Mirror {
    fn new(host: &EditorHost) -> Self {
        let changed: Rc<RefCell<Vec<Changed>>> = Rc::default();
        let pushes = Pushed::ALL
            .into_iter()
            .map(|pushed| {
                let changed = Rc::downgrade(&changed);
                host.on_pushed(pushed, move || {
                    if let Some(changed) = changed.upgrade() {
                        changed.borrow_mut().push(Changed::Pushed(pushed));
                    }
                })
            })
            .collect();
        Self {
            host: host.clone(),
            pushed: Pushed::ALL.map(|pushed| OwnedSignal::new(host.revision(pushed))),
            watches: RefCell::default(),
            changed,
            _pushes: pushes,
        }
    }

    fn watch(&self, pushed: Pushed) -> ReadSignal<u64> {
        self.pushed[pushed as usize].read_only()
    }

    fn track(&self, pushed: Pushed) {
        self.pushed[pushed as usize].get();
    }

    fn watched(&self, watch: std::any::TypeId) -> ReadSignal<u64> {
        self.watches
            .borrow_mut()
            .entry(watch)
            .or_insert_with(|| {
                let changed = Rc::downgrade(&self.changed);
                let changes = self.host.on_watched_of(watch, move || {
                    if let Some(changed) = changed.upgrade() {
                        changed.borrow_mut().push(Changed::Watch(watch));
                    }
                });
                (
                    changes,
                    OwnedSignal::new(self.host.watch_revision_of(watch)),
                )
            })
            .1
            .read_only()
    }

    fn sync(&self) {
        let changed = std::mem::take(&mut *self.changed.borrow_mut());
        for changed in changed {
            match changed {
                Changed::Pushed(pushed) => {
                    self.pushed[pushed as usize].set(self.host.revision(pushed));
                }
                Changed::Watch(watch) => {
                    let revision = self
                        .watches
                        .borrow()
                        .get(&watch)
                        .map(|(_, revision)| revision.handle());
                    if let Some(revision) = revision {
                        revision.set(self.host.watch_revision_of(watch));
                    }
                }
            }
        }
    }
}

pub(crate) fn defer_graph_changes(host: &EditorHost) {
    host.defer_graph_changes_unless(|| beui::native::try_with_document(|_| ()).is_some());
}

#[derive(Default)]
pub(crate) struct Contents {
    live: Rc<ContentCache>,
    projections: RefCell<std::collections::HashMap<Option<Uuid>, Box<dyn std::any::Any>>>,
}

#[derive(Clone, Copy)]
pub struct Editor(Stored<EditorState>);

pub struct OwnedEditor {
    editor: Editor,
    _state: OwnedStored<EditorState>,
}

#[derive(Clone, Copy)]
struct ShownSignals {
    files: Signal<Option<FileDrop>>,
    chrome: Signal<bool>,
    placed_content: Signal<Option<Rect>>,
    frame_anchor: Signal<Option<Rect>>,
    editable: Signal<bool>,
    presenting: Signal<bool>,
    drag: Signal<Option<Drag>>,
    placed: Signal<Rect>,
    canvas: Signal<Option<CanvasView>>,
    world: Signal<Option<Vec2>>,
    scale: Signal<f32>,
    pixels_per_point: Signal<f32>,
    resized: Signal<Option<Vec2>>,
    presence_visible: Signal<bool>,
    revealed: Signal<Option<u64>>,
}

impl ShownSignals {
    fn new(shown: &ShownEditor) -> Self {
        Self {
            files: create_signal(shown.files.clone()),
            chrome: create_signal(shown.chrome),
            placed_content: create_signal(shown.placed_content),
            frame_anchor: create_signal(shown.frame_anchor),
            editable: create_signal(shown.editable),
            presenting: create_signal(shown.presenting),
            drag: create_signal(shown.drag),
            placed: create_signal(shown.placed),
            canvas: create_signal(shown.canvas),
            world: create_signal(shown.world),
            scale: create_signal(shown.scale),
            pixels_per_point: create_signal(shown.pixels_per_point),
            resized: create_signal(shown.resized),
            presence_visible: create_signal(shown.presence_visible),
            revealed: create_signal(shown.revealed),
        }
    }

    fn set(&self, shown: &ShownEditor) {
        self.files.set(shown.files.clone());
        self.chrome.set(shown.chrome);
        self.placed_content.set(shown.placed_content);
        self.frame_anchor.set(shown.frame_anchor);
        self.editable.set(shown.editable);
        self.presenting.set(shown.presenting);
        self.drag.set(shown.drag);
        self.placed.set(shown.placed);
        self.canvas.set(shown.canvas);
        self.world.set(shown.world);
        self.scale.set(shown.scale);
        self.pixels_per_point.set(shown.pixels_per_point);
        self.resized.set(shown.resized);
        self.presence_visible.set(shown.presence_visible);
        self.revealed.set(shown.revealed);
    }
}

impl OwnedEditor {
    pub fn new(host: EditorHost, block: Uuid) -> Self {
        Self::scaled(host, block, Rc::default(), Rc::default())
    }

    pub(crate) fn scaled(
        host: EditorHost,
        block: Uuid,
        scale: Rc<Cell<PointScale>>,
        contents: Rc<Contents>,
    ) -> Self {
        defer_graph_changes(&host);
        let core = EditorCore::scaled(host.clone(), block, scale, Rc::clone(&contents.live));
        let owner = RootScope::new();
        let shown = ShownSignals::new(&core.shown());
        let showing = core.on_shown(move |now| shown.set(now));
        let frame_children = create_signal(Vec::<(u64, Memo<Vec<MenuNode>>)>::new());
        let picking = create_signal(None::<crate::picking::Picking>);
        let popped = create_signal(None::<crate::popover::Popped>);
        let pushed = Mirror::new(&host);
        let state = OwnedStored::new(EditorState {
            _scope: owner.into_scope(),
            core,
            _showing: showing,
            shown,
            roots: RefCell::new(None),
            settings: RefCell::default(),
            content: RefCell::new(None),
            contents,
            children: RefCell::new(Vec::new()),
            next_child: Cell::new(0),
            picking,
            popped,
            next_pick: Cell::new(0),
            picker_size: Cell::new(None),
            fitted: Cell::new(None),
            pushed,
            frame_children,
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
    core: EditorCore,
    _showing: Subscription,
    shown: ShownSignals,
    roots: RefCell<Option<BlockList>>,
    settings: RefCell<settings::SettingsCache>,
    content: RefCell<Option<(NodeRef, u64)>>,
    contents: Rc<Contents>,
    children: RefCell<Vec<(u64, Rc<ChildRecord>)>>,
    next_child: Cell<u64>,
    picking: Signal<Option<crate::picking::Picking>>,
    popped: Signal<Option<crate::popover::Popped>>,
    next_pick: Cell<u64>,
    picker_size: Cell<Option<Vec2>>,
    fitted: Cell<Option<NodeRef>>,
    pushed: Mirror,
    frame_children: Signal<Vec<(u64, Memo<Vec<MenuNode>>)>>,
}

impl Editor {
    fn state(&self) -> Rc<EditorState> {
        self.0.value()
    }

    pub fn core(&self) -> EditorCore {
        self.state().core.clone()
    }

    pub fn host(&self) -> EditorHost {
        self.state().core.host().clone()
    }

    pub fn blocks(&self) -> Blocks {
        Blocks::of(self.state().core.host())
    }

    pub fn block_id(&self) -> Uuid {
        self.state().core.block_id()
    }

    pub fn watch_blocks(&self, query: BlockQuery) -> Memo<Ready<Vec<crate::BlockInfo>>> {
        let list = self.blocks().watch(query);
        create_memo(move || list.read())
    }

    pub fn editable(&self) -> ReadSignal<bool> {
        self.state().shown.editable.read_only()
    }

    pub fn read_only(&self) -> Memo<bool> {
        let editable = self.state().shown.editable;
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
        if block == self.block_id() {
            return self.block_content();
        }
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
        self.state().core.host().view_block()
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
        self.state().core.set_view_state(key, state);
    }

    pub fn related_content<C>(&self, block: Memo<Option<Uuid>>) -> crate::RelatedContent<C>
    where
        C: be_block::LiveEdit + Clone + Default,
    {
        crate::RelatedContent::new(*self, block)
    }

    pub fn seed_content<C: be_block::BlockContent>(&self, block: Uuid, content: &C) {
        self.state().core.host().seed_content(block, content);
    }

    pub fn replace_content<C: be_block::BlockContent>(&self, block: Uuid, content: &C) {
        self.state().core.host().replace_content(block, content);
    }

    pub fn create_child<C: be_block::BlockContent>(&self, content: &C) -> Uuid {
        self.state().core.create_child(content)
    }

    pub fn stored_head(&self, block: Uuid) -> Memo<crate::HeadRead> {
        let host = self.host();
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
        let live = match block {
            Some(block) => state.core.content_of::<C>(block),
            None => state.core.content::<C>(None),
        };
        let owned = OwnedContentProjection::from(live);
        let source = owned.handle();
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
        then(self.state().core.host(), asked, answered)
    }

    pub fn woken(&self) -> (Waker, ReadSignal<u64>) {
        let woken = create_signal(0);
        let (waker, wakes) = self.state().core.woken(move |count| woken.set(count));
        crate::content::keep(wakes);
        (waker, woken.read_only())
    }

    pub fn audio(&self) -> Memo<AudioStatus> {
        let host = self.host();
        let revision = self.pushed(Pushed::Audio);
        create_memo(move || {
            revision.get();
            host.audio()
        })
    }

    pub fn history(&self, block: Uuid) -> Memo<BlockHistory> {
        let host = self.host();
        host.watch_history([block]);
        let revision = self.pushed(Pushed::Histories);
        create_memo(move || {
            revision.get();
            host.history(block)
        })
    }

    pub fn version_status(&self) -> Memo<block_editor_plugin::VersionStatus> {
        let host = self.host();
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
        let core = self.core();
        create_memo(move || {
            revision.get();
            core.version_status().unwrap_or_default()
        })
    }

    pub fn version(&self, command: block_editor_plugin::VersionCommand) {
        self.state().core.version(command);
    }

    pub fn histories(&self) -> ReadSignal<u64> {
        self.pushed(Pushed::Histories)
    }

    pub fn artifacts(&self) -> ReadSignal<u64> {
        self.pushed(Pushed::Artifacts)
    }

    pub fn focused_block(&self) -> Memo<FocusedBlock> {
        let host = self.host();
        let revision = self.pushed(Pushed::Focus);
        create_memo(move || {
            revision.get();
            host.focused_block()
        })
    }

    pub fn take_panel_requests(&self) -> Vec<HostPanel> {
        self.state().core.host().take_panel_requests()
    }

    pub fn watch<W: Watched>(&self) -> Memo<W::Value>
    where
        W::Value: Default,
    {
        let host = self.host();
        let revision = self.watch_revision::<W>();
        create_memo(move || {
            revision.get();
            host.watched::<W>().unwrap_or_default()
        })
    }

    pub fn offered<W: Watched>(&self) -> Memo<bool> {
        let host = self.host();
        let revision = self.watch_revision::<W>();
        create_memo(move || {
            revision.get();
            host.offers::<W>()
        })
    }

    fn watch_revision<W: Watched>(&self) -> ReadSignal<u64> {
        self.state().pushed.watched(std::any::TypeId::of::<W>())
    }

    pub fn act(&self, call: impl FnOnce() + 'static) {
        self.state().core.host().act(call);
    }

    pub fn web_view_events(&self) -> ReadSignal<u64> {
        self.pushed(Pushed::WebView)
    }

    pub fn files(&self) -> ReadSignal<Option<FileDrop>> {
        self.state().shown.files.read_only()
    }

    pub fn canvas(&self) -> ReadSignal<Option<CanvasView>> {
        self.state().shown.canvas.read_only()
    }

    pub fn world(&self) -> ReadSignal<Option<Vec2>> {
        self.state().shown.world.read_only()
    }

    pub fn scale(&self) -> ReadSignal<f32> {
        self.state().shown.scale.read_only()
    }

    pub fn chrome_shown(&self) -> ReadSignal<bool> {
        self.state().shown.chrome.read_only()
    }

    pub fn placed_content(&self) -> ReadSignal<Option<Rect>> {
        self.state().shown.placed_content.read_only()
    }

    pub fn frame_anchor(&self) -> ReadSignal<Option<Rect>> {
        self.state().shown.frame_anchor.read_only()
    }

    pub fn presenting(&self) -> ReadSignal<bool> {
        self.state().shown.presenting.read_only()
    }

    pub fn present(&self, presenting: bool) {
        self.state().core.host().present(presenting);
    }

    pub fn block_types(&self) -> Rc<BlockCatalog> {
        self.state().pushed.track(Pushed::Catalog);
        self.state().core.host().block_types()
    }

    pub fn block_types_known(&self) -> Memo<bool> {
        let host = self.host();
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

    pub fn fit_to(&self, node: Option<NodeRef>) {
        self.state().fitted.set(node);
    }

    pub(crate) fn fit_intrinsic(&self, document: &mut Document, context: &beui::native::Context) {
        let Some(node) = self.state().fitted.get().and_then(|node| node.try_get()) else {
            return;
        };
        let node = document.content_of_overlay(node).unwrap_or(node);
        let measured = document
            .measure_node(context, node, Vec2::splat(f32::INFINITY))
            .filter(|size| {
                size.x.is_finite() && size.y.is_finite() && size.x > 0.0 && size.y > 0.0
            });
        if measured.is_some() && self.intrinsic_size() != measured {
            self.set_intrinsic_size(measured);
        }
    }

    pub fn set_intrinsic_size(&self, size: Option<Vec2>) {
        self.state().core.set_intrinsic_size(size);
    }

    pub fn intrinsic_size(&self) -> Option<Vec2> {
        self.state().core.intrinsic_size()
    }

    pub fn pick_block(
        &self,
        filter: BlockFilter,
        anchor: impl Into<crate::PickAnchor>,
        picked: impl FnOnce(Result<PickedBlock, block_editor_plugin::Failure>) + 'static,
    ) {
        let state = self.state();
        let serial = state.next_pick.get() + 1;
        state.next_pick.set(serial);
        let mut filter = filter;
        let parent = match self.block_id() {
            own if own.is_nil() => BlockParent::Root,
            own => {
                if !filter.excluded.contains(&own.into_bytes()) {
                    filter.excluded.push(own.into_bytes());
                }
                BlockParent::Block(own)
            }
        };
        let touch = beui::native::try_with_document(|document| document.touched()).unwrap_or(false);
        state.picking.set(Some(crate::picking::Picking::new(
            serial,
            filter,
            parent,
            anchor.into(),
            touch,
            Box::new(picked),
        )));
    }

    pub fn rename_block(&self, block: Uuid, anchor: impl Into<crate::PickAnchor>) {
        self.pop(crate::popover::Asked::Rename(block), anchor.into());
    }

    pub fn show_app_menu(&self, anchor: impl Into<crate::PickAnchor>) {
        self.pop(crate::popover::Asked::AppMenu, anchor.into());
    }

    fn pop(&self, asked: crate::popover::Asked, anchor: crate::PickAnchor) {
        let state = self.state();
        let serial = state.next_pick.get() + 1;
        state.next_pick.set(serial);
        let touch = beui::native::try_with_document(|document| document.touched()).unwrap_or(false);
        state.popped.set(Some(crate::popover::Popped {
            serial,
            asked,
            anchor,
            touch,
        }));
    }

    pub(crate) fn popped(&self) -> ReadSignal<Option<crate::popover::Popped>> {
        self.state().popped.read_only()
    }

    pub(crate) fn close_popped(&self, serial: u64) {
        let popped = self.state().popped;
        if popped.with_untracked(|popped| popped.is_some_and(|popped| popped.serial == serial)) {
            popped.set(None);
        }
    }

    pub(crate) fn frame_child_menu(&self) -> Option<Memo<Vec<MenuNode>>> {
        self.state()
            .frame_children
            .with(|held| held.last().map(|(_, menu)| *menu))
    }

    pub(crate) fn hold_frame_child_menu(&self, key: u64, menu: Option<Memo<Vec<MenuNode>>>) {
        let Some(state) = self.0.try_value() else {
            return;
        };
        let held = state.frame_children;
        if !held.is_alive() {
            return;
        }
        let present = held.with_untracked(|held| held.iter().any(|(at, _)| *at == key));
        match menu {
            Some(menu) if !present => held.update(|held| held.push((key, menu))),
            None if present => held.update(|held| held.retain(|(at, _)| *at != key)),
            _ => {}
        }
    }

    pub(crate) fn content_window(&self) -> Option<beui::WindowId> {
        let node = self
            .state()
            .content
            .borrow()
            .as_ref()
            .and_then(|(node, _)| node.try_get());
        try_with_document(|document| node.map(|node| document.window_of(node)))
            .flatten()
            .or_else(|| {
                use_context::<crate::popover::PopoverWindow>()?
                    .0
                    .get_untracked()
            })
    }

    pub(crate) fn picker_size(&self) -> Option<Vec2> {
        self.state().picker_size.get()
    }

    pub(crate) fn remember_picker_size(&self, size: Vec2) {
        self.state().picker_size.set(Some(size));
    }

    pub(crate) fn picking(&self) -> ReadSignal<Option<crate::picking::Picking>> {
        self.state().picking.read_only()
    }

    pub(crate) fn finish_pick(&self, serial: u64, answer: BlockPick) {
        let state = self.state();
        let Some(picking) = state
            .picking
            .get_untracked()
            .filter(|picking| picking.serial() == serial)
        else {
            return;
        };
        state.picking.set(None);
        picking.answer(answer);
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
        dock_tab: Option<u64>,
        docks: bool,
        tab: Option<DockTabControl>,
    ) -> ReadSignal<ChildState> {
        let editor_state = self.state();
        let state = create_signal(ChildState::default());
        let placed = editor_state.core.add_child(
            move |now: &ChildState| {
                state.set(now.clone());
                report.call(now.clone());
            },
            move |action| bar.call(action),
            move || focus.call(()),
        );
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
                placed,
                zone: document_zone(),
                dock_tab,
                docks,
                tab,
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
        self.state().core.content_rect()
    }

    pub fn placed(&self) -> ReadSignal<Rect> {
        self.state().shown.placed.read_only()
    }

    pub fn pixels_per_point(&self) -> ReadSignal<f32> {
        self.state().shown.pixels_per_point.read_only()
    }

    pub fn resized(&self) -> ReadSignal<Option<Vec2>> {
        self.state().shown.resized.read_only()
    }

    pub fn report_resize(&self, size: Vec2) {
        self.state().core.report_resize(size);
    }

    pub fn show<P: PresenceKind>(&self, value: Option<&P>) {
        self.state().core.show(value);
    }

    pub fn peers<P>(&self) -> ReadSignal<Vec<(u64, P)>>
    where
        P: PresenceKind + Clone + PartialEq + 'static,
    {
        let peers = create_signal(Vec::new());
        let core = self.core();
        core.host().ask_for_peers(None);
        let revision = self.pushed(Pushed::Peers);
        let seen = Cell::new(0);
        create_effect(move || {
            revision.get();
            if let Some(decoded) = core.new_peers::<P>(&seen) {
                peers.set(decoded);
            }
        });
        peers.read_only()
    }

    pub fn send_to_peers<M: ChannelKind>(&self, to: Option<u64>, message: &M) {
        self.state().core.send_to_peers(to, message);
    }

    pub fn on_peer_message<M>(&self, receive: impl Fn(u64, M) + 'static)
    where
        M: ChannelKind + 'static,
    {
        let core = self.core();
        core.host().ask_for_peers(None);
        let revision = self.pushed(Pushed::PeerMessages);
        let seen = Cell::new(core.host().peer_messages_seen());
        create_effect(move || {
            revision.get();
            let received = core.new_peer_messages::<M>(&seen);
            untrack(|| {
                for (from, message) in received {
                    receive(from, message);
                }
            });
        });
    }

    pub fn presence_visible(&self) -> ReadSignal<bool> {
        self.state().shown.presence_visible.read_only()
    }

    pub fn report_presence_visible(&self, visible: bool) {
        self.state().core.report_presence_visible(visible);
    }

    pub fn revealed(&self) -> ReadSignal<Option<u64>> {
        self.state().shown.revealed.read_only()
    }

    pub fn report_reveal(&self, client_id: u64) {
        self.state().core.report_reveal(client_id);
    }

    pub fn on_replace_child(&self, replace: impl Fn(Uuid, Uuid) -> bool + 'static) {
        self.state().core.on_replace_child(replace);
    }

    pub fn replace_child(&self, old: Uuid, new: Uuid) -> bool {
        self.state().core.replace_child(old, new)
    }

    pub fn drag(&self) -> ReadSignal<Option<Drag>> {
        self.state().shown.drag.read_only()
    }

    pub fn accept_drag(&self, accepted: bool) {
        self.state().core.host().accept_drag(accepted);
    }

    pub fn pan(&self, delta: Vec2) {
        self.state().core.pan(delta);
    }

    pub fn zoom(&self, factor: f32) {
        self.state().core.zoom(factor);
    }

    pub fn zoom_at(&self, factor: f32, anchor: Pos2) {
        self.state().core.zoom_at(factor, anchor);
    }

    pub(crate) fn keep_fitted_area(&self, area: Option<FitArea>) {
        self.state().core.keep_fitted(area);
    }

    pub fn pan_drag(&self) {
        self.state().core.pan_drag();
    }

    pub fn reveal(&self, target: Rect) {
        self.state().core.reveal(target);
    }

    pub fn begin_frame(&self) {
        let state = self.state();
        state.pushed.sync();
        state.core.begin_frame();
    }

    pub fn end_frame(&self, document: &Document, window: beui::WindowId) {
        let state = self.state();
        let zone = document.zone();
        let mut records: Vec<(Vec<usize>, Rc<ChildRecord>)> = self
            .records()
            .into_iter()
            .filter(|record| record.zone == 0 || record.zone == zone)
            .filter(|record| {
                record
                    .slot
                    .node()
                    .is_none_or(|node| document.window_of(node) == window)
            })
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
        let overlays = document.overlay_layers(window);
        let mut overlays = overlays.iter().peekable();
        for (order, record) in records {
            while let Some((_, rect)) = overlays.next_if(|(layer, _)| *layer < order) {
                state.core.occlude(*rect);
            }
            record.placed.place(self.place_child(document, &record));
        }
        for (_, rect) in overlays {
            state.core.occlude(*rect);
        }
        let node = state
            .content
            .borrow()
            .as_ref()
            .filter(|(_, owner)| *owner == 0 || *owner == zone)
            .and_then(|(node, _)| node.try_get())
            .filter(|node| document.window_of(*node) == window);
        let Some(rect) = node.and_then(|node| document.node_rect(node)) else {
            return;
        };
        state.core.report_content(rect);
    }
}

fn document_zone() -> u64 {
    beui::native::try_with_document(|document| document.zone()).unwrap_or(0)
}

impl Editor {
    fn place_child(&self, document: &Document, record: &ChildRecord) -> Option<ChildId> {
        let state = self.state();
        let node = record.slot.node()?;
        let rect = document.node_rect(node)?;
        let placement = record.slot.placement()?;
        let content = record.content.get_untracked()?;
        let unscale = state.core.ratio().recip();
        let shown = document
            .window_rect(document.window_of(node))
            .unwrap_or_else(|| document.viewport_rect());
        let escaped = document.escaped_layer_of(node);
        let to_placed = |rect: Rect| match escaped {
            Some((_, to_host)) => to_host.rect(rect),
            None => rect.scaled(unscale),
        };
        let fills_tab =
            record.own_frame.get_untracked() || matches!(content, SubregionContent::DockTab(_));
        let anchor = match record.tab.filter(|_| fills_tab) {
            None => Some(to_placed(rect)),
            Some(tab) if untrack(|| tab.fullscreen()) => None,
            Some(tab) => match tab.handle() {
                Some(handle) => document.node_rect(handle).map(to_placed),
                None => state.core.host().frame_anchor(),
            },
        };
        let placed = || {
            state.core.host().place_child(
                content.wire(),
                to_placed(rect),
                to_placed(placement.clip),
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
                record.dock_tab,
                record.docks,
                anchor,
            )
        };
        Some(match escaped {
            Some((id, _)) => state.core.host().in_view(
                block_plugin_api::View::anchored(beui_platform_plugin_api::anchored_view(id)),
                placed,
            ),
            None => placed(),
        })
    }
}

#[derive(Clone)]
pub struct Creation(Rc<CreationSignals>);

struct CreationSignals {
    core: CreationCore,
    children: OwnedEditor,
    pushed: Mirror,
    seen: Cell<Option<u64>>,
}

impl Creation {
    pub const MAIN: &str = CreationCore::MAIN;

    pub fn new(host: EditorHost) -> Self {
        Self::for_template(host, Self::MAIN)
    }

    pub fn for_template(host: EditorHost, template: impl Into<String>) -> Self {
        defer_graph_changes(&host);
        Self(Rc::new(CreationSignals {
            pushed: Mirror::new(&host),
            children: OwnedEditor::new(host.clone(), Uuid::nil()),
            core: CreationCore::new(host, template),
            seen: Cell::new(None),
        }))
    }

    pub fn blocks(&self) -> Blocks {
        Blocks::of(self.0.core.host())
    }

    pub fn then<T: 'static>(
        &self,
        asked: impl std::future::Future<Output = Option<T>> + 'static,
        answered: impl FnOnce(T) + 'static,
    ) -> Task {
        then(self.0.core.host(), asked, answered)
    }

    pub fn begin_frame(&self) {
        let changes = Some(self.0.core.host().changes());
        if self.0.seen.replace(changes) != changes {
            self.0.pushed.sync();
        }
        self.0.children.begin_frame();
    }

    pub fn pick_block(
        &self,
        filter: BlockFilter,
        anchor: impl Into<crate::PickAnchor>,
        picked: impl FnOnce(Result<PickedBlock, block_editor_plugin::Failure>) + 'static,
    ) {
        self.0.children.pick_block(filter, anchor, picked);
    }

    pub(crate) fn children(&self) -> Editor {
        *self.0.children
    }
}

impl std::ops::Deref for Creation {
    type Target = CreationCore;

    fn deref(&self) -> &CreationCore {
        &self.0.core
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
        self.state().core.host().account_id()
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

mod settings;
mod tags;

pub use settings::{KindsAbove, OrganizationLayers, SetKind};

#[cfg(test)]
mod tests;
