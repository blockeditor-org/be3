use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::runtime::in_memo;
use crate::{
    Get, Memo, Set, Signal, Stored, create_memo, create_signal, on_cleanup, provide_context, store,
    use_context,
};

/// Why a `Ready` value is not there yet. Data that may not have arrived is a `Ready<T>`, so
/// a memo derives from it with `?` and no `Option` or `loaded` flag sits beside it
/// (decisions/loading_state_is_ready_and_waits_at_a_boundary.md).
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

/// What a `Loading` component provides: it collects the waits registered under it and
/// reveals its content once none is pending, after which nothing registers with it again.
/// An inner boundary keeps its content's waits from the outer one.
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

    fn add(self, probe: Probe) -> u64 {
        let id = self.waits.with(|waits| {
            let id = waits.next.get();
            waits.next.set(id + 1);
            waits.probes.borrow_mut().push((id, probe));
            id
        });
        self.changed.update(|changed| *changed += 1);
        id
    }

    fn add_scoped(self, probe: Probe) {
        let id = self.add(probe);
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

/// Makes the nearest boundary wait while `probe` is pending. The wait lasts as long as the
/// current scope, so a source a binding caches across reruns is made in a scope kept with
/// it, not in the binding's own, which the next run drops.
pub fn wait_for(probe: impl Fn() -> Ready<()> + 'static) {
    if let Some(boundary) = Boundary::current() {
        boundary.add_scoped(Rc::new(probe));
    }
}

pub fn wait_while<A: ?Sized + 'static>(alive: Weak<A>, probe: impl Fn() -> Ready<()> + 'static) {
    if let Some(boundary) = Boundary::current() {
        boundary.add(Rc::new(move || match alive.upgrade() {
            Some(_) => probe(),
            None => Ok(()),
        }));
    }
}

/// The last `T` the source was (`T::default()` before the first), waiting on the nearest
/// boundary until it arrives. A `Memo<Ready<T>>` handed to a `Prop<T>` is held this way.
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
