use std::cell::{Cell, RefCell};
use std::rc::Rc;

use be_block::presence::PresenceKind;
use beui::reactive::{
    Callback, CanvasView, EmbedSlot, Memo, NodeRef, Prop, ReadSignal, WriteSignal, create_memo,
    create_signal, on_cleanup,
};
use beui::{Document, Pos2, Rect, Vec2};
use block_plugin_api::{
    ChildId, ChildLayer, ChildMode, EditorCapabilities, InteractionMode, ResizeMode, ViewChange,
};
use block_ui::BlockCatalog;
use uuid::Uuid;

use crate::{
    BlockFilter, BlockList, BlockParent, BlockPicker, BlockQuery, Blocks, ContentProjection,
    EditorHost, PickedBlock,
};

type Regenerate = Rc<dyn Fn(&[u8])>;
type PollArtifact = Rc<dyn Fn() -> Option<Result<(), String>>>;
type ReplaceChild = Rc<dyn Fn(Uuid, Uuid) -> bool>;

pub struct Artifacts(Rc<ArtifactState>);

struct ArtifactState {
    host: EditorHost,
    artifact: crate::Artifact,
    regenerate: RefCell<Option<Regenerate>>,
    poll: RefCell<Option<PollArtifact>>,
    settings: ReadSignal<Vec<u8>>,
    set_settings: WriteSignal<Vec<u8>>,
    edit: RefCell<Option<Vec<u8>>>,
}

impl Clone for Artifacts {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl Artifacts {
    pub fn new(host: EditorHost, artifact: crate::Artifact) -> Self {
        let (settings, set_settings) = create_signal(Vec::new());
        Self(Rc::new(ArtifactState {
            host,
            artifact,
            regenerate: RefCell::new(None),
            poll: RefCell::new(None),
            settings,
            set_settings,
            edit: RefCell::new(None),
        }))
    }

    pub fn settings(&self) -> ReadSignal<Vec<u8>> {
        self.0.settings.clone()
    }

    pub fn edit_settings(&self, data: Vec<u8>) {
        self.0.set_settings.set(data.clone());
        *self.0.edit.borrow_mut() = Some(data);
    }

    pub fn receive_settings(&self, data: &[u8]) {
        if self.0.settings.get_untracked() != data {
            self.0.set_settings.set(data.to_vec());
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
}

impl ChildTarget {
    pub fn new(id: Uuid, block_type: Uuid) -> Self {
        Self { id, block_type }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChildState {
    pub placed: bool,
    pub available: bool,
    pub hovered: bool,
    pub active: bool,
    pub intrinsic_size: Option<Vec2>,
    pub aspect_ratio: Option<f32>,
    pub interaction: Option<InteractionMode>,
    pub capabilities: EditorCapabilities,
    pub resize: ResizeMode,
    pub error: Option<String>,
}

impl ChildState {
    fn of(host: &EditorHost, child: Option<ChildId>) -> Self {
        let Some(status) = child.and_then(|child| host.child_status(child)) else {
            return Self {
                placed: child.is_some(),
                ..Self::default()
            };
        };
        Self {
            placed: true,
            available: status.available,
            hovered: status.hovered,
            active: status.active,
            intrinsic_size: status
                .intrinsic
                .map(|size| Vec2::new(size.width, size.height)),
            aspect_ratio: status.aspect_ratio,
            interaction: Some(status.interaction),
            capabilities: status.capabilities,
            resize: status.resize,
            error: status.error,
        }
    }
}

struct ChildRecord {
    slot: EmbedSlot,
    block: Prop<Option<ChildTarget>>,
    mode: Prop<ChildMode>,
    layer: Prop<ChildLayer>,
    own_frame: Prop<bool>,
    top_bar: Prop<bool>,
    rotation: Prop<f32>,
    opacity: Prop<f32>,
    intrinsic: Prop<Option<Vec2>>,
    state: WriteSignal<ChildState>,
    read: ReadSignal<ChildState>,
    report: Callback<ChildState>,
    view_change: Callback<ViewChange>,
    child: Cell<Option<ChildId>>,
}

struct PendingPick {
    picker: BlockPicker,
    picked: Rc<dyn Fn(Result<PickedBlock, String>)>,
}

type Work = RefCell<Vec<(u64, Rc<dyn Fn()>)>>;

fn run(work: &Work) {
    let callbacks = work.borrow().clone();
    for (_, callback) in callbacks {
        callback();
    }
}

fn register(work: &Rc<Work>, next: &Cell<u64>, callback: impl Fn() + 'static) {
    let id = next.get();
    next.set(id + 1);
    work.borrow_mut().push((id, Rc::new(callback)));
    let work = Rc::downgrade(work);
    on_cleanup(move || {
        if let Some(work) = work.upgrade() {
            work.borrow_mut().retain(|(existing, _)| *existing != id);
        }
    });
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

#[derive(Clone)]
pub struct Editor(Rc<EditorState>);

struct EditorState {
    host: EditorHost,
    block: Uuid,
    roots: RefCell<Option<BlockList>>,
    canvas: ReadSignal<Option<CanvasView>>,
    world: ReadSignal<Option<Vec2>>,
    scale: ReadSignal<f32>,
    chrome: ReadSignal<bool>,
    editable: ReadSignal<bool>,
    set_canvas: WriteSignal<Option<CanvasView>>,
    set_world: WriteSignal<Option<Vec2>>,
    set_scale: WriteSignal<f32>,
    set_chrome: WriteSignal<bool>,
    set_editable: WriteSignal<bool>,
    presenting: ReadSignal<bool>,
    set_presenting: WriteSignal<bool>,
    drag: ReadSignal<Option<Drag>>,
    set_drag: WriteSignal<Option<Drag>>,
    pixels_per_point: ReadSignal<f32>,
    set_pixels_per_point: WriteSignal<f32>,
    resized: ReadSignal<Option<Vec2>>,
    set_resized: WriteSignal<Option<Vec2>>,
    pending_resize: Cell<Option<Vec2>>,
    presence_visible: ReadSignal<bool>,
    set_presence_visible: WriteSignal<bool>,
    pending_presence: Cell<Option<bool>>,
    revealed: ReadSignal<Option<u64>>,
    set_revealed: WriteSignal<Option<u64>>,
    pending_reveal: Cell<Option<u64>>,
    replace: RefCell<Option<ReplaceChild>>,
    content: RefCell<Option<NodeRef>>,
    projections: RefCell<std::collections::HashMap<Option<Uuid>, Rc<dyn std::any::Any>>>,
    content_rect: Cell<Rect>,
    intrinsic: Cell<Option<Vec2>>,
    children: RefCell<Vec<(u64, Rc<ChildRecord>)>>,
    next_child: Cell<u64>,
    pick: RefCell<Option<PendingPick>>,
    each_frame: Rc<Work>,
    next_work: Cell<u64>,
}

impl Editor {
    pub fn new(host: EditorHost, block: Uuid) -> Self {
        let (canvas, set_canvas) = create_signal(None::<CanvasView>);
        let (world, set_world) = create_signal(None::<Vec2>);
        let (scale, set_scale) = create_signal(1.0_f32);
        let (chrome, set_chrome) = create_signal(true);
        let (editable, set_editable) = create_signal(host.editable());
        let (presenting, set_presenting) = create_signal(false);
        let (drag, set_drag) = create_signal(None::<Drag>);
        let (pixels_per_point, set_pixels_per_point) = create_signal(1.0_f32);
        let (resized, set_resized) = create_signal(None::<Vec2>);
        let (presence_visible, set_presence_visible) = create_signal(false);
        let (revealed, set_revealed) = create_signal(None::<u64>);
        Self(Rc::new(EditorState {
            host,
            block,
            roots: RefCell::new(None),
            canvas,
            world,
            scale,
            chrome,
            editable,
            set_canvas,
            set_world,
            set_scale,
            set_chrome,
            set_editable,
            presenting,
            set_presenting,
            drag,
            set_drag,
            pixels_per_point,
            set_pixels_per_point,
            resized,
            set_resized,
            pending_resize: Cell::new(None),
            presence_visible,
            set_presence_visible,
            pending_presence: Cell::new(None),
            revealed,
            set_revealed,
            pending_reveal: Cell::new(None),
            replace: RefCell::new(None),
            content: RefCell::new(None),
            projections: RefCell::new(std::collections::HashMap::new()),
            content_rect: Cell::new(Rect::ZERO),
            intrinsic: Cell::new(None),
            children: RefCell::new(Vec::new()),
            next_child: Cell::new(0),
            pick: RefCell::new(None),
            each_frame: Rc::new(RefCell::new(Vec::new())),
            next_work: Cell::new(0),
        }))
    }

    pub fn host(&self) -> &EditorHost {
        &self.0.host
    }

    pub fn blocks(&self) -> Blocks {
        self.0.host.blocks()
    }

    pub fn block_id(&self) -> Uuid {
        self.0.block
    }

    pub fn watch_blocks(&self, query: BlockQuery) -> Memo<Option<Vec<crate::BlockInfo>>> {
        let list = self.blocks().watch(query);
        let (listed, set_listed) = create_signal(None::<Vec<crate::BlockInfo>>);
        let seen = Cell::new(None::<u64>);
        let last = RefCell::new(None::<Vec<crate::BlockInfo>>);
        let blocks = self.blocks();
        self.each_frame(move || {
            let revision = blocks.revision();
            if seen.replace(Some(revision)) == Some(revision) {
                return;
            }
            let current = list.is_loaded().then(|| list.read());
            if *last.borrow() == current {
                return;
            }
            last.replace(current.clone());
            set_listed.set(current);
        });
        create_memo(move || listed.get())
    }

    pub fn editable(&self) -> ReadSignal<bool> {
        self.0.editable.clone()
    }

    pub fn read_only(&self) -> Memo<bool> {
        let editable = self.0.editable.clone();
        create_memo(move || !editable.get())
    }

    pub fn block_content<C>(&self) -> Rc<ContentProjection<C>>
    where
        C: be_block::LiveEdit + Clone + Default,
    {
        self.projection(None)
    }

    pub fn content_of<C>(&self, block: Uuid) -> Rc<ContentProjection<C>>
    where
        C: be_block::LiveEdit + Clone + Default,
    {
        if block == self.0.block {
            return self.block_content();
        }
        self.0.host.watch_content(block, C::CONTENT_TYPE);
        self.projection(Some(block))
    }

    pub fn related_content<C>(&self, block: Memo<Option<Uuid>>) -> Rc<crate::RelatedContent<C>>
    where
        C: be_block::LiveEdit + Clone + Default,
    {
        Rc::new(crate::RelatedContent::new(self.clone(), block))
    }

    pub fn seed_content<C: be_block::BlockContent>(&self, block: Uuid, content: &C) {
        self.0.host.seed_content(block, content);
    }

    pub fn replace_content<C: be_block::BlockContent>(&self, block: Uuid, content: &C) {
        self.0.host.replace_content(block, content);
    }

    pub fn create_child<C: be_block::BlockContent>(&self, content: &C) -> Uuid {
        self.blocks()
            .create(content, BlockParent::Block(self.0.block))
    }

    fn projection<C>(&self, block: Option<Uuid>) -> Rc<ContentProjection<C>>
    where
        C: be_block::LiveEdit + Clone + Default,
    {
        let cached = self
            .0
            .projections
            .borrow()
            .get(&block)
            .cloned()
            .and_then(|held| held.downcast::<ContentProjection<C>>().ok());
        if let Some(source) = cached {
            return source;
        }
        let source = Rc::new(ContentProjection::<C>::new(self.0.host.clone(), block));
        let pumped = Rc::clone(&source);
        self.each_frame(move || pumped.pump());
        self.0
            .projections
            .borrow_mut()
            .insert(block, Rc::clone(&source) as Rc<dyn std::any::Any>);
        source
    }

    pub fn each_frame(&self, work: impl Fn() + 'static) {
        register(&self.0.each_frame, &self.0.next_work, work);
    }

    pub fn canvas(&self) -> ReadSignal<Option<CanvasView>> {
        self.0.canvas.clone()
    }

    pub fn world(&self) -> ReadSignal<Option<Vec2>> {
        self.0.world.clone()
    }

    pub fn scale(&self) -> ReadSignal<f32> {
        self.0.scale.clone()
    }

    pub fn chrome_shown(&self) -> ReadSignal<bool> {
        self.0.chrome.clone()
    }

    pub fn presenting(&self) -> ReadSignal<bool> {
        self.0.presenting.clone()
    }

    pub fn present(&self, presenting: bool) {
        self.0.host.present(presenting);
    }

    pub fn block_types(&self) -> Rc<BlockCatalog> {
        self.0.host.block_types()
    }

    pub fn set_intrinsic_size(&self, size: Option<Vec2>) {
        self.0.intrinsic.set(size);
    }

    pub fn intrinsic_size(&self) -> Option<Vec2> {
        self.0.intrinsic.get()
    }

    pub fn pick_block(
        &self,
        filter: BlockFilter,
        picked: impl Fn(Result<PickedBlock, String>) + 'static,
    ) {
        let mut picker = BlockPicker::default();
        picker.open(&self.0.host, filter);
        *self.0.pick.borrow_mut() = Some(PendingPick {
            picker,
            picked: Rc::new(picked),
        });
    }

    pub(crate) fn register_child(
        &self,
        slot: EmbedSlot,
        block: Prop<Option<ChildTarget>>,
        mode: Prop<ChildMode>,
        layer: Prop<ChildLayer>,
        own_frame: Prop<bool>,
        top_bar: Prop<bool>,
        rotation: Prop<f32>,
        opacity: Prop<f32>,
        intrinsic: Prop<Option<Vec2>>,
        report: Callback<ChildState>,
        view_change: Callback<ViewChange>,
    ) -> ReadSignal<ChildState> {
        let (state, set_state) = create_signal(ChildState::default());
        let key = self.0.next_child.get();
        self.0.next_child.set(key + 1);
        self.0.children.borrow_mut().push((
            key,
            Rc::new(ChildRecord {
                slot,
                block,
                mode,
                layer,
                own_frame,
                top_bar,
                rotation,
                opacity,
                intrinsic,
                state: set_state,
                read: state.clone(),
                report,
                view_change,
                child: Cell::new(None),
            }),
        ));
        let editor = Rc::clone(&self.0);
        on_cleanup(move || {
            editor
                .children
                .borrow_mut()
                .retain(|(other, _)| *other != key);
        });
        state
    }

    fn records(&self) -> Vec<Rc<ChildRecord>> {
        self.0
            .children
            .borrow()
            .iter()
            .map(|(_, record)| Rc::clone(record))
            .collect()
    }

    fn poll_pick(&self) {
        let picked = {
            let mut pending = self.0.pick.borrow_mut();
            let Some(pick) = pending.as_mut() else {
                return;
            };
            let Some(picked) = pick.picker.poll(&self.0.host) else {
                return;
            };
            pending.take().map(|pick| (pick.picked, picked))
        };
        if let Some((callback, picked)) = picked {
            callback(picked);
        }
    }

    pub fn content(&self, node: &NodeRef) {
        *self.0.content.borrow_mut() = Some(node.clone());
    }

    pub fn content_rect(&self) -> Rect {
        self.0.content_rect.get()
    }

    pub fn pixels_per_point(&self) -> ReadSignal<f32> {
        self.0.pixels_per_point.clone()
    }

    pub fn resized(&self) -> ReadSignal<Option<Vec2>> {
        self.0.resized.clone()
    }

    pub fn report_resize(&self, size: Vec2) {
        self.0.pending_resize.set(Some(size));
    }

    pub fn show<P: PresenceKind>(&self, value: Option<&P>) {
        let value = value.and_then(|value| serde_json::to_vec(value).ok());
        self.0.host.show_presence(None, P::ID, value);
    }

    pub fn peers<P>(&self) -> ReadSignal<Vec<(u64, P)>>
    where
        P: PresenceKind + Clone + PartialEq + 'static,
    {
        let (peers, set_peers) = create_signal(Vec::new());
        let host = self.0.host.clone();
        let seen = Cell::new(0);
        let last: RefCell<Vec<(u64, P)>> = RefCell::new(Vec::new());
        self.each_frame(move || {
            let Some((revision, held)) = host.peers_since(None, seen.get()) else {
                return;
            };
            seen.set(revision);
            let decoded: Vec<(u64, P)> = held
                .iter()
                .filter(|peer| peer.kind == P::ID)
                .filter_map(|peer| {
                    serde_json::from_slice(&peer.value)
                        .ok()
                        .map(|value| (peer.client, value))
                })
                .collect();
            if *last.borrow() != decoded {
                last.replace(decoded.clone());
                set_peers.set(decoded);
            }
        });
        peers
    }

    pub fn presence_visible(&self) -> ReadSignal<bool> {
        self.0.presence_visible.clone()
    }

    pub fn report_presence_visible(&self, visible: bool) {
        self.0.pending_presence.set(Some(visible));
    }

    pub fn revealed(&self) -> ReadSignal<Option<u64>> {
        self.0.revealed.clone()
    }

    pub fn report_reveal(&self, client_id: u64) {
        self.0.pending_reveal.set(Some(client_id));
    }

    pub fn on_replace_child(&self, replace: impl Fn(Uuid, Uuid) -> bool + 'static) {
        *self.0.replace.borrow_mut() = Some(Rc::new(replace));
    }

    pub fn replace_child(&self, old: Uuid, new: Uuid) -> bool {
        let replace = self.0.replace.borrow().clone();
        replace.is_some_and(|replace| replace(old, new))
    }

    pub fn drag(&self) -> ReadSignal<Option<Drag>> {
        self.0.drag.clone()
    }

    pub fn accept_drag(&self, accepted: bool) {
        self.0.host.accept_drag(accepted);
    }

    pub fn place_web_view(&self, rect: Option<Rect>) {
        self.0.host.place_beui_web_view(rect);
    }

    pub fn pan(&self, delta: Vec2) {
        self.0.host.beui_view().pan(delta);
    }

    pub fn zoom(&self, factor: f32) {
        self.0.host.beui_view().zoom(factor, None);
    }

    pub fn zoom_at(&self, factor: f32, anchor: Pos2) {
        self.0.host.beui_view().zoom(factor, Some(anchor));
    }

    pub fn resume_auto_fit(&self) {
        self.0.host.resume_auto_fit_view();
    }

    pub fn fit(&self) {
        self.0.host.beui_view().fit();
    }

    pub fn reveal(&self, target: Rect) {
        let view = self.0.host.beui_view();
        let Some(placement) = view.canvas() else {
            return;
        };
        let shown = placement.rect_to_screen(target).center();
        view.pan(self.0.content_rect.get().center() - shown);
    }

    pub fn begin_frame(&self) {
        let view = self.0.host.beui_view();
        let scale = view.scale().max(f32::EPSILON);
        self.0.set_canvas.set(view.canvas());
        self.0.set_world.set(
            view.rect()
                .map(|rect| Vec2::new(rect.width() / scale, rect.height() / scale)),
        );
        self.0.set_scale.set(view.scale());
        self.0.set_chrome.set(self.0.host.chrome_shown());
        self.0.set_editable.set(self.0.host.editable());
        self.0.set_presenting.set(self.0.host.presenting());
        self.0.set_drag.set(self.0.host.beui_drag());
        self.0
            .set_pixels_per_point
            .set(self.0.host.beui_pixels_per_point());
        self.0.set_resized.set(self.0.pending_resize.take());
        if let Some(visible) = self.0.pending_presence.take() {
            self.0.set_presence_visible.set(visible);
        }
        self.0.set_revealed.set(self.0.pending_reveal.take());
        for record in self.records() {
            if let Some(child) = record.child.get() {
                for change in self.0.host.take_child_view_changes(child) {
                    record.view_change.call(change);
                }
            }
            let state = ChildState::of(&self.0.host, record.child.get());
            if record.read.get_untracked() == state {
                continue;
            }
            record.state.set(state.clone());
            record.report.call(state);
        }
        self.poll_pick();
        run(&self.0.each_frame);
    }

    pub fn end_frame(&self, document: &Document) {
        for record in self.records() {
            record.child.set(self.place_child(document, &record));
        }
        for rect in document.overlay_rects() {
            self.0.host.occlude_beui(rect);
        }
        let node = self.0.content.borrow().as_ref().and_then(NodeRef::try_get);
        let Some(rect) = node.and_then(|node| document.node_rect(node)) else {
            return;
        };
        self.0.content_rect.set(rect);
        self.0.host.beui_view().set_content(rect);
    }
}

impl Editor {
    fn place_child(&self, document: &Document, record: &ChildRecord) -> Option<ChildId> {
        let node = record.slot.node()?;
        let rect = document.node_rect(node)?;
        let placement = record.slot.placement()?;
        let target = record.block.peek()?;
        Some(self.0.host.place_beui_child(
            target.id,
            target.block_type,
            rect,
            placement.clip,
            record.mode.peek(),
            record.layer.peek(),
            record.own_frame.peek(),
            record.top_bar.peek(),
            record.rotation.peek(),
            record.opacity.peek(),
            record.intrinsic.peek(),
        ))
    }
}

type Maker = Rc<dyn Fn() -> Result<Uuid, String>>;

#[derive(Clone)]
pub struct Creation(Rc<CreationState>);

struct CreationState {
    host: EditorHost,
    maker: RefCell<Option<Maker>>,
    each_frame: Rc<Work>,
    next_work: Cell<u64>,
}

impl Creation {
    pub fn new(host: EditorHost) -> Self {
        Self(Rc::new(CreationState {
            host,
            maker: RefCell::new(None),
            each_frame: Rc::new(RefCell::new(Vec::new())),
            next_work: Cell::new(0),
        }))
    }

    pub fn host(&self) -> &EditorHost {
        &self.0.host
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

    pub fn set_ready(&self, ready: bool) {
        self.0.host.set_creation_ready(ready);
    }

    pub fn on_create(&self, make: impl Fn() -> Result<Uuid, String> + 'static) {
        *self.0.maker.borrow_mut() = Some(Rc::new(make));
    }

    pub fn each_frame(&self, work: impl Fn() + 'static) {
        register(&self.0.each_frame, &self.0.next_work, work);
    }

    pub fn create_block(&self) -> Result<Uuid, String> {
        let maker = self.0.maker.borrow().clone();
        match maker {
            Some(make) => make(),
            None => Err("this editor does not create blocks".to_owned()),
        }
    }

    pub fn begin_frame(&self) {
        run(&self.0.each_frame);
    }
}

impl crate::root_settings::SettingsGraph for Editor {
    fn roots(&self) -> Option<Vec<(Uuid, Uuid)>> {
        let mut roots = self.0.roots.borrow_mut();
        let list = roots.get_or_insert_with(|| self.blocks().watch(BlockQuery::Roots));
        list.is_loaded().then(|| {
            list.read()
                .into_iter()
                .map(|info| (info.id, info.block_type))
                .collect()
        })
    }

    fn settings(&self, block: Uuid) -> Option<be_block::Settings> {
        self.content_of::<be_block::SettingsContent>(block)
            .read(|settings| settings.root().clone())
    }

    fn create(&self, block_type: Uuid, content: Vec<u8>, parent: BlockParent) -> Uuid {
        self.blocks()
            .create_with(block_type, Some(content), parent, None, None)
    }

    fn edit_settings(&self, block: Uuid, edit: be_block::Edit) {
        self.content_of::<be_block::SettingsContent>(block)
            .operate(edit);
    }
}
