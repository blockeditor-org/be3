use stable_hash::HashMap;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};

use block_plugin_api::EditorInstanceId;

use crate::{EditorHost, ViewHandler, Waker};

#[derive(Default)]
pub(crate) struct ModuleState {
    handler: RefCell<Option<Box<dyn ViewHandler>>>,
    locals: RefCell<HashMap<TypeId, Rc<dyn Any>>>,
    hosts: RefCell<HashMap<EditorInstanceId, EditorHost>>,
    wakers: Arc<Mutex<HashMap<EditorInstanceId, Waker>>>,
}

#[derive(Clone, Default)]
pub struct Module(Rc<ModuleState>);

impl Module {
    pub(crate) fn from_state(state: Rc<ModuleState>) -> Self {
        Self(state)
    }

    pub(crate) fn downgrade(&self) -> Weak<ModuleState> {
        Rc::downgrade(&self.0)
    }

    pub fn handles(&self) -> bool {
        self.0
            .handler
            .try_borrow()
            .map_or(true, |handler| handler.is_some())
    }

    pub fn handle(&self, handler: Box<dyn ViewHandler>) {
        *self.0.handler.borrow_mut() = Some(handler);
    }

    pub fn with_handler<R>(&self, act: impl FnOnce(&mut dyn ViewHandler) -> R) -> Option<R> {
        let mut handler = self.0.handler.borrow_mut();
        Some(act(handler.as_deref_mut()?))
    }

    pub fn take_handler(&self) -> Option<Box<dyn ViewHandler>> {
        self.0.handler.borrow_mut().take()
    }

    pub fn host(&self, instance: EditorInstanceId) -> Option<EditorHost> {
        self.0.hosts.borrow().get(&instance).cloned()
    }

    pub fn waker(&self) -> Waker {
        let wakers = Arc::clone(&self.0.wakers);
        let waker = Waker::default();
        waker.install(move || {
            let held: Vec<Waker> = wakers
                .lock()
                .unwrap_or_else(|held| held.into_inner())
                .values()
                .cloned()
                .collect();
            for waker in held {
                waker.wake();
            }
        });
        waker
    }

    pub(crate) fn enter(&self, instance: EditorInstanceId, host: &EditorHost) {
        host.join_module(self);
        self.0.hosts.borrow_mut().insert(instance, host.clone());
        self.wakers().insert(instance, host.waker());
    }

    pub(crate) fn leave(&self, instance: EditorInstanceId) {
        let left = self.0.hosts.borrow_mut().remove(&instance);
        drop(left);
        self.wakers().remove(&instance);
    }

    fn wakers(&self) -> std::sync::MutexGuard<'_, HashMap<EditorInstanceId, Waker>> {
        self.0
            .wakers
            .lock()
            .unwrap_or_else(|held| held.into_inner())
    }

    pub fn local<T: 'static>(&self, make: impl FnOnce() -> T) -> Rc<T> {
        let mut locals = self.0.locals.borrow_mut();
        let held = locals
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Rc::new(make()) as Rc<dyn Any>);
        Rc::clone(held)
            .downcast()
            .expect("a module local is keyed by its own type")
    }
}
