use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn an_awake_kept_subtree_keeps_running_and_reads_as_shown_while_hidden() {
    let _root = crate::reactive::RootScope::new();
    let visible = create_signal(true);
    let awake = create_signal(vec![1u8]);
    let ticks = create_signal(0);
    let seen = create_signal(0);
    let shown_inside = create_signal(None::<bool>);
    let document = build_node(move || {
        let awake = create_memo(move || awake.get());
        view! {
            <KeepAlive awake={awake}>
                <List spacing=0.0>
                    <Show value={visible}>|| {
                        view! {
                            <Kept key=1u8>|| {
                                let shown = kept_shown();
                                create_effect(move || {
                                    ticks.get();
                                    seen.update(|seen| *seen += 1);
                                    shown_inside.set(shown.map(|shown| shown.get()));
                                });
                                view! {
                                    <Text>"held"</Text>
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
    visible.set(false);
    harness.frame(Vec::new());
    let ran = seen.get_untracked();
    ticks.set(1);
    harness.frame(Vec::new());
    assert_eq!(seen.get_untracked(), ran + 1, "an awake subtree runs while hidden");
    assert_eq!(shown_inside.get_untracked(), Some(true), "and reads as shown");

    awake.set(Vec::new());
    harness.frame(Vec::new());
    let ran = seen.get_untracked();
    ticks.set(2);
    harness.frame(Vec::new());
    assert_eq!(seen.get_untracked(), ran, "dropped from `awake`, it pauses");
}
