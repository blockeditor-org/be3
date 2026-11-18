use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_kept_subtree_in_a_run_of_children_finds_its_keep_alive() {
    let _root = crate::reactive::RootScope::new();
    let shown = create_signal(true);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame width=10.0 height=10.0 />
                <KeepAlive>
                    <Show value={shown}>
                        <Kept>
                            <Frame @test_id="kept" width=10.0 height=10.0 />
                        </Kept>
                    </Show>
                </KeepAlive>
                <Wrapped shown />
                <Passed shown />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(harness.document().find_test_id("kept").is_some());
    assert!(
        harness.document().find_test_id("wrapped").is_some(),
        "and so does one a component hands back as its node"
    );
    assert!(
        harness.document().find_test_id("passed").is_some(),
        "or as a run of children it hands on"
    );
}

#[component]
fn Wrapped(shown: Signal<bool>) -> NodeId {
    view! {
        <KeepAlive>
            <Show value={shown}>
                <Kept>
                    <Frame @test_id="wrapped" width=10.0 height=10.0 />
                </Kept>
            </Show>
        </KeepAlive>
    }
}

#[component]
fn Passed(shown: Signal<bool>) -> DynamicSegment<ListItem> {
    view! {
        <KeepAlive>
            <Show value={shown}>
                <Kept>
                    <Frame @test_id="passed" width=10.0 height=10.0 />
                </Kept>
            </Show>
        </KeepAlive>
    }
}
