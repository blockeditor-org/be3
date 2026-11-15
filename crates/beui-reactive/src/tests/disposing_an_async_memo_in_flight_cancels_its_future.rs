use super::executor::{Answer, install, run_futures, spawned};
use super::*;
use crate::create_async_memo;

struct Dropped(Rc<Cell<bool>>);

impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

#[test]
fn disposing_an_async_memo_in_flight_cancels_its_future() {
    let _root = RootScope::new();
    install();
    let dropped = Rc::new(Cell::new(false));
    let answer = Answer::<u32>::default();
    let scope = Scope::new();
    scope.run(|| {
        let dropped = dropped.clone();
        let answer = answer.clone();
        create_async_memo(move || {
            let guard = Dropped(dropped.clone());
            let answer = answer.clone();
            async move {
                let value = answer.await;
                drop(guard);
                Ok(value)
            }
        });
    });
    run_futures();
    assert!(!dropped.get());

    scope.dispose();
    assert!(dropped.get(), "disposing it drops the future in flight");
    answer.give(1);
    run_futures();
    assert_eq!(spawned(), 0, "the executor lets go of it");
}
