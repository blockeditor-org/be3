//! Catching a panic: the one container every guest thread, and host work done for one guest or
//! task, runs in (`contain`), whose depth a process's panic hook reads (`containing`) to tell a
//! panic that was caught and reported from one that crashes the process, and the message a panic
//! carries.

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, Once, PoisonError};

thread_local! {
    static CONTAINING: Cell<u32> = const { Cell::new(0) };
}

/// Whether `set_hook` put a process's hook in place, which `contain` then leaves as it is. Held
/// while either sets a hook, so the two cannot interleave.
static HOOKED: Mutex<bool> = Mutex::new(false);
static QUIETED: Once = Once::new();

/// Runs `run`, and catches a panic in it as its message. While it runs, the panic hook hears
/// a panic as contained (`containing`).
pub fn contain<R>(run: impl FnOnce() -> R) -> Result<R, String> {
    quiet_contained_panics();
    CONTAINING.with(|depth| depth.set(depth.get() + 1));
    let ran = catch_unwind(AssertUnwindSafe(run));
    CONTAINING.with(|depth| depth.set(depth.get() - 1));
    ran.map_err(|payload| message(&*payload))
}

/// Polls `task` inside `contain`, so a panic in any poll ends it with the panic's message.
pub async fn contain_task(task: impl std::future::Future<Output = ()>) -> Result<(), String> {
    let mut task = std::pin::pin!(task);
    std::future::poll_fn(|context| match contain(|| task.as_mut().poll(context)) {
        Ok(polled) => polled.map(Ok),
        Err(panic) => std::task::Poll::Ready(Err(panic)),
    })
    .await
}

/// Whether this thread is inside `contain`, so a panic now is caught and reported by its caller.
pub fn containing() -> bool {
    CONTAINING.with(Cell::get) > 0
}

/// Puts a process's panic hook in place, which hears every panic, contained or not.
pub fn set_hook(hook: Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>) {
    let mut hooked = HOOKED.lock().unwrap_or_else(PoisonError::into_inner);
    *hooked = true;
    std::panic::set_hook(hook);
}

/// The message a panic's payload carries.
pub fn message(payload: &(dyn std::any::Any + Send)) -> String {
    match (
        payload.downcast_ref::<&str>(),
        payload.downcast_ref::<String>(),
    ) {
        (Some(message), _) => (*message).to_owned(),
        (None, Some(message)) => message.clone(),
        (None, None) => "a panic without a message".to_owned(),
    }
}

/// Where no hook was set through `set_hook` (a test's process), a contained panic is said in a
/// line rather than through the hook before it, which with RUST_BACKTRACE set symbolizes a
/// backtrace of the whole binary: of a debug test harness, most of a second of CPU and hundreds
/// of MB, for a panic the caller expects. Every other panic still goes to that hook.
fn quiet_contained_panics() {
    // A hook cannot be set while this thread unwinds; a later `contain` sets it.
    if std::thread::panicking() {
        return;
    }
    QUIETED.call_once(|| {
        let hooked = HOOKED.lock().unwrap_or_else(PoisonError::into_inner);
        if *hooked {
            return;
        }
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !containing() {
                return previous(info);
            }
            let thread = std::thread::current();
            let name = thread.name().unwrap_or("unnamed");
            eprintln!("thread '{name}' {info}\n(the panic was contained)");
        }));
    });
}
