use stable_hash::{HashMap, HashSet};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use be_block::be_model::{Document, FieldRef, List, Model};
use be_block::{LiveEdit, ObjectId, Root, Touched};
use block_type_guest::editor::{EditorMirror, FromEditor, Gesture};

use crate::EditorHost;
use crate::state::{Callbacks, Subscription, Unready};

type Run<C> = dyn Fn(&C, &[Touched]);

pub use block_type_guest::editor::Projected;

struct Projection<C: LiveEdit> {
    host: EditorHost,
    block: Option<uuid::Uuid>,
    mirror: RefCell<EditorMirror<C>>,
    told: Cell<bool>,
    watchers: Callbacks<Option<Touched>, Run<C>>,
    touched: RefCell<Vec<Touched>>,
    announced: Cell<u64>,
    revisions: Callbacks<(), dyn Fn(u64)>,
    due: RefCell<Option<Rc<dyn Fn()>>>,
}

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
            due: RefCell::new(None),
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
            Ok(())
        } else {
            Err(Unready::Loading)
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

    pub fn operate(&self, operation: C::Op) {
        if !self.0.host.editable() || !self.loaded() {
            return;
        }
        self.operate_anyway(operation);
    }

    fn gesture(&self, gesture: Gesture) {
        if self.0.host.editable() && self.loaded() {
            self.0
                .host
                .send_to_replica(self.0.block, FromEditor::Gesture(gesture));
        }
    }

    pub fn begin_gesture(&self) {
        self.gesture(Gesture::Begin);
    }

    pub fn commit_gesture(&self) {
        self.gesture(Gesture::Commit);
    }

    pub fn cancel_gesture(&self) {
        self.gesture(Gesture::Cancel);
    }

    pub fn operate_anyway(&self, operation: C::Op) {
        let sent = self.0.mirror.borrow_mut().operate(operation);
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

    fn announce(&self) {
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
        if self.0.touched.borrow().is_empty() {
            return;
        }
        let due = self.0.due.borrow().clone();
        if let Some(due) = due {
            due();
        }
    }

    fn adopt(&self) {
        for update in self.0.host.take_content_updates(self.0.block) {
            let answer = self.0.mirror.borrow_mut().receive(update);
            if let Some(answer) = answer {
                self.0.host.send_to_replica(self.0.block, answer);
            }
        }
        self.collect_touched();
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
