use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
    rc::Rc,
};

use be_block::be_model::{Document, Field, FieldRef, List, Model};
use be_block::{LiveEdit, ObjectId, Root, Touched};
use reactive::{
    Get, KeyedStore, OwnedSignal, OwnedStored, Pending, ReadSignal, Ready, Set, Stored,
    create_signal, on_cleanup,
};

use crate::{EditorHost, host::ContentUpdate};

type Run<C> = Rc<dyn Fn(&C, &[Touched])>;

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

#[derive(Clone, Debug, PartialEq)]
pub enum Projected<Op> {
    Applied(Op),
    Rebuilt,
}

struct Projection<C: LiveEdit> {
    host: EditorHost,
    block: Option<uuid::Uuid>,
    confirmed: RefCell<C>,
    visible: RefCell<C>,
    pending: RefCell<VecDeque<C::Op>>,
    acknowledged: Cell<u64>,
    loaded: Cell<bool>,
    told: Cell<bool>,
    resyncing: Cell<bool>,
    watchers: Rc<RefCell<Watchers<C>>>,
    next_watcher: Cell<u64>,
    touched: RefCell<Vec<Touched>>,
    revision: Cell<u64>,
    announced: OwnedSignal<u64>,
    due: RefCell<Option<Rc<dyn Fn()>>>,
    projected: RefCell<Option<Vec<Projected<C::Op>>>>,
}

impl<C: LiveEdit + Clone + Default> Projection<C> {
    fn new(host: EditorHost, block: Option<uuid::Uuid>) -> Self {
        Self {
            host,
            block,
            confirmed: RefCell::new(C::default()),
            visible: RefCell::new(C::default()),
            pending: RefCell::new(VecDeque::new()),
            acknowledged: Cell::new(0),
            loaded: Cell::new(false),
            told: Cell::new(false),
            resyncing: Cell::new(false),
            watchers: Rc::new(RefCell::new(Watchers::new())),
            next_watcher: Cell::new(0),
            touched: RefCell::new(Vec::new()),
            revision: Cell::new(0),
            announced: OwnedSignal::new(0),
            due: RefCell::new(None),
            projected: RefCell::new(None),
        }
    }

    fn record(&self) {
        self.projected.borrow_mut().get_or_insert_with(Vec::new);
    }

    fn take_projected(&self) -> Vec<Projected<C::Op>> {
        self.adopt_lazily();
        self.projected
            .borrow_mut()
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    fn log(&self, projected: Projected<C::Op>) {
        if let Some(log) = self.projected.borrow_mut().as_mut() {
            log.push(projected);
        }
    }

    fn on_due(&self, due: impl Fn() + 'static) {
        *self.due.borrow_mut() = Some(Rc::new(due));
    }

    fn ready(&self) -> Ready<()> {
        self.announced.get();
        self.adopt_lazily();
        let told = self.told.get() || self.due.borrow().is_none();
        if self.loaded.get() && told {
            Ok(())
        } else {
            Err(Pending::Loading)
        }
    }

    fn revision(&self) -> Ready<u64> {
        self.ready().map(|()| self.revision.get())
    }

    fn read<T>(&self, read: impl FnOnce(&C) -> T) -> Ready<T> {
        self.ready().map(|()| read(&self.visible.borrow()))
    }

    fn watch<T>(&self, key: Option<Touched>, project: impl Fn(&C) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + PartialEq + 'static,
    {
        let read = create_signal(project(&self.visible.borrow()));
        self.register(key, Rc::new(move |content, _| read.set(project(content))));
        read.read_only()
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

    fn project_keyed<K, V>(
        &self,
        project: impl Fn(&C, &KeyedStore<K, V>) + 'static,
    ) -> KeyedStore<K, V>
    where
        K: Clone + Eq + Hash + 'static,
        V: Clone + PartialEq + 'static,
    {
        let store: KeyedStore<K, V> = KeyedStore::new();
        project(&self.visible.borrow(), &store);
        self.register(None, Rc::new(move |content, _| project(content, &store)));
        store
    }

    fn operate(&self, operation: C::Op) {
        if !self.host.editable() || !self.loaded.get() {
            return;
        }
        self.operate_anyway(operation);
    }

    fn operate_anyway(&self, operation: C::Op) {
        self.visible
            .borrow_mut()
            .apply_touching(&operation, &mut self.touched.borrow_mut());
        self.revision.set(self.revision.get() + 1);
        self.host
            .operate_content_at(self.block, C::encode_operation(&operation));
        self.pending.borrow_mut().push_back(operation);
        self.notify();
        self.announced.set(self.revision.get());
    }

    fn pump(&self) {
        self.adopt();
        self.notify();
        if self.loaded.get() {
            self.told.set(true);
        }
        self.announced.set(self.revision.get());
    }

    fn notify(&self) {
        let touched = std::mem::take(&mut *self.touched.borrow_mut());
        if touched.is_empty() {
            return;
        }
        let runs = self.watchers.borrow().touched_by(&touched);
        for run in runs {
            run(&self.visible.borrow(), &touched);
        }
    }

    fn adopt_lazily(&self) {
        self.adopt();
        if self.touched.borrow().is_empty() {
            return;
        }
        let due = self.due.borrow().clone();
        if let Some(due) = due {
            due();
        }
    }

    fn adopt(&self) {
        for update in self.host.take_content_updates(self.block) {
            match update {
                ContentUpdate::Snapshot(content) => {
                    if content.content_type != C::CONTENT_TYPE {
                        continue;
                    }
                    let Ok(mut confirmed) = C::decode(&content.bytes) else {
                        continue;
                    };
                    if confirmed.adopt_session_state(&content.session).is_err() {
                        continue;
                    }
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
                    self.loaded.set(true);
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
                            self.log(Projected::Applied(operation));
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
        self.log(Projected::Rebuilt);
        self.touched.borrow_mut().push(Touched::Everything);
        self.revision.set(self.revision.get() + 1);
    }
}

pub struct ContentProjection<C: LiveEdit>(Stored<Projection<C>>);

impl<C: LiveEdit> Clone for ContentProjection<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: LiveEdit> Copy for ContentProjection<C> {}

impl<C: LiveEdit> PartialEq for ContentProjection<C> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<C: LiveEdit> Eq for ContentProjection<C> {}

pub struct OwnedContentProjection<C: LiveEdit + 'static> {
    handle: ContentProjection<C>,
    _state: OwnedStored<Projection<C>>,
}

impl<C: LiveEdit + Clone + Default + 'static> OwnedContentProjection<C> {
    pub fn new(host: EditorHost, block: Option<uuid::Uuid>) -> Self {
        let state = OwnedStored::new(Projection::new(host, block));
        Self {
            handle: ContentProjection(state.handle()),
            _state: state,
        }
    }
}

impl<C: LiveEdit + 'static> OwnedContentProjection<C> {
    pub fn handle(&self) -> ContentProjection<C> {
        self.handle
    }
}

impl<C: LiveEdit + 'static> std::ops::Deref for OwnedContentProjection<C> {
    type Target = ContentProjection<C>;

    fn deref(&self) -> &ContentProjection<C> {
        &self.handle
    }
}

impl<C: LiveEdit + Clone + Default + 'static> ContentProjection<C> {
    fn inner(&self) -> Rc<Projection<C>> {
        self.0.value()
    }

    pub fn is_alive(&self) -> bool {
        self.0.is_alive()
    }

    pub fn record(&self) {
        self.inner().record();
    }

    pub fn take_projected(&self) -> Vec<Projected<C::Op>> {
        self.inner().take_projected()
    }

    pub fn on_due(&self, due: impl Fn() + 'static) {
        self.inner().on_due(due);
    }

    pub fn content_type(&self) -> uuid::Uuid {
        C::CONTENT_TYPE
    }

    pub fn block(&self) -> Option<uuid::Uuid> {
        self.inner().block
    }

    pub fn ready(&self) -> Ready<()> {
        self.inner().ready()
    }

    pub fn revision(&self) -> Ready<u64> {
        self.inner().revision()
    }

    pub fn read<T>(&self, read: impl FnOnce(&C) -> T) -> Ready<T> {
        self.inner().read(read)
    }

    pub fn project<T>(&self, project: impl Fn(&C) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + PartialEq + 'static,
    {
        self.inner().watch(None, project)
    }

    pub fn project_on<T>(&self, key: Touched, project: impl Fn(&C) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + PartialEq + 'static,
    {
        self.inner().watch(Some(key), project)
    }

    pub fn project_keyed<K, V>(
        &self,
        project: impl Fn(&C, &KeyedStore<K, V>) + 'static,
    ) -> KeyedStore<K, V>
    where
        K: Clone + Eq + Hash + 'static,
        V: Clone + PartialEq + 'static,
    {
        self.inner().project_keyed(project)
    }

    pub fn operate(&self, operation: C::Op) {
        self.inner().operate(operation);
    }

    pub fn operate_anyway(&self, operation: C::Op) {
        self.inner().operate_anyway(operation);
    }

    pub fn pump(&self) {
        self.inner().pump();
    }
}

impl<R: Root + 'static> ContentProjection<Document<R>>
where
    Document<R>: LiveEdit + Clone + Default,
{
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

    pub fn objects<M: 'static, T>(
        &self,
        owner: ObjectId,
        field: FieldRef<M, List<T>>,
    ) -> ReadSignal<Objects<T>>
    where
        T: Model + 'static,
    {
        let projection = self.inner();
        let read = move |content: &Document<R>, touched: &[Touched], held: &Objects<T>| {
            let everything = touched.contains(&Touched::Everything);
            let changed: HashSet<ObjectId> = touched
                .iter()
                .filter_map(|touched| match touched {
                    Touched::Subtree(id) => Some(*id),
                    _ => None,
                })
                .collect();
            let kept: HashMap<ObjectId, Rc<T>> = match everything {
                true => HashMap::new(),
                false => held
                    .iter()
                    .filter(|(id, _)| !changed.contains(id))
                    .map(|(id, object)| (*id, Rc::clone(object)))
                    .collect(),
            };
            content
                .ids(owner, field)
                .into_iter()
                .filter_map(|id| match kept.get(&id) {
                    Some(object) => Some((id, Rc::clone(object))),
                    None => Some((id, Rc::new(content.read::<T>(id)?))),
                })
                .collect::<Vec<_>>()
        };
        let first = read(
            &projection.visible.borrow(),
            &[Touched::Everything],
            &Objects::default(),
        );
        let objects = create_signal(Objects(Rc::new(first)));
        projection.register(
            Some(Touched::Subtree(owner)),
            Rc::new(move |content, touched| {
                let next = objects.with_untracked(|held| read(content, touched, held));
                objects.set(Objects(Rc::new(next)));
            }),
        );
        objects.read_only()
    }

    pub fn object<T>(&self, id: ObjectId) -> ReadSignal<Option<T>>
    where
        T: Model + Clone + PartialEq + 'static,
    {
        self.project_on(Touched::Subtree(id), move |content| content.read::<T>(id))
    }
}

pub struct Objects<T>(Rc<Vec<(ObjectId, Rc<T>)>>);

impl<T> Objects<T> {
    pub fn iter(&self) -> impl Iterator<Item = &(ObjectId, Rc<T>)> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T> Default for Objects<T> {
    fn default() -> Self {
        Self(Rc::new(Vec::new()))
    }
}

impl<T> Clone for Objects<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<T> PartialEq for Objects<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .zip(other.0.iter())
                .all(|((id, held), (other_id, other))| id == other_id && Rc::ptr_eq(held, other))
    }
}

#[cfg(test)]
mod tests;
