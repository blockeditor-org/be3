use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_kept_subtree_inside_a_hidden_one_pauses_with_it() {
    let _root = crate::reactive::RootScope::new();
    let visible = create_signal(true);
    let ticks = create_signal(0);
    let seen = create_signal(0);
    let document = build_node(move || {
        view! {
            <KeepAlive>
                <List spacing=0.0>
                    <Show value={visible}>||
                        <Kept key=1u8>||
                            <List spacing=0.0>
                                <Kept key=2u8>|| {
                                    create_effect(move || {
                                        ticks.get();
                                        seen.update(|seen| *seen += 1);
                                    });
                                    view! {
                                        <Text>"inner"</Text>
                                    }
                                }</Kept>
                            </List>
                        </Kept>
                    </Show>
                </List>
            </KeepAlive>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let ran = seen.get_untracked();

    visible.set(false);
    harness.frame(Vec::new());
    ticks.set(1);
    harness.frame(Vec::new());
    assert_eq!(
        seen.get_untracked(),
        ran,
        "the inner kept subtree waits while the outer is hidden"
    );

    visible.set(true);
    harness.frame(Vec::new());
    assert_eq!(
        seen.get_untracked(),
        ran + 1,
        "and catches up once it shows"
    );
}
