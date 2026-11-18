use super::*;
use crate::Reader;

struct Uncloned(u32);

#[test]
fn a_reader_reads_a_signal_a_memo_or_a_closure_alike() {
    let _root = RootScope::new();
    let count = create_signal(1);
    let doubled = create_memo(move || count.get() * 2);
    let readers: [Reader<u32>; 4] = [
        count.into(),
        count.read_only().into(),
        doubled.into(),
        Reader::new(move || count.get() + 10),
    ];
    let seen = Rc::new(RefCell::new(Vec::new()));
    let recorded = seen.clone();
    create_effect(move || {
        let values: Vec<u32> = readers.iter().map(Get::get).collect();
        recorded.borrow_mut().push(values);
    });
    flush();
    count.set(2);
    flush();
    assert_eq!(*seen.borrow(), [vec![1, 1, 2, 11], vec![2, 2, 4, 12]]);

    let held: Reader<Uncloned> = create_signal(Uncloned(7)).into();
    assert_eq!(
        held.with_untracked(|held| held.0),
        7,
        "a signal is read in place"
    );
}
