use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_kept_key_dropped_from_its_keep_set_goes_once_nothing_shows_it() {
    let _root = crate::reactive::RootScope::new();
    let listed = create_signal(vec![1u64, 2]);
    let shown = create_signal(Some(1u64));
    let gone = create_signal(Vec::<u64>::new());
    let document = build_node(move || {
        let keep = create_memo(move || listed.get());
        view! {
            <KeepAlive keep={keep}>
                <List spacing=0.0>
                    <Dynamic value={shown} fallback=()>|key: u64| {
                        view! {
                            <Kept key>|| {
                                on_cleanup(move || gone.update(|gone| gone.push(key)));
                                view! {
                                    <Text>{format!("panel {key}")}</Text>
                                }
                            }</Kept>
                        }
                    }</Dynamic>
                </List>
            </KeepAlive>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    shown.set(Some(2));
    harness.frame(Vec::new());
    assert!(
        gone.get_untracked().is_empty(),
        "a listed key is kept while hidden"
    );

    listed.set(vec![2]);
    harness.frame(Vec::new());
    assert_eq!(
        gone.get_untracked(),
        [1],
        "a hidden key dropped from the list goes at once"
    );

    listed.set(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        gone.get_untracked(),
        [1],
        "a dropped key stays while a `Kept` shows it"
    );
    shown.set(None);
    harness.frame(Vec::new());
    assert_eq!(
        gone.get_untracked(),
        [1, 2],
        "and goes once that `Kept` does"
    );
}
