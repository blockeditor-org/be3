use super::*;
use crate::{provide_context, use_context};

#[derive(Clone, PartialEq, Debug)]
struct Theme(u32);

#[test]
fn a_keyed_row_kept_across_a_rerun_still_reads_context() {
    let root = Scope::new();
    let seen: Rc<RefCell<Vec<Option<Theme>>>> = Rc::default();
    let sink = seen.clone();
    let (keys, poke) = root.run(move || {
        provide_context(Theme(7));
        let keys = create_signal(vec![1u32]);
        let poke = create_signal(0);
        let rows = Rc::new(KeyedItems::new(move |_: u32| {
            let sink = sink.clone();
            create_effect(move || {
                poke.get();
                sink.borrow_mut().push(use_context::<Theme>());
            });
        }));
        create_effect(move || {
            let _ = rows.map(keys.get()).items().len();
        });
        (keys, poke)
    });
    assert_eq!(*seen.borrow(), [Some(Theme(7))]);

    keys.set(vec![1, 2]);
    flush();
    poke.set(1);
    flush();
    assert_eq!(
        *seen.borrow(),
        [
            Some(Theme(7)),
            Some(Theme(7)),
            Some(Theme(7)),
            Some(Theme(7))
        ],
        "a row the mapping kept across its re-run still reads the context it was built under"
    );
}
