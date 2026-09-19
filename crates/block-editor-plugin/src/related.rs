use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use beui::reactive::{Memo, ReadSignal, batch, create_memo, create_signal};
use block::Block;
use block_client::references::ReferenceResolutionCache;
use block_client::{BlockClient, BlockHandle, block_ref::BlockRef};
use block_reactive::BlockWatch;
use uuid::Uuid;

use crate::{Editor, EditorHost};

type Projection<B> = Rc<dyn Fn(Option<&B>)>;

struct Watched<B: Block> {
    handle: BlockHandle<B>,
    watch: BlockWatch<B>,
}

pub struct RelatedBlock<B: Block> {
    host: EditorHost,
    client: Arc<BlockClient>,
    watched: RefCell<Option<Watched<B>>>,
    projections: RefCell<Vec<Projection<B>>>,
    pending: Cell<bool>,
}

impl<B: Block> RelatedBlock<B> {
    fn new(host: EditorHost, client: Arc<BlockClient>) -> Self {
        Self {
            host,
            client,
            watched: RefCell::new(None),
            projections: RefCell::new(Vec::new()),
            pending: Cell::new(false),
        }
    }

    pub fn id(&self) -> Option<Uuid> {
        self.watched
            .borrow()
            .as_ref()
            .map(|watched| watched.handle.id())
    }

    pub fn handle(&self) -> Option<BlockHandle<B>> {
        self.watched
            .borrow()
            .as_ref()
            .map(|watched| watched.handle.clone())
    }

    pub fn project<T>(&self, project: impl Fn(&B) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + Default + PartialEq + 'static,
    {
        self.project_or(T::default(), project)
    }

    pub fn project_or<T>(&self, initial: T, project: impl Fn(&B) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + PartialEq + 'static,
    {
        let seed = self.with_block(|block| match block {
            Some(block) => project(block),
            None => initial.clone(),
        });
        let (read, write) = create_signal(seed);
        self.projections.borrow_mut().push(Rc::new(move |block| {
            write.set(match block {
                Some(block) => project(block),
                None => initial.clone(),
            });
        }));
        read
    }

    pub fn operate(&self, operation: B::Operation) {
        if !self.host.editable() {
            return;
        }
        let Some(handle) = self.handle() else {
            return;
        };
        handle.operate(operation);
        self.pending.set(true);
    }

    pub fn operate_grouped(&self, operations: impl IntoIterator<Item = B::Operation>) {
        if !self.host.editable() {
            return;
        }
        let Some(handle) = self.handle() else {
            return;
        };
        handle.operate_grouped(operations);
        self.pending.set(true);
    }

    fn with_block<R>(&self, work: impl FnOnce(Option<&B>) -> R) -> R {
        let handle = self.handle();
        let block = handle.as_ref().and_then(BlockHandle::read);
        work(block.as_deref())
    }

    fn watch(&self, id: Option<Uuid>) {
        let current = self.id();
        if current == id {
            return;
        }
        *self.watched.borrow_mut() = id.map(|id| {
            let handle = self.client.get_block::<B>(id);
            let waker = self.host.waker();
            let watch = BlockWatch::new(&handle, move || waker.wake());
            Watched { handle, watch }
        });
        self.pending.set(true);
    }

    fn pump(&self, id: Option<Uuid>) {
        self.watch(id);
        let changed = match self.watched.borrow_mut().as_mut() {
            Some(watched) => watched.watch.take(),
            None => false,
        };
        if !changed && !self.pending.get() {
            return;
        }
        let projections = self.projections.borrow().clone();
        let seen = self.with_block(|block| {
            batch(|| {
                for projection in projections {
                    projection(block);
                }
            });
            block.is_some()
        });
        self.pending.set(id.is_some() && !seen);
    }
}

impl Editor {
    pub fn related<B: Block>(&self, id: Memo<Option<Uuid>>) -> Rc<RelatedBlock<B>> {
        let related = Rc::new(RelatedBlock::<B>::new(
            self.host().clone(),
            Arc::clone(self.client()),
        ));
        let pumped = Rc::clone(&related);
        self.each_frame(move || pumped.pump(id.get_untracked()));
        related
    }

    pub fn resolve(
        &self,
        referencing: Memo<Option<Uuid>>,
        reference: ReadSignal<Option<BlockRef>>,
    ) -> Memo<Option<Uuid>> {
        let (resolved, set_resolved) = create_signal(None::<Uuid>);
        let cache = RefCell::new(ReferenceResolutionCache::default());
        let client = Arc::clone(self.client());
        self.each_frame(move || {
            let mut cache = cache.borrow_mut();
            cache.poll();
            let found = referencing.get_untracked().and_then(|referencing| {
                let reference = reference.get_untracked()?;
                cache.resolve(&client, referencing, reference)
            });
            set_resolved.set(found);
        });
        create_memo(move || resolved.get())
    }
}
