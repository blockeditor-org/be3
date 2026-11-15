use super::executor::install;
use super::*;
use crate::create_async_memo;

/// Several writes before a flush run the async memo once, with the last value; the answer it
/// lands at once is for that value, and a later write leaves it due until it runs again.
#[test]
fn an_answer_there_at_once_is_for_the_latest_write() {
    let _root = RootScope::new();
    install();
    let id = create_signal(1_u32);
    let asked = Rc::new(RefCell::new(Vec::new()));
    let made = asked.clone();
    let memo = create_async_memo(move || {
        let id = id.get();
        made.borrow_mut().push(id);
        async move { Ok(id) }
    });
    id.set(2);
    id.set(3);
    flush();
    assert_eq!(*asked.borrow(), vec![1, 3]);
    assert_eq!(memo.get_untracked(), Ok(3));

    id.set(4);
    assert_eq!(
        memo.get_untracked(),
        Ok(4),
        "a read asks for the newest input first"
    );
    flush();
    assert_eq!(
        *asked.borrow(),
        vec![1, 3, 4],
        "and the flush does not ask again"
    );
}
