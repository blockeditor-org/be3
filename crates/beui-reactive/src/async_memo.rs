use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use crate::computation::{Effect, create_binding};
use crate::stored::StoredCell;
use crate::loading::{ErrorBoundary, Pending, Ready};
use crate::runtime::check_tracked_read;
use crate::scope::on_cleanup;
use crate::signal::{Get, Set, Signal, create_signal};
use crate::source::{Source, create_source};
use crate::trigger::{Trigger, create_trigger};

/// A future the host's executor runs for an async memo.
pub type LocalFuture = Pin<Box<dyn Future<Output = ()>>>;

/// Hands a future to an executor, or back if that executor is gone.
type Spawner = dyn Fn(LocalFuture) -> Result<(), LocalFuture>;

thread_local! {
    static SPAWNER: RefCell<Option<Rc<Spawner>>> = const { RefCell::new(None) };
    /// Futures spawned while no spawner could take them, handed to the next installed.
    static UNSPAWNED: RefCell<Vec<LocalFuture>> = const { RefCell::new(Vec::new()) };
}

/// Installs what runs the futures of async memos on this thread, as `wake_with` installs the
/// waker: beui-reactive has no executor of its own. The host's app framework installs one
/// (be-sdk-beui runs them on be-sdk's executor). A future it is handed may finish without
/// being polled again once its memo drops it: cancelling is done here, not by the executor.
/// `spawn` hands a future back once its executor is gone (a program's runner that stopped),
/// and futures spawned while no spawner takes them wait for the next one installed.
pub fn spawn_with(spawn: impl Fn(LocalFuture) -> Result<(), LocalFuture> + 'static) {
    SPAWNER.with(|spawner| spawner.replace(Some(Rc::new(spawn))));
    for future in UNSPAWNED.with(|unspawned| std::mem::take(&mut *unspawned.borrow_mut())) {
        hand_over(future);
    }
}

fn hand_over(future: LocalFuture) {
    let spawner = SPAWNER.with(|spawner| spawner.borrow().clone());
    let unspawned = match spawner {
        Some(spawner) => spawner(future).err(),
        None => Some(future),
    };
    if let Some(future) = unspawned {
        UNSPAWNED.with(|unspawned| unspawned.borrow_mut().push(future));
    }
}

/// A future its async memo can drop before it finishes.
struct Running {
    future: RefCell<Option<LocalFuture>>,
    waker: RefCell<Option<Waker>>,
    /// Cancelled while it was being polled: the poll drops it once it returns.
    cancelled: Cell<bool>,
}

impl Running {
    fn cancel(&self) {
        self.cancelled.set(true);
        if let Ok(mut future) = self.future.try_borrow_mut() {
            drop(future.take());
        }
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }
    }
}

fn spawn(future: impl Future<Output = ()> + 'static) -> Rc<Running> {
    let running = Rc::new(Running {
        future: RefCell::new(Some(Box::pin(future))),
        waker: RefCell::new(None),
        cancelled: Cell::new(false),
    });
    let polled = Rc::clone(&running);
    hand_over(Box::pin(std::future::poll_fn(move |context: &mut Context<'_>| {
        let mut held = polled.future.borrow_mut();
        let Some(future) = held.as_mut() else {
            return Poll::Ready(());
        };
        match future.as_mut().poll(context) {
            Poll::Ready(()) => {
                *held = None;
                Poll::Ready(())
            }
            Poll::Pending if polled.cancelled.get() => {
                *held = None;
                Poll::Ready(())
            }
            Poll::Pending => {
                *polled.waker.borrow_mut() = Some(context.waker().clone());
                Poll::Pending
            }
        }
    })));
    running
}

/// A value computed by a future: `Err(Pending::Loading)` until the first answer, then the last
/// good answer while a newer one is on its way (`pending()` says so), or `Err(Pending::Failed)`
/// if the latest failed. `Copy`, like every handle.
pub struct AsyncMemo<T: 'static> {
    value: Source<T>,
    pending: Signal<bool>,
    rerun: Trigger,
}

impl<T> Clone for AsyncMemo<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for AsyncMemo<T> {}

impl<T> PartialEq for AsyncMemo<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> Eq for AsyncMemo<T> {}

impl<T> std::fmt::Debug for AsyncMemo<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AsyncMemo").field(&self.value).finish()
    }
}

/// An async memo, like SolidJS 2.0's: `source` runs tracked, like a memo, and returns the
/// future that computes the value; when what it read changes it runs again, and the future
/// it returned before is dropped, which cancels it. It runs at once (a binding), so it starts
/// inside a hidden `Loading` too.
///
/// The future runs on the spawner the host installed (`spawn_with`); it ends with a `Ready<T>`,
/// so `?` on another `Ready` inside it fails or waits the memo with it. Its readiness is part
/// of the graph, as a `Source`'s is. An `Errored` boundary around where it is made runs it
/// again on reset.
pub fn create_async_memo<T, F>(mut source: impl FnMut() -> F + 'static) -> AsyncMemo<T>
where
    T: PartialEq + 'static,
    F: Future<Output = Ready<T>> + 'static,
{
    let value = create_source();
    let pending = create_signal(true);
    let rerun = create_trigger();
    // An answer that lands after what the source read changed, but before it runs again and
    // drops the future, is for what it read before: it is dropped too.
    let binding = StoredCell::new(None::<Effect>);
    let made = create_binding(move || {
        rerun.track();
        let future = source();
        pending.set(true);
        let running = spawn(async move {
            let answer = future.await;
            if binding.get().is_some_and(|binding| binding.is_due()) {
                return;
            }
            value.set(answer);
            pending.set(false);
        });
        on_cleanup(move || running.cancel());
    });
    binding.set(Some(made));
    let memo = AsyncMemo {
        value,
        pending,
        rerun,
    };
    value.with_node(|node| ErrorBoundary::hold_refresh(node, std::rc::Rc::new(move || memo.refresh())));
    memo
}

impl<T: 'static> AsyncMemo<T> {
    /// Whether a future is on its way: the first, or a newer one while the last answer shows.
    #[track_caller]
    pub fn pending(&self) -> bool {
        check_tracked_read::<Self>();
        self.pending.get()
    }

    /// Runs `source` again, dropping the future in flight: what an `Errored` boundary's reset
    /// does to the async memos under it. A failure it showed reads as loading again.
    pub fn refresh(&self)
    where
        T: PartialEq,
    {
        if self
            .value
            .with_untracked(|value| matches!(value, Err(Pending::Failed(_))))
        {
            self.value.set(Err(Pending::Loading));
        }
        self.rerun.notify();
    }

    pub fn is_alive(&self) -> bool {
        self.value.is_alive()
    }

    pub(crate) fn source(&self) -> Source<T> {
        self.value
    }
}

impl<T: 'static> Get<Ready<T>> for AsyncMemo<T> {
    #[track_caller]
    fn with<R>(&self, f: impl FnOnce(&Ready<T>) -> R) -> R {
        check_tracked_read::<Self>();
        self.value.with(f)
    }

    fn with_untracked<R>(&self, f: impl FnOnce(&Ready<T>) -> R) -> R {
        self.value.with_untracked(f)
    }
}
