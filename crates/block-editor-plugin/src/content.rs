use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
    rc::Rc,
};

use be_block::be_model::{Document, Field, FieldRef, List, Model};
use be_block::{LiveEdit, ObjectId, Root, Touched};
use reactive::{KeyedStore, ReadSignal, WriteSignal, batch, create_signal, on_cleanup};

use crate::{EditorHost, host::ContentUpdate};

type Run<C> = Rc<dyn Fn(&C)>;

struct Watchers<C> {
    unkeyed: Vec<(u64, Run<C>)>,
    keyed: HashMap<Touched, Vec<(u64, Run<C>)>>,
}

impl<C> Watchers<C> {
    fn new() -> Self {
        Self {
            unkeyed: Vec::new(),
            keyed: HashMap::new(),
        }
    }

    fn held(&mut self, key: Option<Touched>) -> Option<&mut Vec<(u64, Run<C>)>> {
        match key {
            Some(key) => self.keyed.get_mut(&key),
            None => Some(&mut self.unkeyed),
        }
    }

    fn touched_by(&self, touched: &[Touched]) -> Vec<Run<C>> {
        let mut runs: Vec<Run<C>> = self.unkeyed.iter().map(|(_, run)| Rc::clone(run)).collect();
        if touched.contains(&Touched::Everything) {
            runs.extend(self.keyed.values().flatten().map(|(_, run)| Rc::clone(run)));
            return runs;
        }
        let keys: HashSet<&Touched> = touched.iter().collect();
        for key in keys {
            if let Some(held) = self.keyed.get(key) {
                runs.extend(held.iter().map(|(_, run)| Rc::clone(run)));
            }
        }
        runs
    }
}

pub struct ContentProjection<C: LiveEdit> {
    host: EditorHost,
    block: Option<uuid::Uuid>,
    confirmed: RefCell<C>,
    visible: RefCell<C>,
    pending: RefCell<VecDeque<C::Op>>,
    acknowledged: Cell<u64>,
    loaded: Cell<bool>,
    resyncing: Cell<bool>,
    watchers: Rc<RefCell<Watchers<C>>>,
    next_watcher: Cell<u64>,
    touched: RefCell<Vec<Touched>>,
    revision: Cell<u64>,
    announced: ReadSignal<u64>,
    announce: WriteSignal<u64>,
    loaded_signals: RefCell<Vec<WriteSignal<bool>>>,
}

impl<C: LiveEdit + Clone + Default> ContentProjection<C> {
    pub fn new(host: EditorHost, block: Option<uuid::Uuid>) -> Self {
        let (announced, announce) = create_signal(0);
        Self {
            host,
            block,
            confirmed: RefCell::new(C::default()),
            visible: RefCell::new(C::default()),
            pending: RefCell::new(VecDeque::new()),
            acknowledged: Cell::new(0),
            loaded: Cell::new(false),
            resyncing: Cell::new(false),
            watchers: Rc::new(RefCell::new(Watchers::new())),
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
        self.register(key, Rc::new(move |content| write.set(project(content))));
        read
    }

    fn register(&self, key: Option<Touched>, run: Run<C>) {
        let id = self.next_watcher.get();
        self.next_watcher.set(id + 1);
        {
            let mut watchers = self.watchers.borrow_mut();
            match key {
                Some(key) => watchers.keyed.entry(key).or_default().push((id, run)),
                None => watchers.unkeyed.push((id, run)),
            }
        }
        let watchers = Rc::downgrade(&self.watchers);
        on_cleanup(move || {
            let Some(watchers) = watchers.upgrade() else {
                return;
            };
            let mut watchers = watchers.borrow_mut();
            let empty = watchers.held(key).is_some_and(|held| {
                held.retain(|(held, _)| *held != id);
                held.is_empty()
            });
            if let (Some(key), true) = (key, empty) {
                watchers.keyed.remove(&key);
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
        self.register(None, Rc::new(move |content| project(content, &target)));
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

    pub fn pump(&self) {
        self.adopt();
        self.notify();
        self.announce.set(self.revision.get());
    }

    fn notify(&self) {
        let touched = std::mem::take(&mut *self.touched.borrow_mut());
        if touched.is_empty() {
            return;
        }
        let runs = self.watchers.borrow().touched_by(&touched);
        batch(|| {
            for run in runs {
                run(&self.visible.borrow());
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
                    self.resyncing.set(false);
                    if !self.loaded.replace(true) {
                        for loaded in self.loaded_signals.borrow_mut().drain(..) {
                            loaded.set(true);
                        }
                    }
                    self.rebuild();
                }
                ContentUpdate::Operations(operations) => {
                    if !self.loaded.get() || self.resyncing.get() {
                        continue;
                    }
                    let mut rebuild = false;
                    for (bytes, mine) in operations {
                        let Ok(operation) = C::decode_operation(&bytes) else {
                            self.resyncing.set(true);
                            self.host.request_content_resend(self.block);
                            break;
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
