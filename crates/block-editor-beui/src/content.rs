use std::hash::Hash;
use std::rc::Rc;

use be_block::be_model::{Document, Field, FieldRef, List, Model};
use be_block::{LiveEdit, ObjectId, Root, Touched};
use beui::reactive::*;
use block_editor_plugin::{EditorHost, LiveContent, Objects, Projected, Subscription, Unready};

pub(crate) fn ready<T>(read: Result<T, Unready>) -> Ready<T> {
    read.map_err(|unready| match unready {
        Unready::Loading => Pending::Loading,
        Unready::Failed(error) => Pending::Failed(error),
    })
}

pub(crate) fn keep(subscription: Subscription) {
    on_cleanup(move || drop(subscription));
}

struct Projection<C: LiveEdit> {
    _announcing: Subscription,
    announced: OwnedSignal<u64>,
    live: LiveContent<C>,
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
        Self::from(LiveContent::new(host, block))
    }
}

impl<C: LiveEdit + Clone + Default + 'static> From<LiveContent<C>> for OwnedContentProjection<C> {
    fn from(live: LiveContent<C>) -> Self {
        let announced = OwnedSignal::new(0);
        let bumped = announced.handle();
        let state = OwnedStored::new(Projection {
            _announcing: live.on_revision(move |_| bumped.update(|count| *count += 1)),
            announced,
            live,
        });
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

    pub fn live(&self) -> LiveContent<C> {
        self.inner().live.clone()
    }

    pub fn is_alive(&self) -> bool {
        self.0.is_alive()
    }

    pub fn record(&self) {
        self.inner().live.record();
    }

    pub fn stop_recording(&self) {
        self.inner().live.stop_recording();
    }

    pub fn take_projected(&self) -> Vec<Projected<C::Op>> {
        self.inner().live.take_projected()
    }

    pub fn on_due(&self, due: impl Fn() + 'static) {
        self.inner().live.on_due(due);
    }

    pub fn content_type(&self) -> uuid::Uuid {
        C::CONTENT_TYPE
    }

    pub fn block(&self) -> Option<uuid::Uuid> {
        self.inner().live.block()
    }

    pub fn ready(&self) -> Ready<()> {
        let projection = self.inner();
        projection.announced.get();
        ready(projection.live.ready())
    }

    pub fn revision(&self) -> Ready<u64> {
        let projection = self.inner();
        projection.announced.get();
        ready(projection.live.revision())
    }

    pub fn read<T>(&self, read: impl FnOnce(&C) -> T) -> Ready<T> {
        let projection = self.inner();
        projection.announced.get();
        ready(projection.live.read(read))
    }

    fn watch<T>(&self, key: Option<Touched>, project: impl Fn(&C) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + PartialEq + 'static,
    {
        let live = self.live();
        let read = create_signal(live.visible(&project));
        let run = move |content: &C, _: &[Touched]| read.set(project(content));
        keep(match key {
            Some(key) => live.on_touched(key, run),
            None => live.on_change(run),
        });
        read.read_only()
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

    pub fn project_keyed<K, V>(
        &self,
        project: impl Fn(&C, &KeyedStore<K, V>) + 'static,
    ) -> KeyedStore<K, V>
    where
        K: Clone + Eq + Hash + 'static,
        V: Clone + PartialEq + 'static,
    {
        let live = self.live();
        let store: KeyedStore<K, V> = KeyedStore::new();
        live.visible(|content| project(content, &store));
        keep(live.on_change(move |content, _| project(content, &store)));
        store
    }

    pub fn on_touched(&self, key: Touched, run: impl Fn(&C, &[Touched]) + 'static) {
        let live = self.live();
        live.visible(|content| run(content, &[Touched::Everything]));
        keep(live.on_touched(key, run));
    }

    pub fn operate(&self, operation: C::Op) {
        self.inner().live.operate(operation);
    }

    pub fn operate_anyway(&self, operation: C::Op) {
        self.inner().live.operate_anyway(operation);
    }

    pub fn begin_gesture(&self) {
        self.inner().live.begin_gesture();
    }

    pub fn commit_gesture(&self) {
        self.inner().live.commit_gesture();
    }

    pub fn cancel_gesture(&self) {
        self.inner().live.cancel_gesture();
    }

    pub fn pump(&self) {
        self.inner().live.pump();
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
        let objects = create_signal(Objects::default());
        self.on_touched(Touched::Subtree(owner), move |content, touched| {
            let next = objects
                .with_untracked(|held| Objects::reread(content, owner, field, touched, held));
            objects.set(next);
        });
        objects.read_only()
    }

    pub fn object<T>(&self, id: ObjectId) -> ReadSignal<Option<T>>
    where
        T: Model + Clone + PartialEq + 'static,
    {
        let live = self.live();
        let read = create_signal(live.visible(|content| content.read::<T>(id)));
        keep(live.on_touched(
            Touched::Subtree(id),
            move |content: &Document<R>, _: &[Touched]| {
                let next = match read.get_untracked() {
                    Some(held) => content.reread(id, held),
                    None => content.read(id),
                };
                read.set(next);
            },
        ));
        read.read_only()
    }
}
