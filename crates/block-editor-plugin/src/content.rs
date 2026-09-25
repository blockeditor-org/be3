use std::{
    cell::{Cell, RefCell},
    collections::{HashSet, VecDeque},
    hash::Hash,
    rc::Rc,
};

use be_block::be_model::{Document, Field, FieldRef, List, Model};
use be_block::{LiveEdit, ObjectId, Root, Touched};
use beui::reactive::{KeyedStore, ReadSignal, WriteSignal, batch, create_signal, on_cleanup};

use crate::{EditorHost, host::ContentUpdate};

struct Watcher<C> {
    key: Option<Touched>,
    run: Rc<dyn Fn(&C)>,
}

impl<C> Clone for Watcher<C> {
    fn clone(&self) -> Self {
        Self {
            key: self.key,
            run: Rc::clone(&self.run),
        }
    }
}

type Watchers<C> = Rc<RefCell<Vec<(u64, Watcher<C>)>>>;

pub struct ContentProjection<C: LiveEdit> {
    host: EditorHost,
    block: Option<uuid::Uuid>,
    confirmed: RefCell<C>,
    visible: RefCell<C>,
    pending: RefCell<VecDeque<C::Op>>,
    acknowledged: Cell<u64>,
    loaded: Cell<bool>,
    watchers: Watchers<C>,
    next_watcher: Cell<u64>,
    touched: RefCell<Vec<Touched>>,
    revision: Cell<u64>,
    announced: ReadSignal<u64>,
    announce: WriteSignal<u64>,
    loaded_signals: RefCell<Vec<WriteSignal<bool>>>,
}

impl<C: LiveEdit + Clone + Default> ContentProjection<C> {
    pub(crate) fn new(host: EditorHost, block: Option<uuid::Uuid>) -> Self {
        let (announced, announce) = create_signal(0);
        Self {
            host,
            block,
            confirmed: RefCell::new(C::default()),
            visible: RefCell::new(C::default()),
            pending: RefCell::new(VecDeque::new()),
            acknowledged: Cell::new(0),
            loaded: Cell::new(false),
            watchers: Rc::new(RefCell::new(Vec::new())),
            next_watcher: Cell::new(0),
            touched: RefCell::new(Vec::new()),
            revision: Cell::new(0),
            announced,
            announce,
            loaded_signals: RefCell::new(Vec::new()),
        }
    }

    pub fn content_type(&self) -> uuid::Uuid {
        C::CONTENT_TYPE
    }

    pub fn block(&self) -> Option<uuid::Uuid> {
        self.block
    }

    pub fn loaded(&self) -> ReadSignal<bool> {
        let (loaded, set_loaded) = create_signal(self.loaded.get());
        self.loaded_signals.borrow_mut().push(set_loaded);
        loaded
    }

    pub fn revision(&self) -> Option<u64> {
        self.announced.get();
        self.adopt();
        self.loaded.get().then(|| self.revision.get())
    }

    pub fn read<T>(&self, read: impl FnOnce(&C) -> T) -> Option<T> {
        self.announced.get();
        self.adopt();
        self.loaded.get().then(|| read(&self.visible.borrow()))
    }

    pub fn project<T>(&self, project: impl Fn(&C) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + PartialEq + 'static,
    {
        self.watch(None, project)
    }

    pub fn project_on<T>(&self, key: Touched, project: impl Fn(&C) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + PartialEq + 'static,
    {
        self.watch(Some(key), project)
    }

    fn watch<T>(&self, key: Option<Touched>, project: impl Fn(&C) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + PartialEq + 'static,
    {
        let (read, write) = create_signal(project(&self.visible.borrow()));
        self.register(Watcher {
            key,
            run: Rc::new(move |content| write.set(project(content))),
        });
        read
    }

    fn register(&self, watcher: Watcher<C>) {
        let id = self.next_watcher.get();
        self.next_watcher.set(id + 1);
        self.watchers.borrow_mut().push((id, watcher));
        let watchers = Rc::downgrade(&self.watchers);
        on_cleanup(move || {
            if let Some(watchers) = watchers.upgrade() {
                watchers.borrow_mut().retain(|(held, _)| *held != id);
            }
        });
    }

    pub fn project_keyed<K, V>(
        &self,
        project: impl Fn(&C, &KeyedStore<K, V>) + 'static,
    ) -> KeyedStore<K, V>
    where
        K: Clone + Eq + Hash + 'static,
        V: Clone + PartialEq + 'static,
    {
        let store: KeyedStore<K, V> = KeyedStore::new();
        project(&self.visible.borrow(), &store);
        let target = store.clone();
        self.register(Watcher {
            key: None,
            run: Rc::new(move |content| project(content, &target)),
        });
        store
    }

    pub fn operate(&self, operation: C::Op) {
        if !self.host.editable() {
            return;
        }
        self.visible
            .borrow_mut()
            .apply_touching(&operation, &mut self.touched.borrow_mut());
        self.revision.set(self.revision.get() + 1);
        self.host
            .operate_content_at(self.block, C::encode_operation(&operation));
        self.pending.borrow_mut().push_back(operation);
        self.notify();
        self.announce.set(self.revision.get());
    }

    pub(crate) fn pump(&self) {
        self.adopt();
        self.notify();
        self.announce.set(self.revision.get());
    }

    fn notify(&self) {
        let touched = std::mem::take(&mut *self.touched.borrow_mut());
        if touched.is_empty() {
            return;
        }
        let everything = touched.contains(&Touched::Everything);
        let keys: HashSet<Touched> = touched.into_iter().collect();
        let watchers: Vec<Watcher<C>> = self
            .watchers
            .borrow()
            .iter()
            .map(|(_, watcher)| watcher)
            .filter(|watcher| everything || watcher.key.is_none_or(|key| keys.contains(&key)))
            .cloned()
            .collect();
        batch(|| {
            for watcher in watchers {
                (watcher.run)(&self.visible.borrow());
            }
        });
    }

    fn adopt(&self) {
        for update in self.host.take_content_updates(self.block) {
            match update {
                ContentUpdate::Snapshot(content) => {
                    if content.content_type != C::CONTENT_TYPE {
                        continue;
                    }
                    let Ok(confirmed) = C::decode(&content.bytes) else {
                        continue;
                    };
                    let taken = content.applied.saturating_sub(self.acknowledged.get());
                    self.acknowledged.set(content.applied);
                    {
                        let mut pending = self.pending.borrow_mut();
                        for _ in 0..taken.min(pending.len() as u64) {
                            pending.pop_front();
                        }
                    }
                    *self.confirmed.borrow_mut() = confirmed;
                    if !self.loaded.replace(true) {
                        for loaded in self.loaded_signals.borrow_mut().drain(..) {
                            loaded.set(true);
                        }
                    }
                    self.rebuild();
                }
                ContentUpdate::Operations(operations) => {
                    if !self.loaded.get() {
                        continue;
                    }
                    let mut rebuild = false;
                    for (bytes, mine) in operations {
                        let Ok(operation) = C::decode_operation(&bytes) else {
                            rebuild = true;
                            continue;
                        };
                        self.confirmed.borrow_mut().apply(&operation);
                        let mut pending = self.pending.borrow_mut();
                        if mine {
                            pending.pop_front();
                            self.acknowledged.set(self.acknowledged.get() + 1);
                        } else if pending.is_empty() && !rebuild {
                            self.visible
                                .borrow_mut()
                                .apply_touching(&operation, &mut self.touched.borrow_mut());
                            self.revision.set(self.revision.get() + 1);
                        } else {
                            rebuild = true;
                        }
                    }
                    if rebuild {
                        self.rebuild();
                    }
                }
            }
        }
    }

    fn rebuild(&self) {
        let mut visible = self.confirmed.borrow().clone();
        for operation in self.pending.borrow().iter() {
            visible.apply(operation);
        }
        *self.visible.borrow_mut() = visible;
        self.touched.borrow_mut().push(Touched::Everything);
        self.revision.set(self.revision.get() + 1);
    }
}

impl<R: Root + 'static> ContentProjection<Document<R>> {
    pub fn field<M: 'static, F>(&self, object: ObjectId, field: FieldRef<M, F>) -> ReadSignal<F>
    where
        F: Field + Clone + PartialEq + 'static,
    {
        self.project_on(Touched::Field(object, field.index()), move |content| {
            content.field(object, field)
        })
    }

    pub fn ids<M: 'static, T: 'static>(
        &self,
        owner: ObjectId,
        field: FieldRef<M, List<T>>,
    ) -> ReadSignal<Vec<ObjectId>> {
        self.project_on(Touched::Field(owner, field.index()), move |content| {
            content.ids(owner, field)
        })
    }

    pub fn object<T>(&self, id: ObjectId) -> ReadSignal<Option<T>>
    where
        T: Model + Clone + PartialEq + 'static,
    {
        self.project_on(Touched::Subtree(id), move |content| content.read::<T>(id))
    }
}

#[cfg(test)]
mod tests;
