use super::*;
use crate::ForwardedInput;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_catcher_in_a_dialog_hears_a_press_on_it() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(true);
    let heard = create_signal(Vec::<Event>::new());
    let catcher = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Dialog open={open} title="Dialog" width=400.0 on_dismiss={|| {}}>
                    <Interactive
                        @node_ref={&catcher}
                        on_forward={move |input: ForwardedInput| {
                            heard.update(|heard| heard.extend(flattened(input)))
                        }}
                    >
                        <Frame height=200.0 />
                    </Interactive>
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let rect = harness.rect(catcher.get());
    harness.press_at(rect.center());

    assert!(
        heard
            .get_untracked()
            .iter()
            .any(|event| matches!(event, Event::PointerButton { pressed: true, .. })),
        "{:?}",
        heard.get_untracked()
    );
}
