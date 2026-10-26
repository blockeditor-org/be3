use super::*;
use crate::primitives::List;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_dialog_opens_in_the_middle_and_escape_dismisses_it() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let dismissed = create_signal(0);
    let surface = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Dialog
                    open={open}
                    title="Event"
                    width={Size::Custom(300.0)}
                    on_dismiss={move || dismissed.update(|dismissed| *dismissed += 1)}
                >
                    <Text color=Color32::WHITE @node_ref={&surface}>"Body"</Text>
                </Dialog>
            </List>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(
        surface.try_get().is_none(),
        "a closed dialog builds nothing it holds"
    );

    open.set(true);
    harness.frame(Vec::new());
    let body = harness.rect(surface.get());
    assert!(body.is_positive());
    assert!((body.center().x - VIEWPORT.x / 2.0).abs() < 2.0);

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        dismissed.get_untracked(),
        0,
        "Escape plays the slide-away a back gesture plays"
    );
    harness.settle();
    assert_eq!(
        dismissed.get_untracked(),
        1,
        "and dismisses once it is gone"
    );
}
