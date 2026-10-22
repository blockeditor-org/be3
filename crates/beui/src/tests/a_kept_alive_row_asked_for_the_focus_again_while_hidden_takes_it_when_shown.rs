use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

#[test]
fn a_kept_alive_row_asked_for_the_focus_again_while_hidden_takes_it_when_shown() {
    let _root = crate::reactive::RootScope::new();
    let row = NodeRef::new();
    let visible = create_signal(true);
    let aimed = create_signal(0);
    let document = build_node(move || {
        let wanted = create_memo(move || aimed.get() == 0);
        view! {
            <List spacing=0.0>
                <Button on_click={|| ()}>
                    <Text>"outside"</Text>
                </Button>
                <ShowKeepAlive condition={visible}>
                    <Button @node_ref=&row focused={wanted} on_click={|| ()}>
                        <Text>"row"</Text>
                    </Button>
                </ShowKeepAlive>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let row = row.get();
    let holds = |harness: &Harness| harness.document().focus_is_within(row);
    assert!(holds(&harness), "the row asked for the focus takes it");

    visible.set(false);
    harness.frame(Vec::new());
    assert!(!holds(&harness), "hidden, it gives the focus up");

    aimed.set(1);
    harness.frame(Vec::new());
    aimed.set(0);
    harness.frame(Vec::new());
    visible.set(true);
    harness.frame(Vec::new());
    assert!(
        holds(&harness),
        "asked for the focus again while hidden, it takes it when it is shown"
    );
}
