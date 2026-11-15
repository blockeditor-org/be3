use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_kept_inside_a_dropped_kept_of_the_same_keep_alive_goes_with_it() {
    let _root = crate::reactive::RootScope::new();
    let visible = create_signal(true);
    let listed = create_signal(vec![1u8, 2]);
    let gone = create_signal(Vec::<u8>::new());
    let builds = create_signal(0);
    let document = build_node(move || {
        let keep = create_memo(move || listed.get());
        view! {
            <KeepAlive keep={keep}>
                <List spacing=0.0>
                    <Show value={visible}>|| {
                        view! {
                            <Kept key=1u8>|| {
                                builds.update(|builds| *builds += 1);
                                on_cleanup(move || gone.update(|gone| gone.push(1)));
                                view! {
                                    <List spacing=0.0>
                                        <Kept key=2u8>|| {
                                            on_cleanup(move || gone.update(|gone| gone.push(2)));
                                            view! {
                                                <Text>"inner"</Text>
                                            }
                                        }</Kept>
                                    </List>
                                }
                            }</Kept>
                        }
                    }</Show>
                </List>
            </KeepAlive>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    listed.set(vec![2]);
    visible.set(false);
    harness.frame(Vec::new());
    assert!(gone.get_untracked().contains(&1), "the outer one, no longer kept, goes once hidden");

    visible.set(true);
    harness.frame(Vec::new());
    assert_eq!(builds.get_untracked(), 2, "and showing it again builds it afresh");
}
