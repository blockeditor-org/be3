use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn escape_hides_a_focused_tooltip_and_a_second_escape_reaches_its_dialog() {
    let _root = crate::reactive::RootScope::new();
    let save = NodeRef::new();
    let open = create_signal(true);
    let document = build_node(move || {
        view! {
            <List spacing=8.0>
                <Dialog open={open} title="Export" on_dismiss={move || open.set(false)}>
                    <Tooltip label="Saves a copy">
                        <Button @node_ref=&save label="Save" />
                    </Tooltip>
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.settle();
    for _ in 0..10 {
        if harness.document().focus_is_within(save.get()) {
            break;
        }
        harness.key(Key::Tab, Modifiers::NONE);
        harness.frame(Vec::new());
    }
    assert!(harness.document().focus_is_within(save.get()));
    let tip = |harness: &Harness| harness.document().find_test_id("tooltip").is_some();
    assert!(tip(&harness), "the keyboard's focus shows the tooltip");

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(!tip(&harness), "Escape hides the tooltip");
    assert!(open.get_untracked(), "and goes no further than the tooltip");
    assert!(
        harness.document().focus_is_within(save.get()),
        "the focus stays on the button"
    );
    harness.frame(Vec::new());
    assert!(
        !tip(&harness),
        "and the tooltip stays hidden while the focus does"
    );

    harness.key(Key::Escape, Modifiers::NONE);
    harness.settle();
    assert!(
        !open.get_untracked(),
        "with no tooltip shown, Escape reaches the dialog"
    );
}
