use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_keep_alive_that_goes_takes_what_it_kept_with_it() {
    let _root = crate::reactive::RootScope::new();
    let alive = create_signal(true);
    let shown = create_signal(Some(1u64));
    let gone = create_signal(Vec::<u64>::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Show value={alive}>
                    <KeepAlive>
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
                </Show>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    shown.set(Some(2));
    harness.frame(Vec::new());
    assert!(
        gone.get_untracked().is_empty(),
        "the first is kept, hidden, while the second shows"
    );

    alive.set(false);
    harness.frame(Vec::new());
    let mut gone = gone.get_untracked();
    gone.sort_unstable();
    assert_eq!(
        gone,
        [1, 2],
        "the `KeepAlive` going disposes the hidden and the shown alike"
    );
}
