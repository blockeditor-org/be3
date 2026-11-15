//! Replies to the page's commands, as data: a command carries a `Reply`, which is only a
//! number, and the worker answers it with the answer's bytes. The page keeps what each number
//! calls back (`expect`, in its stack's `waiting`), and runs it with the decoded answer as it
//! takes the update that carries it (`take_updates`), so what it reads then has what the worker
//! did before answering.

use std::cell::Cell;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// What a reply calls back with its answer's bytes, or `None` for a reply dropped unanswered.
pub(crate) type Waiting = Box<dyn FnOnce(Option<&[u8]>) + Send>;

static NEXT: AtomicU64 = AtomicU64::new(1);

/// Where the answer to a command goes. Encoding it hands the waiting over to whoever decodes it;
/// dropping it unanswered on the worker drops what waits on it, as dropping a callback would.
pub(crate) struct Reply<T> {
    id: u64,
    /// Answered, or encoded: either way this copy no longer stands for the waiting.
    done: Cell<bool>,
    kind: PhantomData<fn(T)>,
}

/// A reply whose answer runs `reply` on the page. With no stack running, `reply` is dropped at
/// once, as the command it goes with is.
pub(crate) fn expect<T: DeserializeOwned + 'static>(
    reply: impl FnOnce(T) + Send + 'static,
) -> Reply<T> {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let waiting: Waiting = Box::new(move |answer| {
        let Some(answer) = answer else {
            return;
        };
        // The worker encoded it from a `T`: one that does not decode is a bug.
        reply(postcard::from_bytes(answer).expect("an answer decodes as what it answers"));
    });
    crate::with_stack(|stack| stack.waiting.insert(id, waiting));
    Reply {
        id,
        done: Cell::new(false),
        kind: PhantomData,
    }
}

impl<T: Serialize> Reply<T> {
    pub(crate) fn send(self, answer: T) {
        self.done.set(true);
        let bytes = postcard::to_stdvec(&answer).expect("an answer encodes");
        deliver(self.id, Some(bytes));
    }
}

impl<T> Drop for Reply<T> {
    fn drop(&mut self) {
        if !self.done.get() {
            deliver(self.id, None);
        }
    }
}

/// On the worker, holds the answer to `id` for the next update. Elsewhere only a test answers
/// (the worker's own copy of a reply is the one it decodes), so the answer is told at once; a
/// reply the page drops before sending its command leaves its waiting for the stack to drop
/// as it stops.
fn deliver(id: u64, answer: Option<Vec<u8>>) {
    let Some(answer) = crate::seen::hold_answer(id, answer) else {
        return;
    };
    if answer.is_none() {
        return;
    }
    if let Some(waiting) = crate::with_stack(|stack| stack.waiting.remove(&id)).flatten() {
        waiting(answer.as_deref());
    }
}

impl<T> Serialize for Reply<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.done.set(true);
        self.id.serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for Reply<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self {
            id: u64::deserialize(deserializer)?,
            done: Cell::new(false),
            kind: PhantomData,
        })
    }
}

/// A push opened (`be_block_client::OpenedNotification`), as an answer.
#[derive(Deserialize, Serialize)]
pub(crate) struct Opened(
    #[serde(with = "OpenedNotification")] pub be_block_client::OpenedNotification,
);

#[derive(Deserialize, Serialize)]
#[serde(remote = "be_block_client::OpenedNotification")]
struct OpenedNotification {
    organization: uuid::Uuid,
    block: uuid::Uuid,
    sender: uuid::Uuid,
    sent: i64,
    text: String,
}

/// Who signed bytes (`be_block_client::Signer`), as an answer.
#[derive(Deserialize, Serialize)]
pub(crate) struct Signed(#[serde(with = "Signer")] pub be_block_client::Signer);

#[derive(Deserialize, Serialize)]
#[serde(remote = "be_block_client::Signer")]
enum Signer {
    Verified,
    Unverified,
    Changed,
    Bad,
}
