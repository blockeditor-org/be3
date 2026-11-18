use std::rc::Rc;

use be_block_content::be_block_model::{Document, Field, FieldRef, List, Model};
use be_block_content::{LiveEdit, ObjectId, Root, Touched};
use be_sdk::LackingEdit;
use be_sdk::{
    BlockWatch, ContentGesture, EditorHost, LiveContent, Objects, Projected, Subscription, Unready,
};
use beui::reactive::*;

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
    announcing: std::cell::RefCell<Vec<Subscription>>,
    announced: OwnedTrigger,
    /// Whether the content arrived: what every read waits on, so an editor never shows the
    /// empty content first.
    arrived: OwnedSource<()>,
    /// The graph's watch of the block, for a projection of a block that may be one this
    /// viewer cannot view (`unless_unviewable`).
    found: Option<BlockWatch>,
    live: LiveContent<C>,
    payload: std::cell::RefCell<Option<(Scope, AsyncMemo<Payload>)>>,
}

pub struct ContentProjection<C: LiveEdit>(Stored<Projection<C>>);

/// The edits made on this device that the server lacks (`ContentProjection::lacking`): a new
/// value whenever the replica says they changed.
pub struct Lacking<Op>(pub Rc<Vec<LackingEdit<Op>>>);

impl<Op> Clone for Lacking<Op> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<Op> PartialEq for Lacking<Op> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl<Op> std::ops::Deref for Lacking<Op> {
    type Target = [LackingEdit<Op>];

    fn deref(&self) -> &[LackingEdit<Op>] {
        &self.0
    }
}

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
        Self::following(live, None)
    }
}

impl<C: LiveEdit + Clone + Default + 'static> OwnedContentProjection<C> {
    /// A projection of a block that may be one this viewer cannot view, which `found` (the
    /// graph's watch of it) tells: deleted elsewhere, or no longer shared, its content never
    /// arrives, so the projection settles with the empty content rather than hold a `Loading`
    /// for ever.
    pub(crate) fn unless_unviewable(live: LiveContent<C>, found: BlockWatch) -> Self {
        Self::following(live, Some(found))
    }

    fn following(live: LiveContent<C>, found: Option<BlockWatch>) -> Self {
        let state = OwnedStored::new(Projection {
            announcing: std::cell::RefCell::new(Vec::new()),
            announced: OwnedTrigger::new(),
            arrived: OwnedSource::from(arrival(&live, found.as_ref())),
            found,
            live,
            payload: std::cell::RefCell::new(None),
        });
        let held = state.handle();
        let settle = move || {
            if let Some(projection) = held.try_value() {
                projection
                    .arrived
                    .set(arrival(&projection.live, projection.found.as_ref()));
            }
        };
        let projection = state.value();
        let mut announcing = vec![projection.live.on_revision(move |_| {
            settle();
            if let Some(projection) = held.try_value() {
                projection.announced.notify();
            }
        })];
        if let Some(found) = &projection.found {
            announcing.push(found.on_change(settle));
        }
        projection.announcing.replace(announcing);
        Self {
            handle: ContentProjection(state.handle()),
            _state: state,
        }
    }
}

/// Whether `live` arrived; while it is still loading, settled when `found` says the block is
/// not one this viewer can view.
fn arrival<C: LiveEdit + Clone + Default + 'static>(
    live: &LiveContent<C>,
    found: Option<&BlockWatch>,
) -> Ready<()> {
    let viewable = || match found.map(BlockWatch::read) {
        Some(Ok(list)) => list.first().is_some_and(|info| info.access.can_view()),
        _ => true,
    };
    match ready(live.ready()) {
        Err(Pending::Loading) if !viewable() => Ok(()),
        arrived => arrived,
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

    /// Whether the content arrived, waiting until it does (at the nearest `Loading`, when
    /// read outside a memo; a memo that reads it is pending until then).
    pub fn ready(&self) -> Ready<()> {
        self.inner().arrived.get()
    }

    /// Subscribes the running computation to every change of the content, and to its arrival,
    /// without reading it: for one that recomputes from the content some other way.
    pub fn track(&self) {
        let projection = self.inner();
        projection.announced.track();
        projection.arrived.track();
    }

    pub fn revision(&self) -> Ready<u64> {
        let projection = self.inner();
        projection.announced.track();
        projection.arrived.get()?;
        ready(projection.live.revision())
    }

    pub fn read<T>(&self, read: impl FnOnce(&C) -> T) -> Ready<T> {
        let projection = self.inner();
        projection.announced.track();
        projection.arrived.get()?;
        ready(projection.live.read(read))
    }

    /// `signal`, pending until the content arrived: an editor never shows the empty content
    /// first.
    fn once_arrived<T: Clone + PartialEq + 'static>(&self, signal: Signal<T>) -> Memo<T> {
        let arrived = self.inner().arrived.handle();
        create_memo(move || {
            arrived.track();
            signal.get()
        })
    }

    fn watch<T>(&self, key: Option<Touched>, project: impl Fn(&C) -> T + 'static) -> Memo<T>
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
        self.once_arrived(read)
    }

    pub fn project<T>(&self, project: impl Fn(&C) -> T + 'static) -> Memo<T>
    where
        T: Clone + PartialEq + 'static,
    {
        self.watch(None, project)
    }

    pub fn project_on<T>(&self, key: Touched, project: impl Fn(&C) -> T + 'static) -> Memo<T>
    where
        T: Clone + PartialEq + 'static,
    {
        self.watch(Some(key), project)
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

    /// The edits made on this device that the server lacks, each with whether the server
    /// refused it (`LiveContent::lacking`), following them from the first call.
    pub fn lacking(&self) -> ReadSignal<Lacking<C::Op>> {
        let live = self.live();
        live.follow_lacking();
        let lacking = create_signal(Lacking(Rc::new(live.lacking())));
        let read = live.clone();
        keep(live.on_lacking(move || lacking.set(Lacking(Rc::new(read.lacking())))));
        lacking.read_only()
    }

    /// Lets an edit the server refused go (`LiveContent::dismiss_lacking`).
    pub fn dismiss_lacking(&self, id: u64) {
        self.inner().live.dismiss_lacking(id);
    }

    /// Makes an edit in the open gesture `gesture` (`LiveContent::operate_in`).
    pub fn operate_in(&self, gesture: ContentGesture, operation: C::Op) {
        self.inner().live.operate_in(gesture, operation);
    }

    /// Opens an undo group (`LiveContent::begin_gesture`).
    pub fn begin_gesture(&self) -> ContentGesture {
        self.inner().live.begin_gesture()
    }

    /// Opens a tentative gesture (`LiveContent::begin_tentative`): its edits show at once in
    /// every view of the block on this device and are sent only when it commits.
    pub fn begin_tentative(&self) -> ContentGesture {
        self.inner().live.begin_tentative()
    }

    pub fn commit_gesture(&self, gesture: ContentGesture) {
        self.inner().live.commit_gesture(gesture);
    }

    pub fn cancel_gesture(&self, gesture: ContentGesture) {
        self.inner().live.cancel_gesture(gesture);
    }

    /// Whether `gesture` is open: begun, and ended neither here nor by the replica.
    pub fn gesture_open(&self, gesture: ContentGesture) -> bool {
        self.inner().live.gesture_open(gesture)
    }

    /// Runs `run`, for as long as the current scope lives, for each of this editor's gestures
    /// the replica ended (`LiveContent::on_gesture_ended`): it is cancelled already, and the
    /// editor only stops it, as a drag lets go of what it carried.
    pub fn on_gesture_ended(&self, run: impl Fn(ContentGesture) + 'static) {
        keep(self.live().on_gesture_ended(run));
    }

    pub fn break_step(&self) {
        self.inner().live.break_step();
    }

    pub fn pump(&self) {
        self.inner().live.pump();
    }
}

/// A blob's payload as `ContentProjection::payload` reads it. Two are equal only when they
/// are the same read, so a signal holding one never compares megabytes.
#[derive(Clone)]
pub struct Payload(Rc<[u8]>);

impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::ops::Deref for Payload {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl<K: be_block_content::BlobKind> ContentProjection<be_block_content::Blob<K>> {
    /// The payload, which a kind that loads only its header (`BlobKind::LOADS`) leaves in the
    /// store: read by range once the first snapshot arrives, and again with each new one (a
    /// replacement), never on a header edit.
    pub fn payload(&self) -> AsyncMemo<Payload> {
        let projection = self.inner();
        if let Some((_, payload)) = projection.payload.borrow().as_ref() {
            return *payload;
        }
        let announced = projection.announced.handle();
        let this = *self;
        // Kept with the projection, which outlives the scope that first asked for it.
        let scope = Scope::detached();
        let payload = scope.run(|| {
            let stored = create_memo(move || {
                announced.track();
                let live = this.live();
                live.stored_length()
                    .ok()
                    .map(|length| (length, live.snapshots()))
            });
            create_async_memo(move || {
                let stored = stored.get();
                let live = this.live();
                let read = stored.map(|(length, _)| {
                    let (start, held) = live.visible(|blob| {
                        let start =
                            be_block_content::encode_streamed(blob.header(), &[]).len() as u64;
                        let whole = start + blob.data().len() as u64 == length;
                        (start, whole.then(|| Payload(blob.data().into())))
                    });
                    match held {
                        Some(held) => Ok(held),
                        None => Err(live.read_range(start, length.saturating_sub(start))),
                    }
                });
                async move {
                    match read.ok_or(Pending::Loading)? {
                        Ok(held) => Ok(held),
                        Err(reading) => reading
                            .await
                            .map(|bytes| Payload(bytes.into()))
                            .map_err(Pending::Failed),
                    }
                }
            })
        });
        *projection.payload.borrow_mut() = Some((scope, payload));
        payload
    }
}

impl<R: Root + 'static> ContentProjection<Document<R>>
where
    Document<R>: LiveEdit + Clone + Default,
{
    pub fn field<M: 'static, F>(&self, object: ObjectId, field: FieldRef<M, F>) -> Memo<F>
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
    ) -> Memo<Vec<ObjectId>> {
        self.project_on(Touched::Field(owner, field.index()), move |content| {
            content.ids(owner, field)
        })
    }

    pub fn objects<M: 'static, T>(
        &self,
        owner: ObjectId,
        field: FieldRef<M, List<T>>,
    ) -> Memo<Objects<T>>
    where
        T: Model + 'static,
    {
        let objects = create_signal(Objects::default());
        self.on_touched(Touched::Subtree(owner), move |content, touched| {
            let next = objects
                .with_untracked(|held| Objects::reread(content, owner, field, touched, held));
            objects.set(next);
        });
        self.once_arrived(objects)
    }

    pub fn object<T>(&self, id: ObjectId) -> Memo<Option<T>>
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
        self.once_arrived(read)
    }
}

/// Holds what a value control's drag writes into a block in a tentative gesture
/// (decisions/tentative_edits_wait_for_release.md): pass `drag` to the control's drag callback
/// (`NumberInput`'s `on_scrub`, `on_drag_change` on `Slider` and the colour controls) and make
/// the editor's edits through `operate`. Every view of the block on this device follows the
/// drag; it is written once, as one undo step, when kept, and leaves nothing when called off.
pub struct DragHold<C: LiveEdit> {
    content: ContentProjection<C>,
    gesture: StoredCell<Option<ContentGesture>>,
}

impl<C: LiveEdit> Clone for DragHold<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: LiveEdit> Copy for DragHold<C> {}

impl<C: LiveEdit + Clone + Default + 'static> DragHold<C> {
    pub fn new(content: ContentProjection<C>) -> Self {
        Self {
            content,
            gesture: StoredCell::new(None),
        }
    }

    /// Follows the control's drag: opens the gesture as it starts, commits it when it is kept
    /// and cancels it when it is called off. A gesture the replica ended meanwhile takes no
    /// more edits, and ending it sends nothing.
    pub fn drag(&self, change: beui::unstyled::ValueDrag) {
        use beui::unstyled::ValueDrag;
        match change {
            ValueDrag::Started => {
                if self.gesture.get().is_none() {
                    self.gesture.set(Some(self.content.begin_tentative()));
                }
            }
            ValueDrag::Kept => {
                if let Some(gesture) = self.gesture.take() {
                    self.content.commit_gesture(gesture);
                }
            }
            ValueDrag::Cancelled => {
                if let Some(gesture) = self.gesture.take() {
                    self.content.cancel_gesture(gesture);
                }
            }
        }
    }

    /// Makes `operation` in the drag's gesture while one is held, else outside any.
    pub fn operate(&self, operation: C::Op) {
        match self.gesture.get() {
            Some(gesture) => self.content.operate_in(gesture, operation),
            None => self.content.operate(operation),
        }
    }
}
