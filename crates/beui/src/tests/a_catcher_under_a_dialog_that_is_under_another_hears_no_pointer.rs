use super::*;
use crate::ForwardedInput;
use crate::primitives::Frame;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn a_catcher_under_a_dialog_that_is_under_another_hears_no_pointer() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(true);
    let above = create_signal(false);
    let heard = create_signal(Vec::<Event>::new());
    let catcher = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Dialog open={open} title="Below" width=600.0 on_dismiss={|| {}}>
                    <Interactive
                        @node_ref={&catcher}
                        on_forward={move |input: ForwardedInput| {
                            heard.update(|heard| heard.extend(flattened(input)))
                        }}
                    >
                        <Frame height=300.0 />
                    </Interactive>
                </Dialog>
                <Dialog open={above} title="Above" width=200.0 on_dismiss={|| {}}>
                    <Text color=Color32::WHITE>"Body"</Text>
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    above.set(true);
    harness.frame(Vec::new());
    let below = harness.rect(catcher.get());
    heard.update(|heard| heard.clear());
    harness.frame(vec![Input::PointerMoved(below.min + vec2(10.0, 10.0))]);

    assert!(
        heard.with_untracked(|heard| heard.is_empty()),
        "the upper dialog keeps the pointer from the catcher in the one beneath it, {:?}",
        heard.get_untracked()
    );
}
