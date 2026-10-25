use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    hash::Hash,
    rc::Rc,
};

use be_block::be_model::{Document, Field, FieldRef, List, Model};
use be_block::{LiveEdit, ObjectId, Root, Touched};
use block_type_guest::editor::{EditorMirror, FromEditor, Gesture};
use reactive::{
    Get, KeyedStore, OwnedSignal, OwnedStored, Pending, ReadSignal, Ready, Set, Stored,
    create_signal, on_cleanup,
};

use crate::EditorHost;

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

pub use block_type_guest::editor::Projected;

struct Projection<C: LiveEdit> {
    host: EditorHost,
    block: Option<uuid::Uuid>,
    mirror: RefCell<EditorMirror<C>>,
    told: Cell<bool>,
    watchers: Rc<RefCell<Watchers<C>>>,
    next_watcher: Cell<u64>,
    touched: RefCell<Vec<Touched>>,
    announced: OwnedSignal<u64>,
    due: RefCell<Option<Rc<dyn Fn()>>>,
}

impl<C: LiveEdit> Drop for Projection<C> {
    fn drop(&mut self) {
        self.host.unfollow(self.block);
    }
}

impl<C: LiveEdit + Clone + Default> Projection<C> {
    fn new(host: EditorHost, block: Option<uuid::Uuid>) -> Self {
        host.follow(block);
        Self {
            host,
            block,
            mirror: RefCell::new(EditorMirror::default()),
            told: Cell::new(false),
            watchers: Rc::new(RefCell::new(Watchers::new())),
            next_watcher: Cell::new(0),
            touched: RefCell::new(Vec::new()),
            announced: OwnedSignal::new(0),
            due: RefCell::new(None),
        }
    }

    fn loaded(&self) -> bool {
        self.mirror.borrow().is_loaded()
    }

    fn record(&self) {
        self.mirror.borrow_mut().record();
    }

    fn stop_recording(&self) {
        self.mirror.borrow_mut().stop_recording();
    }

    fn take_projected(&self) -> Vec<Projected<C::Op>> {
        self.adopt_lazily();
        self.mirror.borrow_mut().take_projected()
    }

    fn on_due(&self, due: impl Fn() + 'static) {
        *self.due.borrow_mut() = Some(Rc::new(due));
    }

    fn ready(&self) -> Ready<()> {
        self.announced.get();
        self.adopt_lazily();
        let told = self.told.get() || self.due.borrow().is_none();
        if self.loaded() && told {
            Ok(())
        } else {
            Err(Pending::Loading)
        }
    }

    fn revision(&self) -> Ready<u64> {
        self.ready().map(|()| self.mirror.borrow().revision())
    }

    fn read<T>(&self, read: impl FnOnce(&C) -> T) -> Ready<T> {
        self.ready().map(|()| read(self.mirror.borrow().visible()))
    }

    fn watch<T>(&self, key: Option<Touched>, project: impl Fn(&C) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + PartialEq + 'static,
    {
        let read = create_signal(project(self.mirror.borrow().visible()));
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
        project(self.mirror.borrow().visible(), &store);
        self.register(None, Rc::new(move |content, _| project(content, &store)));
        store
    }

    fn operate(&self, operation: C::Op) {
        if !self.host.editable() || !self.loaded() {
            return;
        }
        self.operate_anyway(operation);
    }

    fn gesture(&self, gesture: Gesture) {
        if self.host.editable() && self.loaded() {
            self.host
                .send_to_replica(self.block, FromEditor::Gesture(gesture));
        }
    }

    fn operate_anyway(&self, operation: C::Op) {
        let sent = self.mirror.borrow_mut().operate(operation);
        let Some(sent) = sent else {
            return;
        };
        self.collect_touched();
        self.host.send_to_replica(self.block, sent);
        self.notify();
        self.announced.set(self.mirror.borrow().revision());
    }

    fn pump(&self) {
        self.adopt();
        self.notify();
        if self.loaded() {
            self.told.set(true);
        }
        self.announced.set(self.mirror.borrow().revision());
    }

    fn collect_touched(&self) {
        let touched = self.mirror.borrow_mut().take_touched();
        self.touched.borrow_mut().extend(touched);
    }

    fn notify(&self) {
        let touched = std::mem::take(&mut *self.touched.borrow_mut());
        if touched.is_empty() {
            return;
        }
        let runs = self.watchers.borrow().touched_by(&touched);
        for run in runs {
            run(self.mirror.borrow().visible(), &touched);
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
            let answer = self.mirror.borrow_mut().receive(update);
            if let Some(answer) = answer {
                self.host.send_to_replica(self.block, answer);
            }
        }
        self.collect_touched();
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

    pub fn stop_recording(&self) {
        self.inner().stop_recording();
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

    pub fn on_touched(&self, key: Touched, run: impl Fn(&C, &[Touched]) + 'static) {
        let projection = self.inner();
        run(projection.mirror.borrow().visible(), &[Touched::Everything]);
        projection.register(Some(key), Rc::new(run));
    }

    pub fn operate(&self, operation: C::Op) {
        self.inner().operate(operation);
    }

    pub fn operate_anyway(&self, operation: C::Op) {
        self.inner().operate_anyway(operation);
    }

    pub fn begin_gesture(&self) {
        self.inner().gesture(Gesture::Begin);
    }

    pub fn commit_gesture(&self) {
        self.inner().gesture(Gesture::Commit);
    }

    pub fn cancel_gesture(&self) {
        self.inner().gesture(Gesture::Cancel);
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
        let objects = create_signal(Objects::default());
        self.on_touched(Touched::Subtree(owner), move |content, touched| {
            let next = objects.with_untracked(|held| read(content, touched, held));
            objects.set(Objects(Rc::new(next)));
        });
        objects.read_only()
    }

    pub fn object<T>(&self, id: ObjectId) -> ReadSignal<Option<T>>
    where
        T: Model + Clone + PartialEq + 'static,
    {
        let projection = self.inner();
        let read = create_signal(projection.mirror.borrow().visible().read::<T>(id));
        projection.register(
            Some(Touched::Subtree(id)),
            Rc::new(move |content: &Document<R>, _: &[Touched]| {
                let next = match read.get_untracked() {
                    Some(held) => content.reread(id, held),
                    None => content.read(id),
                };
                read.set(next);
            }),
        );
        read.read_only()
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
