use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use beui::reactive::{
    Callback, CanvasView, EmbedSlot, KeyedStore, Memo, NodeRef, Prop, ReadSignal, WriteSignal,
    create_memo, create_signal, on_cleanup,
};
use beui::{Document, Rect, Vec2};
use block::Block;
use block_client::{BlockClient, BlockHandle};
use block_plugin_api::{ChildId, ChildLayer, ChildMode};
use block_reactive::BlockSource;
use block_ui::BlockCatalog;
use std::hash::Hash;
use uuid::Uuid;

use crate::{BlockFilter, BlockPicker, EditorHost, PickedBlock};

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
            intrinsic_size: (status.intrinsic_width > 0.0 && status.intrinsic_height > 0.0)
                .then(|| Vec2::new(status.intrinsic_width, status.intrinsic_height)),
            aspect_ratio: (status.aspect_ratio > 0.0).then_some(status.aspect_ratio),
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
    state: WriteSignal<ChildState>,
    read: ReadSignal<ChildState>,
    report: Callback<ChildState>,
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

#[derive(Clone)]
pub struct Editor(Rc<EditorState>);

struct EditorState {
    host: EditorHost,
    client: Arc<BlockClient>,
    block: Uuid,
    canvas: ReadSignal<Option<CanvasView>>,
    scale: ReadSignal<f32>,
    chrome: ReadSignal<bool>,
    editable: ReadSignal<bool>,
    set_canvas: WriteSignal<Option<CanvasView>>,
    set_scale: WriteSignal<f32>,
    set_chrome: WriteSignal<bool>,
    set_editable: WriteSignal<bool>,
    presenting: ReadSignal<bool>,
    set_presenting: WriteSignal<bool>,
    content: RefCell<Option<NodeRef>>,
    content_rect: Cell<Rect>,
    intrinsic: Cell<Option<Vec2>>,
    children: RefCell<Vec<(u64, Rc<ChildRecord>)>>,
    next_child: Cell<u64>,
    pick: RefCell<Option<PendingPick>>,
    each_frame: Rc<Work>,
    next_work: Cell<u64>,
}

impl Editor {
    pub fn new(host: EditorHost, client: Arc<BlockClient>, block: Uuid) -> Self {
        let (canvas, set_canvas) = create_signal(None::<CanvasView>);
        let (scale, set_scale) = create_signal(1.0_f32);
        let (chrome, set_chrome) = create_signal(true);
        let (editable, set_editable) = create_signal(host.editable());
        let (presenting, set_presenting) = create_signal(false);
        Self(Rc::new(EditorState {
            host,
            client,
            block,
            canvas,
            scale,
            chrome,
            editable,
            set_canvas,
            set_scale,
            set_chrome,
            set_editable,
            presenting,
            set_presenting,
            content: RefCell::new(None),
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

    pub fn client(&self) -> &Arc<BlockClient> {
        &self.0.client
    }

    pub fn block_id(&self) -> Uuid {
        self.0.block
    }

    pub fn editable(&self) -> ReadSignal<bool> {
        self.0.editable.clone()
    }

    pub fn read_only(&self) -> Memo<bool> {
        let editable = self.0.editable.clone();
        create_memo(move || !editable.get())
    }

    pub fn block<B: Block>(&self) -> Rc<BlockProjection<B>> {
        let waker = self.0.host.waker();
        let source = BlockSource::new(self.0.client.get_block(self.0.block), move || waker.wake());
        let pumped = source.clone();
        self.each_frame(move || pumped.pump());
        Rc::new(BlockProjection {
            source,
            host: self.0.host.clone(),
        })
    }

    pub fn each_frame(&self, work: impl Fn() + 'static) {
        register(&self.0.each_frame, &self.0.next_work, work);
    }

    pub fn canvas(&self) -> ReadSignal<Option<CanvasView>> {
        self.0.canvas.clone()
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
        report: Callback<ChildState>,
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
                state: set_state,
                read: state.clone(),
                report,
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

    pub fn pan(&self, delta: Vec2) {
        self.0.host.beui_view().pan(delta);
    }

    pub fn zoom(&self, factor: f32) {
        self.0.host.beui_view().zoom(factor, None);
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
        self.0.set_canvas.set(view.canvas());
        self.0.set_scale.set(view.scale());
        self.0.set_chrome.set(self.0.host.chrome_shown());
        self.0.set_editable.set(self.0.host.editable());
        self.0.set_presenting.set(self.0.host.presenting());
        for record in self.records() {
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
        ))
    }
}

pub struct BlockProjection<B: Block> {
    source: Rc<BlockSource<B>>,
    host: EditorHost,
}

impl<B: Block> BlockProjection<B> {
    pub fn handle(&self) -> &BlockHandle<B> {
        self.source.handle()
    }

    pub fn id(&self) -> Uuid {
        self.source.id()
    }

    pub fn project<T>(&self, project: impl Fn(&B) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + Default + PartialEq + 'static,
    {
        self.source.project(project)
    }

    pub fn project_or<T>(&self, initial: T, project: impl Fn(&B) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + PartialEq + 'static,
    {
        self.source.project_or(initial, project)
    }

    pub fn project_keyed<K, V>(
        &self,
        project: impl Fn(&B, &KeyedStore<K, V>) + 'static,
    ) -> KeyedStore<K, V>
    where
        K: Clone + Eq + Hash + 'static,
        V: Clone + PartialEq + 'static,
    {
        self.source.project_keyed(project)
    }

    pub fn operate(&self, operation: B::Operation) {
        if self.host.editable() {
            self.source.operate(operation);
        }
    }

    pub fn operate_grouped(&self, operations: impl IntoIterator<Item = B::Operation>) {
        if self.host.editable() {
            self.source.operate_grouped(operations);
        }
    }
}

type Maker = Rc<dyn Fn() -> Result<Uuid, String>>;

#[derive(Clone)]
pub struct Creation(Rc<CreationState>);

struct CreationState {
    host: EditorHost,
    client: Arc<BlockClient>,
    maker: RefCell<Option<Maker>>,
    each_frame: Rc<Work>,
    next_work: Cell<u64>,
}

impl Creation {
    pub fn new(host: EditorHost, client: Arc<BlockClient>) -> Self {
        Self(Rc::new(CreationState {
            host,
            client,
            maker: RefCell::new(None),
            each_frame: Rc::new(RefCell::new(Vec::new())),
            next_work: Cell::new(0),
        }))
    }

    pub fn host(&self) -> &EditorHost {
        &self.0.host
    }

    pub fn client(&self) -> &Arc<BlockClient> {
        &self.0.client
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
