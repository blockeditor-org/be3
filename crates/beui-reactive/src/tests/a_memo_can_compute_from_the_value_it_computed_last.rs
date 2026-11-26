use super::*;

#[test]
fn a_memo_can_compute_from_the_value_it_computed_last() {
    let _root = RootScope::new();
    let measured = create_signal(None::<u32>);
    let kept = create_memo(move |previous: Option<&u32>| {
        measured.get().or(previous.copied()).unwrap_or(0)
    });
    assert_eq!(kept.get(), 0);
    measured.set(Some(5));
    assert_eq!(kept.get(), 5);
    measured.set(None);
    assert_eq!(
        kept.get(),
        5,
        "an answer that turns `None` keeps the last one"
    );
}
