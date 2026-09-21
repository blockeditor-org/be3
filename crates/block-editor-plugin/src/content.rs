use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

use be_block::LiveEdit;
use beui::reactive::{ReadSignal, batch, create_signal};

use crate::EditorHost;

type Projection<C> = Rc<dyn Fn(&C)>;

pub struct ContentProjection<C: LiveEdit> {
    host: EditorHost,
    confirmed: RefCell<C>,
    visible: RefCell<C>,
    pending: RefCell<VecDeque<C::Op>>,
    acknowledged: Cell<u64>,
    seen: Cell<u64>,
    projections: RefCell<Vec<Projection<C>>>,
    dirty: Cell<bool>,
}

impl<C: LiveEdit + Clone + Default> ContentProjection<C> {
    pub(crate) fn new(host: EditorHost) -> Self {
        Self {
            host,
            confirmed: RefCell::new(C::default()),
            visible: RefCell::new(C::default()),
            pending: RefCell::new(VecDeque::new()),
            acknowledged: Cell::new(0),
            seen: Cell::new(0),
            projections: RefCell::new(Vec::new()),
            dirty: Cell::new(false),
        }
    }

    pub fn content_type(&self) -> uuid::Uuid {
        C::CONTENT_TYPE
    }

    pub fn project<T>(&self, project: impl Fn(&C) -> T + 'static) -> ReadSignal<T>
    where
        T: Clone + PartialEq + 'static,
    {
        let (read, write) = create_signal(project(&self.visible.borrow()));
        self.projections
            .borrow_mut()
            .push(Rc::new(move |content| write.set(project(content))));
        read
    }

    pub fn operate(&self, operation: C::Op) {
        if !self.host.editable() {
            return;
        }
        self.visible.borrow_mut().apply(&operation);
        self.host.operate_content(C::encode_operation(&operation));
        self.pending.borrow_mut().push_back(operation);
        self.dirty.set(true);
    }

    pub(crate) fn pump(&self) {
        self.adopt();
        if !self.dirty.replace(false) {
            return;
        }
        let projections = self.projections.borrow().clone();
        let visible = self.visible.borrow();
        batch(|| {
            for projection in projections {
                projection(&visible);
            }
        });
    }

    fn adopt(&self) {
        let Some(content) = self.host.content() else {
            return;
        };
        if content.revision == self.seen.get() || content.content_type != C::CONTENT_TYPE {
            return;
        }
        self.seen.set(content.revision);
        let Ok(confirmed) = C::decode(&content.bytes) else {
            return;
        };
        let taken = content.applied.saturating_sub(self.acknowledged.get());
        self.acknowledged.set(content.applied);
        let mut pending = self.pending.borrow_mut();
        for _ in 0..taken.min(pending.len() as u64) {
            pending.pop_front();
        }
        let mut visible = confirmed.clone();
        for operation in pending.iter() {
            visible.apply(operation);
        }
        *self.confirmed.borrow_mut() = confirmed;
        *self.visible.borrow_mut() = visible;
        self.dirty.set(true);
    }
}
