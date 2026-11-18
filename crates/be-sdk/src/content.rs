use be_lib_stable_hash::{HashMap, HashSet};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use be_block_content::be_block_model::{Document, FieldRef, List, Model};
use be_block_content::{LiveEdit, ObjectId, Root, Touched};
use be_sdk_type_guest::editor::{EditorMirror, FromEditor, Gesture, LackingEdit};

use crate::EditorHost;
use crate::state::{Callbacks, Subscription, Unready};

type Run<C> = dyn Fn(&C, &[Touched]);

pub use be_sdk_type_guest::editor::Projected;

/// A gesture an editor holds open in a block's content: an undo group
/// (`LiveContent::begin_gesture`), whose edits are written as they happen and undo as one step,
/// or a tentative gesture (`LiveContent::begin_tentative`), whose edits are shown at once on
/// this device but held from peers and the journal until it commits
/// (decisions/tentative_edits_wait_for_release.md). An editor may hold several at once, each
/// ending on its own.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ContentGesture(u64);

struct Projection<C: LiveEdit> {
    host: EditorHost,
    block: Option<uuid::Uuid>,
    mirror: RefCell<EditorMirror<C>>,
    told: Cell<bool>,
    watchers: Callbacks<Option<Touched>, Run<C>>,
    touched: RefCell<Vec<Touched>>,
    announced: Cell<u64>,
    revisions: Callbacks<(), dyn Fn(u64)>,
    lacking_revision: Cell<u64>,
    lacking_followed: Cell<bool>,
    lacking_changes: Callbacks<(), dyn Fn()>,
    ended: Callbacks<(), dyn Fn(ContentGesture)>,
    due: RefCell<Option<Rc<dyn Fn()>>>,
    reads: RefCell<HashMap<u64, Box<Read>>>,
}

type Read = dyn FnOnce(Result<Vec<u8>, String>);

impl<C: LiveEdit> Drop for Projection<C> {
    fn drop(&mut self) {
        self.host.unfollow(self.block);
    }
}

pub struct LiveContent<C: LiveEdit>(Rc<Projection<C>>);

impl<C: LiveEdit> Clone for LiveContent<C> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<C: LiveEdit> PartialEq for LiveContent<C> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl<C: LiveEdit> Eq for LiveContent<C> {}

impl<C: LiveEdit + Clone + Default + 'static> LiveContent<C> {
    pub fn new(host: EditorHost, block: Option<uuid::Uuid>) -> Self {
        host.follow(block);
        Self(Rc::new(Projection {
            host,
            block,
            mirror: RefCell::new(EditorMirror::default()),
            told: Cell::new(false),
            watchers: Callbacks::default(),
            touched: RefCell::new(Vec::new()),
            announced: Cell::new(0),
            revisions: Callbacks::default(),
            lacking_revision: Cell::new(0),
            lacking_followed: Cell::new(false),
            lacking_changes: Callbacks::default(),
            ended: Callbacks::default(),
            due: RefCell::new(None),
            reads: RefCell::new(HashMap::default()),
        }))
    }

    pub fn content_type(&self) -> uuid::Uuid {
        C::CONTENT_TYPE
    }

    pub fn block(&self) -> Option<uuid::Uuid> {
        self.0.block
    }

    fn loaded(&self) -> bool {
        self.0.mirror.borrow().is_loaded()
    }

    pub fn record(&self) {
        self.0.mirror.borrow_mut().record();
    }

    pub fn stop_recording(&self) {
        self.0.mirror.borrow_mut().stop_recording();
    }

    pub fn take_projected(&self) -> Vec<Projected<C::Op>> {
        self.adopt_lazily();
        self.0.mirror.borrow_mut().take_projected()
    }

    pub fn on_due(&self, due: impl Fn() + 'static) {
        *self.0.due.borrow_mut() = Some(Rc::new(due));
    }

    pub fn ready(&self) -> Result<(), Unready> {
        self.adopt_lazily();
        let told = self.0.told.get() || self.0.due.borrow().is_none();
        if self.loaded() && told {
            return Ok(());
        }
        match self.0.mirror.borrow().failure() {
            Some(reason) if !self.loaded() => Err(Unready::Failed(reason.to_owned())),
            _ => Err(Unready::Loading),
        }
    }

    pub fn revision(&self) -> Result<u64, Unready> {
        self.ready().map(|()| self.0.mirror.borrow().revision())
    }

    pub fn read<T>(&self, read: impl FnOnce(&C) -> T) -> Result<T, Unready> {
        self.ready()
            .map(|()| read(self.0.mirror.borrow().visible()))
    }

    pub fn visible<T>(&self, read: impl FnOnce(&C) -> T) -> T {
        read(self.0.mirror.borrow().visible())
    }

    pub fn on_touched(&self, key: Touched, run: impl Fn(&C, &[Touched]) + 'static) -> Subscription {
        self.0.watchers.add(Some(key), Rc::new(run))
    }

    pub fn on_change(&self, run: impl Fn(&C, &[Touched]) + 'static) -> Subscription {
        self.0.watchers.add(None, Rc::new(run))
    }

    pub fn on_revision(&self, run: impl Fn(u64) + 'static) -> Subscription {
        self.0.revisions.add((), Rc::new(run))
    }

    /// The edits made on this device that the server lacks, each with whether the server
    /// refused it (`ToEditor::Lacking`): an editor shows what they made as still sending, or
    /// as not sent. Empty until the first answer to `follow_lacking`.
    pub fn lacking(&self) -> Vec<LackingEdit<C::Op>> {
        self.0.mirror.borrow().lacking()
    }

    /// Lets an edit the server refused go (`FromEditor::DismissLacking`): the editor sent it
    /// again, or the person gave it up.
    pub fn dismiss_lacking(&self, id: u64) {
        self.0
            .host
            .send_to_replica(self.0.block, FromEditor::DismissLacking(id));
    }

    /// Asks the replica to follow what the server lacks (`FromEditor::FollowLacking`), and
    /// again with each snapshot, which a new replica sends.
    pub fn follow_lacking(&self) {
        if !self.0.lacking_followed.replace(true) {
            self.0
                .host
                .send_to_replica(self.0.block, FromEditor::FollowLacking);
        }
    }

    pub fn on_lacking(&self, run: impl Fn() + 'static) -> Subscription {
        self.0.lacking_changes.add((), Rc::new(run))
    }

    /// The whole stored form's length, of which a type that loads only its first part
    /// (`be_block_content::Loads::FirstPart`) holds the start.
    pub fn stored_length(&self) -> Result<u64, Unready> {
        self.ready()
            .map(|()| self.0.mirror.borrow().stored_length())
    }

    /// How many snapshots the projection took (`EditorMirror::snapshots`).
    pub fn snapshots(&self) -> u64 {
        self.0.mirror.borrow().snapshots()
    }

    /// Reads a range of the block's stored form as its replica holds it, including what it
    /// has not saved: the future ends once the replica answers. The request is sent at once,
    /// whether or not the future is awaited. A type that loads only its first part reads the
    /// rest this way.
    pub fn read_range(
        &self,
        offset: u64,
        length: u64,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, String>> + use<C> {
        let (token, message) = self.0.mirror.borrow_mut().read_range(offset, length);
        let answer: Rc<RefCell<(Option<Result<Vec<u8>, String>>, Option<std::task::Waker>)>> =
            Rc::default();
        let answered = Rc::clone(&answer);
        self.0.reads.borrow_mut().insert(
            token,
            Box::new(move |read| {
                let mut answered = answered.borrow_mut();
                answered.0 = Some(read);
                if let Some(waker) = answered.1.take() {
                    waker.wake();
                }
            }),
        );
        self.0.host.send_to_replica(self.0.block, message);
        std::future::poll_fn(move |context| {
            let mut answer = answer.borrow_mut();
            match answer.0.take() {
                Some(read) => std::task::Poll::Ready(read),
                None => {
                    answer.1 = Some(context.waker().clone());
                    std::task::Poll::Pending
                }
            }
        })
    }

    /// Appends `bytes` to the block's signed log (a type with a log): the host signs them as
    /// this app's, for the person. Where it went is taken with `take_appended` under the token
    /// this answers; the entry joins `with_log` as it reaches the log, which moves the
    /// revision. What goes `along` with it is done once the server takes it.
    pub fn append(&self, bytes: Vec<u8>, along: be_block_content::Along) -> u64 {
        let (token, message) = self.0.mirror.borrow_mut().append(bytes, along);
        self.0.host.send_to_replica(self.0.block, message);
        token
    }

    /// Where the entries `append` asked for went, by token, since this was last asked: their
    /// place and arrival, none while one waits on the device, or why it was refused.
    pub fn take_appended(&self) -> Vec<(u64, Result<be_block_content::LogAppended, String>)> {
        self.0.mirror.borrow_mut().take_appended()
    }

    /// Asks for at most `limit` entries of the block's signed log from `from`, which join `log`
    /// as they arrive.
    pub fn read_log(&self, from: u64, limit: u32) {
        let (_, message) = self.0.mirror.borrow_mut().read_log(from, limit);
        self.0.host.send_to_replica(self.0.block, message);
    }

    /// Reads the entries of the block's signed log this editor holds, in the log's order, each
    /// id once at its first place: the latest it heard or read, up to `editor::LOG_HELD`, and
    /// every one from where it paged back to.
    pub fn with_log<R>(
        &self,
        read: impl FnOnce(&mut dyn Iterator<Item = &be_block_content::LogItem>) -> R,
    ) -> R {
        read(&mut self.0.mirror.borrow().log())
    }

    /// The log's length as this editor last heard it.
    pub fn log_size(&self) -> u64 {
        self.0.mirror.borrow().log_size()
    }

    /// Reads the entries this device appended that the server has not taken yet, in order:
    /// each id and the bytes the app appended (`EditorMirror::kept`).
    pub fn with_kept<R>(
        &self,
        read: impl FnOnce(&mut dyn Iterator<Item = (uuid::Uuid, &[u8])>) -> R,
    ) -> R {
        read(&mut self.0.mirror.borrow().kept())
    }

    /// Reads the entries not appended, by the token of the `append`: the bytes and why
    /// (`EditorMirror::refused`). Those the server refused the device keeps until dismissed, so
    /// they are here again when the editor opens again.
    pub fn with_refused<R>(
        &self,
        read: impl FnOnce(&mut dyn Iterator<Item = (u64, &[u8], &str)>) -> R,
    ) -> R {
        read(&mut self.0.mirror.borrow().refused())
    }

    /// Lets go of a refused entry, here and on the device: sent again as a new one, or given
    /// up.
    pub fn dismiss_refused(&self, token: u64) {
        let let_go = self.0.mirror.borrow_mut().dismiss_refused(token);
        if let Some(message) = let_go {
            self.0.host.send_to_replica(self.0.block, message);
        }
        self.announce();
    }

    /// Makes an edit outside any gesture.
    pub fn operate(&self, operation: C::Op) {
        if !self.0.host.editable() || !self.loaded() {
            return;
        }
        self.send_operation(None, operation);
    }

    /// Makes an edit in the open gesture `gesture`. One in a gesture the replica ended
    /// (`on_gesture_ended`) is dropped.
    pub fn operate_in(&self, gesture: ContentGesture, operation: C::Op) {
        if !self.0.host.editable() || !self.loaded() {
            return;
        }
        self.send_operation(Some(gesture.0), operation);
    }

    fn gesture(&self, id: u64, gesture: Gesture) {
        if !self.0.host.editable() || !self.loaded() {
            return;
        }
        let message = self.0.mirror.borrow_mut().gesture(id, gesture);
        if let Some(message) = message {
            self.0.host.send_to_replica(self.0.block, message);
        }
    }

    fn begin(&self, gesture: Gesture) -> ContentGesture {
        let id = self.0.host.next_gesture();
        self.gesture(id, gesture);
        ContentGesture(id)
    }

    /// Opens an undo group: the edits made in it (`operate_in`) are written as they happen
    /// and undo as one step.
    pub fn begin_gesture(&self) -> ContentGesture {
        self.begin(Gesture::Begin)
    }

    /// Opens a tentative gesture: the edits made in it (`operate_in`) show at once in every
    /// view of the block on this device, but are sent and saved only when it commits.
    /// Cancelling it, the editor closing or a crash leaves no trace of them. Its edits only
    /// move or change what exists; anything it creates is created on commit.
    pub fn begin_tentative(&self) -> ContentGesture {
        self.begin(Gesture::BeginTentative)
    }

    /// Ends `gesture`, keeping its edits.
    pub fn commit_gesture(&self, gesture: ContentGesture) {
        self.gesture(gesture.0, Gesture::Commit);
    }

    /// Puts back what `gesture` wrote and leaves no history. The host cancels the gestures
    /// still open when the instance closes.
    pub fn cancel_gesture(&self, gesture: ContentGesture) {
        self.gesture(gesture.0, Gesture::Cancel);
    }

    /// Whether `gesture` is open: begun, and ended neither here nor by the replica.
    pub fn gesture_open(&self, gesture: ContentGesture) -> bool {
        self.0.mirror.borrow().gesture_open(gesture.0)
    }

    /// Runs `run` for each of this editor's gestures the replica ended (`ToEditor::GestureEnded`):
    /// an undo cancelled the tentative layer, or what its edits moved was replaced. The gesture
    /// is cancelled already, and its later edits are dropped: the editor only stops it, as a
    /// drag lets go of what it carried.
    pub fn on_gesture_ended(&self, run: impl Fn(ContentGesture) + 'static) -> Subscription {
        self.0.ended.add((), Rc::new(run))
    }

    /// Ends the current undo step, so the next edit starts one of its own however soon it
    /// comes, without opening a gesture.
    pub fn break_step(&self) {
        if self.0.host.editable() && self.loaded() {
            self.0.host.send_to_replica(self.0.block, FromEditor::Break);
        }
    }

    /// Makes an edit outside any gesture even where the block is not editable here: a viewer's
    /// own view state, which a read-only editor still keeps.
    pub fn operate_anyway(&self, operation: C::Op) {
        self.send_operation(None, operation);
    }

    fn send_operation(&self, gesture: Option<u64>, operation: C::Op) {
        let sent = self.0.mirror.borrow_mut().operate(gesture, operation);
        let Some(sent) = sent else {
            return;
        };
        self.collect_touched();
        self.0.host.send_to_replica(self.0.block, sent);
        self.notify();
        self.announce();
    }

    pub fn pump(&self) {
        self.adopt();
        self.notify();
        if self.loaded() {
            self.0.told.set(true);
        }
        self.announce();
    }

    /// Whether anything `notify` or `announce` would tell is waiting: what was touched, a
    /// change to the lacking edits, or a revision not yet announced.
    fn unannounced(&self) -> bool {
        let mirror = self.0.mirror.borrow();
        !self.0.touched.borrow().is_empty()
            || self.0.lacking_revision.get() != mirror.lacking_revision()
            || self.0.announced.get() != mirror.revision()
    }

    fn announce(&self) {
        let lacking = self.0.mirror.borrow().lacking_revision();
        if self.0.lacking_revision.replace(lacking) != lacking {
            for run in self.0.lacking_changes.all() {
                run();
            }
        }
        let revision = self.0.mirror.borrow().revision();
        if self.0.announced.replace(revision) == revision {
            return;
        }
        for run in self.0.revisions.all() {
            run(revision);
        }
    }

    fn collect_touched(&self) {
        let touched = self.0.mirror.borrow_mut().take_touched();
        self.0.touched.borrow_mut().extend(touched);
    }

    fn notify(&self) {
        let touched = std::mem::take(&mut *self.0.touched.borrow_mut());
        if touched.is_empty() {
            return;
        }
        let runs = match touched.contains(&Touched::Everything) {
            true => self.0.watchers.all(),
            false => {
                let keys: HashSet<Option<Touched>> = std::iter::once(None)
                    .chain(touched.iter().copied().map(Some))
                    .collect();
                self.0.watchers.of(&keys)
            }
        };
        for run in runs {
            run(self.0.mirror.borrow().visible(), &touched);
        }
    }

    fn adopt_lazily(&self) {
        self.adopt();
        // What a read took is announced at the next pump, which only this asks for now: the
        // host lists for pumping only blocks whose updates are still waiting.
        if !self.unannounced() {
            return;
        }
        let due = self.0.due.borrow().clone();
        if let Some(due) = due {
            due();
        }
    }

    fn adopt(&self) {
        let snapshots = self.0.mirror.borrow().snapshots();
        for update in self.0.host.take_content_updates(self.0.block) {
            let answer = self.0.mirror.borrow_mut().receive(update);
            if let Some(answer) = answer {
                self.0.host.send_to_replica(self.0.block, answer);
            }
        }
        let ended = self.0.mirror.borrow_mut().take_ended();
        for id in ended {
            for run in self.0.ended.all() {
                run(ContentGesture(id));
            }
        }
        // A replica that sends a snapshot may be new, or may not have heard the first ask.
        if self.0.lacking_followed.get() && self.0.mirror.borrow().snapshots() != snapshots {
            self.0
                .host
                .send_to_replica(self.0.block, FromEditor::FollowLacking);
        }
        self.collect_touched();
        let ranges = self.0.mirror.borrow_mut().take_ranges();
        for (token, result) in ranges {
            let done = self.0.reads.borrow_mut().remove(&token);
            if let Some(done) = done {
                done(result);
            }
        }
    }
}

impl<T: Model> Objects<T> {
    pub fn reread<R: Root, M: 'static>(
        content: &Document<R>,
        owner: ObjectId,
        field: FieldRef<M, List<T>>,
        touched: &[Touched],
        held: &Self,
    ) -> Self
    where
        T: 'static,
    {
        let everything = touched.contains(&Touched::Everything);
        let changed: HashSet<ObjectId> = touched
            .iter()
            .filter_map(|touched| match touched {
                Touched::Subtree(id) => Some(*id),
                _ => None,
            })
            .collect();
        let kept: HashMap<ObjectId, Rc<T>> = match everything {
            true => HashMap::default(),
            false => held
                .iter()
                .filter(|(id, _)| !changed.contains(id))
                .map(|(id, object)| (*id, Rc::clone(object)))
                .collect(),
        };
        let objects = content
            .ids(owner, field)
            .into_iter()
            .filter_map(|id| match kept.get(&id) {
                Some(object) => Some((id, Rc::clone(object))),
                None => Some((id, Rc::new(content.read::<T>(id)?))),
            })
            .collect();
        Self(Rc::new(objects))
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
