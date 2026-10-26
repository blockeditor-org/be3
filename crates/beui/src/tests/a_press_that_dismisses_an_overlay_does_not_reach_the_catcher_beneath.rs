use super::*;
use crate::ForwardedInput;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

const WIDE: Vec2 = Vec2::new(1600.0, 1000.0);

fn presses(heard: ReadSignal<Vec<Event>>) -> usize {
    heard.with_untracked(|heard| {
        heard
            .iter()
            .filter(|event| matches!(event, Event::PointerButton { pressed: true, .. }))
            .count()
    })
}

#[test]
fn a_press_that_dismisses_an_overlay_does_not_reach_the_catcher_beneath() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(true);
    let heard = create_signal(Vec::<Event>::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Canvas @sizing=ItemSize::Percent(100.0)>
                    <CanvasItem x=0.0 y=0.0 width={WIDE.x} height={WIDE.y}>
                        <Interactive
                            on_forward={move |input: ForwardedInput| {
                                heard.update(|heard| heard.extend(flattened(input)))
                            }}
                        >
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                </Canvas>
                <Dialog
                    open={open}
                    title="Event"
                    width={Size::Custom(300.0)}
                    on_dismiss={move || open.set(false)}
                >
                    <Text color=Color32::WHITE>"Body"</Text>
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::sized(document, WIDE);
    harness.frame(Vec::new());

    let outside = pos2(20.0, 20.0);
    harness.frame(vec![Input::PointerMoved(outside)]);
    harness.frame(
        [true, false]
            .into_iter()
            .map(|pressed| Input::PointerButton {
                pos: outside,
                button: PointerButton::Primary,
                pressed,
            })
            .collect(),
    );
    harness.settle();
    assert!(!open.get_untracked(), "the press outside closes the dialog");
    assert_eq!(
        presses(heard.read_only()),
        0,
        "the press that closed the dialog is not also the catcher's, {:?}",
        heard.get_untracked()
    );

    harness.click(outside);
    assert_eq!(
        presses(heard.read_only()),
        1,
        "the next press reaches the catcher, {:?}",
        heard.get_untracked()
    );

    with_installed(harness.document_mut(), |_| open.set(true));
    harness.settle();
    harness.click(outside);
    harness.settle();
    assert!(!open.get_untracked());
    assert_eq!(
        presses(heard.read_only()),
        1,
        "a press and release in frames of their own are kept from the catcher too, {:?}",
        heard.get_untracked()
    );
}
