use std::collections::HashMap;

type Handle = u32;

pub(crate) struct Counter {
    next: Handle,
}

impl Counter {
    pub(crate) fn new() -> Self {
        Self { next: 1 }
    }

    pub(crate) fn take(&mut self) -> Handle {
        let handle = self.next;
        self.next = self.next.wrapping_add(1).max(1);
        handle
    }
}

pub(crate) struct Table<T> {
    entries: HashMap<Handle, T>,
    counter: Counter,
}

impl<T> Table<T> {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            counter: Counter::new(),
        }
    }

    pub(crate) fn insert(&mut self, value: T) -> Handle {
        let handle = self.counter.take();
        self.entries.insert(handle, value);
        handle
    }

    /// Puts back a value `take` took, under the handle it had.
    pub(crate) fn put(&mut self, handle: Handle, value: T) {
        self.entries.insert(handle, value);
    }

    pub(crate) fn skip(&mut self) -> Handle {
        self.counter.take()
    }

    pub(crate) fn handles(&self) -> impl Iterator<Item = Handle> + use<T> {
        let mut handles: Vec<Handle> = self.entries.keys().copied().collect();
        handles.sort_unstable();
        handles.into_iter()
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &T> {
        self.entries.values()
    }

    pub(crate) fn get(&self, handle: Handle, kind: &str) -> Result<&T, String> {
        self.entries
            .get(&handle)
            .ok_or_else(|| format!("no {kind} is registered as handle {handle}"))
    }

    pub(crate) fn get_mut(&mut self, handle: Handle, kind: &str) -> Result<&mut T, String> {
        self.entries
            .get_mut(&handle)
            .ok_or_else(|| format!("no {kind} is registered as handle {handle}"))
    }

    pub(crate) fn take(&mut self, handle: Handle, kind: &str) -> Result<T, String> {
        self.entries
            .remove(&handle)
            .ok_or_else(|| format!("no {kind} is registered as handle {handle}"))
    }

    pub(crate) fn remove(&mut self, handle: Handle) {
        self.entries.remove(&handle);
    }
}

impl<T> Default for Table<T> {
    fn default() -> Self {
        Self::new()
    }
}
