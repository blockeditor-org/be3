use std::cell::{Cell, RefCell};
use std::future::Future;
use std::hash::Hash;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Wake, Waker};

use block::Block;
use block_client::BlockHandle;
use reactive::{KeyedStore, ReadSignal, batch, create_signal};
use uuid::Uuid;

struct Notify {
    pending: AtomicBool,
    wake: Box<dyn Fn() + Send + Sync>,
}

impl Wake for Notify {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.pending.store(true, Ordering::Release);
        (self.wake)();
    }
}

pub struct BlockWatch<B: Block> {
    future: Pin<Box<dyn Future<Output = ()>>>,
    changed: Rc<Cell<bool>>,
    notify: Arc<Notify>,
    block: PhantomData<B>,
}

impl<B: Block> BlockWatch<B> {
    pub fn new(handle: &BlockHandle<B>, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let changed = Rc::new(Cell::new(false));
        let observed = changed.clone();
        let watched = handle.clone();
        let future = Box::pin(async move {
            watched
                .wait_until(move |_| {
                    observed.set(true);
                    false
                })
                .await;
        });
        Self {
            future,
            changed,
            notify: Arc::new(Notify {
                pending: AtomicBool::new(true),
                wake: Box::new(wake),
            }),
            block: PhantomData,
        }
    }

    pub fn take(&mut self) -> bool {
        if self.notify.pending.swap(false, Ordering::AcqRel) {
            let waker = Waker::from(self.notify.clone());
            let _ = self.future.as_mut().poll(&mut Context::from_waker(&waker));
        }
        self.changed.replace(false)
    }
}

type Projection<B> = Rc<dyn Fn(&B)>;

pub struct BlockSource<B: Block> {
    handle: BlockHandle<B>,
    watch: RefCell<BlockWatch<B>>,
    projections: RefCell<Vec<Projection<B>>>,
    seeded: Cell<bool>,
}

impl<B: Block> BlockSource<B> {
    pub fn new(handle: BlockHandle<B>, wake: impl Fn() + Send + Sync + 'static) -> Rc<Self> {
        let mut watch = BlockWatch::new(&handle, wake);
        let seeded = watch.take();
        Rc::new(Self {
            handle,
            watch: RefCell::new(watch),
            projections: RefCell::new(Vec::new()),
            seeded: Cell::new(seeded),
        })
    }

    pub fn handle(&self) -> &BlockHandle<B> {
        &self.handle
    }

    pub fn id(&self) -> Uuid {
        self.handle.id()
    }

    pub fn pump(&self) {
        let changed = self.watch.borrow_mut().take();
        if !changed && self.seeded.get() {
            return;
        }
        batch(|| {
            let Some(block) = self.handle.read() else {
                return;
            };
            self.seeded.set(true);
            let projections = self.projections.borrow().clone();
            for projection in projections {
                projection(&block);
            }
        });
    }

    pub fn operate(&self, operation: B::Operation) {
        self.handle.operate(operation);
        self.pump();
    }

    pub fn operate_grouped(&self, operations: impl IntoIterator<Item = B::Operation>) {
        self.handle.operate_grouped(operations);
        self.pump();
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
        let seed = match self.handle.read() {
            Some(block) => project(&block),
            None => initial,
        };
        let (read, write) = create_signal(seed);
        self.projections
            .borrow_mut()
            .push(Rc::new(move |block| write.set(project(block))));
        read
    }

    pub fn project_keyed<K, V>(
        &self,
        project: impl Fn(&B, &KeyedStore<K, V>) + 'static,
    ) -> KeyedStore<K, V>
    where
        K: Clone + Eq + Hash + 'static,
        V: Clone + PartialEq + 'static,
    {
        let store: KeyedStore<K, V> = KeyedStore::new();
        if let Some(block) = self.handle.read() {
            project(&block, &store);
        }
        let target = store.clone();
        self.projections
            .borrow_mut()
            .push(Rc::new(move |block| project(block, &target)));
        store
    }
}

#[cfg(test)]
mod tests;
