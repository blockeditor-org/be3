//! Replies to the page's commands, as data: a command carries a `Reply`, which is only a
//! number, and the worker answers it with the answer's bytes. The page keeps what each number
//! calls back (`expect`) and runs it with the decoded answer.
//!
//! While the page and the worker share a memory, the table of what each number calls back is
//! one both reach, and the worker runs the call once the `Seen` with what it did before it is
//! handed over (`seen::after_publish`). Once the worker has a memory of its own
//! (plans/web_page_never_waits.md), the answers travel to the page in the message that hands it
//! that `Seen`, and the table is the page's alone.

use std::cell::Cell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

type Waiting = Box<dyn FnOnce(Option<&[u8]>) + Send>;

static NEXT: AtomicU64 = AtomicU64::new(1);

static WAITING: Mutex<Option<HashMap<u64, Waiting>>> = Mutex::new(None);

/// Where the answer to a command goes. Encoding it hands the waiting over to whoever decodes it;
/// dropping it unanswered drops what waits on it, as dropping a callback would.
pub(crate) struct Reply<T> {
    id: u64,
    /// Answered, or encoded: either way this copy no longer stands for the waiting.
    done: Cell<bool>,
    kind: PhantomData<fn(T)>,
}

/// A reply whose answer runs `reply` on the page.
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
    WAITING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_default()
        .insert(id, waiting);
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

/// Runs what waits on `id` with `answer`, or drops it for none, once the worker's `Seen` with
/// what it did before is handed over.
fn deliver(id: u64, answer: Option<Vec<u8>>) {
    crate::seen::after_publish(move |()| {
        let waiting = WAITING
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
            .and_then(|waiting| waiting.remove(&id));
        if let Some(waiting) = waiting {
            waiting(answer.as_deref());
        }
    })(());
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
