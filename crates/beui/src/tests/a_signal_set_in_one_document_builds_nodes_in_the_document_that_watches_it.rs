use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

#[test]
fn a_signal_set_in_one_document_builds_nodes_in_the_document_that_watches_it() {
    let _root = crate::reactive::RootScope::new();
    let shown = create_signal(false);
    let press = NodeRef::new();
    let pressing = build_node(move || {
        view! {
            <List spacing=0.0>
                <Button @node_ref=&press on_click={move || shown.set(true)}>
                    <Text>"show"</Text>
                </Button>
            </List>
        }
    });
    let watching = build_node(move || {
        view! {
            <List spacing=0.0>
                <Show condition={shown}>
                    <Text @test_id={"late"}>"late"</Text>
                </Show>
            </List>
        }
    });
    let press = press.get();
    let mut pressing = Harness::new(pressing);
    let mut watching = Harness::new(watching);
    pressing.frame(Vec::new());
    watching.frame(Vec::new());

    pressing.click(pressing.center(press));
    pressing.frame(Vec::new());
    watching.frame(Vec::new());

    assert_eq!(pressing.document().find_test_id("late"), None);
    assert!(watching.document().find_test_id("late").is_some());
}
