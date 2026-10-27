use std::cell::RefCell;

use crate::core::tree::set_component_state;
use beui_tree::reactive::{Stored, store};

pub struct ComponentRef<A: 'static>(Stored<RefCell<Option<A>>>);

impl<A: 'static> Clone for ComponentRef<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: 'static> Copy for ComponentRef<A> {}

impl<A: Clone + 'static> Default for ComponentRef<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Clone + 'static> ComponentRef<A> {
    pub fn new() -> Self {
        Self(store(RefCell::new(None)))
    }

    pub fn fill(&self, api: A) {
        if let Some(cell) = self.0.try_value() {
            *cell.borrow_mut() = Some(api.clone());
        }
        set_component_state(api);
    }

    pub fn get(&self) -> A {
        self.try_get()
            .expect("component ref read before the component it points at was built")
    }

    pub fn try_get(&self) -> Option<A> {
        self.0.try_with(|cell| cell.borrow().clone()).flatten()
    }
}
