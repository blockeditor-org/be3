use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

fn laid_out(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

#[test]
fn an_action_bar_keeps_the_pins_it_has_room_for_and_offers_the_rest() {
    let _root = crate::reactive::RootScope::new();
    let narrow = create_signal(false);
    let document = build_node(move || {
        let undo = Action::new("undo", "Undo", || {})
            .glyph(crate::icons::ICON_UNDO_2)
            .pin(Pin::Icon)
            .detached();
        let share = Action::new("share", "Share", || {})
            .pin(Pin::Label)
            .detached();
        let rename = Action::new("rename", "Rename", || {})
            .pin(Pin::Title)
            .detached();
        let delete = Action::new("delete", "Delete", || {}).detached();
        let menu = vec![
            MenuNode::action(&undo),
            MenuNode::action(&share),
            MenuNode::action(&rename),
            MenuNode::action(&delete),
        ];
        view! {
            <List spacing=0.0>
                <Dynamic value={narrow}>|narrow: bool| {
                    let menu = menu.clone();
                    match narrow {
                        false => view! {
                            <ActionBar menu />
                        },
                        true => view! {
                            <ActionBar menu labels=ActionLabels::Overflow title=true />
                        },
                    }
                }</Dynamic>
            </List>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    assert!(laid_out(&harness, "actions.pin.undo"));
    assert!(laid_out(&harness, "actions.pin.share"));
    assert!(!laid_out(&harness, "actions.pin.rename"));
    harness.click(harness.center(harness.find("actions.menu")));
    harness.settle();
    assert!(
        laid_out(&harness, "actions.menu.rename"),
        "a title pin stays in the menu"
    );
    assert!(laid_out(&harness, "actions.menu.delete"));
    assert!(
        !laid_out(&harness, "actions.menu.share"),
        "the menu leaves out what the bar holds"
    );
    harness.key(Key::Escape, Modifiers::NONE);
    harness.settle();

    narrow.set(true);
    harness.settle();
    assert!(laid_out(&harness, "actions.pin.undo"));
    assert!(
        !laid_out(&harness, "actions.pin.share"),
        "labelled pins overflow when there is no room for labels"
    );
    harness.click(harness.center(harness.find("actions.menu")));
    harness.settle();
    assert!(laid_out(&harness, "actions.menu.share"));
    assert!(
        !laid_out(&harness, "actions.menu.rename"),
        "the title action is the caller's to show"
    );
}
