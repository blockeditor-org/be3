use super::executor::install;
use super::*;
use crate::create_async_memo;

/// A memo that first runs while an async memo it reads is due again pulls it: the async memo
/// asks inside the memo's computation, writing its own state as a binding may, and its
/// answer, there at once, lands before the memo reads it.
#[test]
fn a_memo_over_an_async_memo_reads_its_answer_for_the_current_input() {
    let _root = RootScope::new();
    install();
    let id = create_signal(1_u32);
    let memo = create_async_memo(move || {
        let id = id.get();
        async move { Ok(id * 10) }
    });
    assert_eq!(
        memo.get_untracked(),
        Ok(10),
        "an answer there at once lands with no executor"
    );

    id.set(2);
    let derived = create_memo(move || memo.get());
    assert_eq!(
        derived.get(),
        Ok(20),
        "read before any flush, it asks for the new input"
    );
    flush();
    assert_eq!(derived.get(), Ok(20));
    id.set(3);
    flush();
    assert_eq!(derived.get(), Ok(30));
}
