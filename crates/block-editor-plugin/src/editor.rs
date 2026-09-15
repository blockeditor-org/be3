use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use beui::reactive::{CanvasView, KeyedStore, NodeRef, ReadSignal, WriteSignal, create_signal};
use beui::{Document, Rect, Vec2};
use block::Block;
use block_client::{BlockClient, BlockHandle};
use block_reactive::BlockSource;
use std::hash::Hash;
use uuid::Uuid;

use crate::EditorHost;

type Work = RefCell<Vec<Rc<dyn Fn()>>>;

fn run(work: &Work) {
    let callbacks = work.borrow().clone();
    for callback in callbacks {
        callback();
    }
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
    set_canvas: WriteSignal<Option<CanvasView>>,
    set_scale: WriteSignal<f32>,
    set_chrome: WriteSignal<bool>,
    content: RefCell<Option<NodeRef>>,
    content_rect: Cell<Rect>,
    each_frame: Work,
}

impl Editor {
    pub fn new(host: EditorHost, client: Arc<BlockClient>, block: Uuid) -> Self {
        let (canvas, set_canvas) = create_signal(None::<CanvasView>);
        let (scale, set_scale) = create_signal(1.0_f32);
        let (chrome, set_chrome) = create_signal(true);
        Self(Rc::new(EditorState {
            host,
            client,
            block,
            canvas,
            scale,
            chrome,
            set_canvas,
            set_scale,
            set_chrome,
            content: RefCell::new(None),
            content_rect: Cell::new(Rect::ZERO),
            each_frame: RefCell::new(Vec::new()),
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

    pub fn editable(&self) -> bool {
        self.0.host.editable()
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
        self.0.each_frame.borrow_mut().push(Rc::new(work));
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
        run(&self.0.each_frame);
    }

    pub fn end_frame(&self, document: &Document) {
        let node = self.0.content.borrow().as_ref().and_then(NodeRef::try_get);
        let Some(rect) = node.and_then(|node| document.node_rect(node)) else {
            return;
        };
        self.0.content_rect.set(rect);
        self.0.host.beui_view().set_content(rect);
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
    each_frame: Work,
}

impl Creation {
    pub fn new(host: EditorHost, client: Arc<BlockClient>) -> Self {
        Self(Rc::new(CreationState {
            host,
            client,
            maker: RefCell::new(None),
            each_frame: RefCell::new(Vec::new()),
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
        self.0.each_frame.borrow_mut().push(Rc::new(work));
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
