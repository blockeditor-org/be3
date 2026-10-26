use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn clicking_a_switch_moves_its_knob_and_survives_a_tab_round_trip() {
    let _root = crate::reactive::RootScope::new();
    let tab = create_signal(0usize);
    let switch_ref = NodeRef::new();

    let (document, [_panel]) = toolbar_of(move || {
        let condition = create_memo(move || tab.get() == 0);
        [view! {
            <List spacing=0.0>
                <ShowKeepAlive condition>
                    <Switch @node_ref=&switch_ref on=false />
                </ShowKeepAlive>
            </List>
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let switch = switch_ref.get();
    let knob = knob_of(harness.document(), switch);
    let off_rect = harness.rect(knob);

    harness.click(harness.center(switch));
    harness.frame(Vec::new());
    let on_rect = harness.rect(knob);
    assert_ne!(off_rect, on_rect, "clicking the switch must move the knob");
    assert!(harness.document().api::<ToggleApi>(switch).checked().get());

    with_installed(harness.document_mut(), |_| tab.set(1));
    harness.frame(Vec::new());
    with_installed(harness.document_mut(), |_| tab.set(0));
    harness.frame(Vec::new());

    harness.click(harness.center(switch));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(knob), off_rect, "the knob must move back off");
    assert!(!harness.document().api::<ToggleApi>(switch).checked().get());
}

fn knob_of(document: &Document, switch: NodeId) -> NodeId {
    let mut id = switch;
    while let [child] = document.children(id)[..] {
        id = child;
    }
    id
}
