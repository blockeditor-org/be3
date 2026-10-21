use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::runtime::in_memo;
use crate::{
    Get, Memo, Set, Signal, Stored, create_memo, create_signal, on_cleanup, provide_context,
    store, use_context,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pending {
    Loading,
    Failed(String),
}

pub type Ready<T> = Result<T, Pending>;

type Probe = Rc<dyn Fn() -> Ready<()>>;

#[derive(Default)]
struct Waits {
    probes: RefCell<Vec<(u64, Probe)>>,
    next: Cell<u64>,
    revealed: Cell<bool>,
}

#[derive(Clone, Copy)]
pub struct Boundary {
    changed: Signal<u64>,
    waits: Stored<Waits>,
    state: Memo<Ready<()>>,
}

impl Boundary {
    pub fn provide() -> Self {
        let changed = create_signal(0u64);
        let waits = store(Waits::default());
        let state = create_memo(move || {
            changed.get();
            let probes: Vec<Probe> = waits.with(|waits| {
                waits
                    .probes
                    .borrow()
                    .iter()
                    .map(|(_, probe)| Rc::clone(probe))
                    .collect()
            });
            settle(probes.iter().map(|probe| probe()))
        });
        let boundary = Self {
            changed,
            waits,
            state,
        };
        provide_context(boundary);
        boundary
    }

    pub fn current() -> Option<Self> {
        if in_memo() {
            return None;
        }
        use_context::<Self>().filter(|boundary| {
            boundary
                .waits
                .try_with(|waits| !waits.revealed.get())
                .unwrap_or(false)
        })
    }

    pub fn reveal(&self) {
        let revealed = self.waits.try_with(|waits| {
            waits.revealed.set(true);
            waits.probes.borrow_mut().clear();
        });
        if revealed.is_some() {
            self.changed.update(|changed| *changed += 1);
        }
    }

    pub fn state(&self) -> Ready<()> {
        self.state.get()
    }

    fn add(self, probe: Probe) {
        let id = self.waits.with(|waits| {
            let id = waits.next.get();
            waits.next.set(id + 1);
            waits.probes.borrow_mut().push((id, probe));
            id
        });
        self.changed.update(|changed| *changed += 1);
        on_cleanup(move || {
            let removed = self.waits.try_with(|waits| {
                waits.probes.borrow_mut().retain(|(held, _)| *held != id);
            });
            if removed.is_some() {
                self.changed.update(|changed| *changed += 1);
            }
        });
    }
}

fn settle(states: impl Iterator<Item = Ready<()>>) -> Ready<()> {
    let mut settled = Ok(());
    for state in states {
        match state {
            Ok(()) => {}
            Err(Pending::Failed(error)) => return Err(Pending::Failed(error)),
            Err(Pending::Loading) => settled = Err(Pending::Loading),
        }
    }
    settled
}

pub fn wait_for(probe: impl Fn() -> Ready<()> + 'static) {
    if let Some(boundary) = Boundary::current() {
        boundary.add(Rc::new(probe));
    }
}

pub fn hold<T, S>(source: S) -> Memo<T>
where
    T: Clone + PartialEq + Default + 'static,
    S: Get<Ready<T>> + Clone + 'static,
{
    let state = create_memo({
        let source = source.clone();
        move || source.with(|ready| ready.as_ref().map(|_| ()).map_err(Clone::clone))
    });
    wait_for(move || state.get());
    let mut last = T::default();
    create_memo(move || {
        source.with(|ready| {
            if let Ok(value) = ready {
                last.clone_from(value);
            }
        });
        last.clone()
    })
}
