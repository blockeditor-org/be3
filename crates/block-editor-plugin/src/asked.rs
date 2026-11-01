use stable_hash::HashMap;
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

use block_plugin_api::{
    EditorInstanceId, Failure, FilePick, FileSave, Watched,
    ops::{
        OpError, OpId,
        guest::{self, Pending},
    },
};

thread_local! {
    static EDITORS: RefCell<HashMap<EditorInstanceId, OpId>> = RefCell::new(HashMap::default());
}

pub(crate) fn opened(instance: EditorInstanceId, editor: OpId) {
    EDITORS.with(|editors| editors.borrow_mut().insert(instance, editor));
}

pub(crate) fn closed(instance: EditorInstanceId) {
    EDITORS.with(|editors| editors.borrow_mut().remove(&instance));
}

pub(crate) fn follow<W: Watched>(
    instance: Option<EditorInstanceId>,
    mut each: impl FnMut(W::Value) + 'static,
) {
    guest::spawn(async move {
        let watch = asking(instance, W::open);
        while let Ok(value) = watch.next().await {
            each(value);
        }
    });
}

pub(crate) fn asking<R>(instance: Option<EditorInstanceId>, ask: impl FnOnce() -> R) -> R {
    let editor = instance
        .and_then(|instance| EDITORS.with(|editors| editors.borrow().get(&instance).copied()));
    match editor {
        Some(editor) => guest::with_parent(editor, ask),
        None => ask(),
    }
}

pub(crate) trait Refused {
    fn refused(error: OpError) -> Self;
}

impl<T> Refused for Result<T, Failure> {
    fn refused(error: OpError) -> Self {
        Err(Failure::from(error))
    }
}

impl Refused for FilePick {
    fn refused(error: OpError) -> Self {
        Err(Failure::from(error).to_string())
    }
}

impl Refused for FileSave {
    fn refused(error: OpError) -> Self {
        FileSave::Failed(Failure::from(error).to_string())
    }
}

pub struct Asked<T>(Pin<Box<dyn Future<Output = Option<T>>>>);

impl<T: 'static> Asked<T> {
    pub(crate) fn of<E: 'static>(pending: Pending<T, E>) -> Self
    where
        T: Refused,
        Pending<T, E>: Future<Output = Result<T, OpError>>,
    {
        Self(Box::pin(async move {
            match pending.await {
                Ok(answer) => Some(answer),
                Err(OpError::Canceled) => None,
                Err(error) => Some(T::refused(error)),
            }
        }))
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U + 'static) -> Asked<U> {
        Asked(Box::pin(async move { self.await.map(map) }))
    }
}

impl<T> Future for Asked<T> {
    type Output = Option<T>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<T>> {
        self.0.as_mut().poll(context)
    }
}

#[derive(Default)]
struct TaskState {
    canceled: Cell<bool>,
    finished: Cell<bool>,
    waker: RefCell<Option<Waker>>,
}

#[derive(Clone)]
pub struct Task(Rc<TaskState>);

impl Task {
    pub fn cancel(&self) {
        self.0.canceled.set(true);
        if let Some(waker) = self.0.waker.borrow_mut().take() {
            waker.wake();
        }
    }

    pub fn is_finished(&self) -> bool {
        self.0.finished.get() || self.0.canceled.get()
    }
}

pub(crate) fn spawn(future: impl Future<Output = ()> + 'static) -> Task {
    let state = Rc::new(TaskState::default());
    let task = Task(Rc::clone(&state));
    let mut future = Box::pin(future);
    guest::spawn(std::future::poll_fn(move |context| {
        if state.canceled.get() {
            return Poll::Ready(());
        }
        match future.as_mut().poll(context) {
            Poll::Ready(()) => {
                state.finished.set(true);
                Poll::Ready(())
            }
            Poll::Pending => {
                *state.waker.borrow_mut() = Some(context.waker().clone());
                Poll::Pending
            }
        }
    }));
    task
}
