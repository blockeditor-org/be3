use super::executor::{Answer, install};
use super::*;
use crate::{Reader, create_async_memo};

/// `get`, `pending` and a `Reader` each pull an async memo whose input changed, and only the
/// first of them asks again.
#[test]
fn reading_an_async_memo_and_its_pending_asks_once() {
    let _root = RootScope::new();
    install();
    let id = create_signal(1_u32);
    let asked = Rc::new(Cell::new(0));
    let counted = asked.clone();
    let memo = create_async_memo(move || {
        let _ = id.get();
        counted.set(counted.get() + 1);
        let answer = Answer::<u32>::default();
        async move { Ok(answer.await) }
    });
    let reader = Reader::from(memo);
    assert_eq!(asked.get(), 1);

    id.set(2);
    let _ = memo.get_untracked();
    assert!(untrack(|| memo.pending()));
    assert_eq!(asked.get(), 2, "pending after get asks no more");

    id.set(3);
    let _ = reader.get_untracked();
    assert_eq!(asked.get(), 3, "a Reader pulls too");
    flush();
    assert_eq!(asked.get(), 3);
}
