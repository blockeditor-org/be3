use super::*;
use crate::ForwardedInput;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_catcher_under_an_open_dialog_hears_no_press() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(true);
    let heard = create_signal(Vec::<Event>::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width={VIEWPORT.x} height={VIEWPORT.y}>
                        <Interactive
                            on_forward={move |input: ForwardedInput| {
                                heard.update(|heard| heard.extend(flattened(input)))
                            }}
                        >
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                </Canvas>
                <Dialog open={open} title="Event" width={Size::Custom(300.0)} on_dismiss={|| {}}>
                    <Text color=Color32::WHITE>"Body"</Text>
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.down(pos2(10.0, 10.0), PointerButton::Primary);

    assert!(
        !heard
            .get_untracked()
            .iter()
            .any(|event| matches!(event, Event::PointerButton { .. })),
        "the dialog keeps the press from the catcher beneath it, {:?}",
        heard.get_untracked()
    );
}
