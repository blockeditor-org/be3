//! Running reads at once without an executor of our own: `all` polls every
//! future each time it is woken, so a caller whose reads are round trips
//! waits for them together.

use std::{
    future::{Future, poll_fn},
    pin::Pin,
    task::Poll,
};

pub(crate) async fn all<F: Future>(futures: impl IntoIterator<Item = F>) -> Vec<F::Output> {
    let mut futures: Vec<Pin<Box<F>>> = futures.into_iter().map(Box::pin).collect();
    let mut out: Vec<Option<F::Output>> = futures.iter().map(|_| None).collect();
    poll_fn(move |context| {
        let mut pending = false;
        for (future, slot) in futures.iter_mut().zip(out.iter_mut()) {
            if slot.is_none() {
                match future.as_mut().poll(context) {
                    Poll::Ready(value) => *slot = Some(value),
                    Poll::Pending => pending = true,
                }
            }
        }
        if pending {
            Poll::Pending
        } else {
            Poll::Ready(out.iter_mut().map(|slot| slot.take().unwrap()).collect())
        }
    })
    .await
}

/// `all` over fallible futures, failing with the first error.
pub(crate) async fn try_all<T, E, F: Future<Output = Result<T, E>>>(futures: impl IntoIterator<Item = F>) -> Result<Vec<T>, E> {
    all(futures).await.into_iter().collect()
}
