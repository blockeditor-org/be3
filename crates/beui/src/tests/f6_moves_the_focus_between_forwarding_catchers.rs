use super::*;
use crate::ForwardedInput;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn f6_moves_the_focus_between_forwarding_catchers() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal::<Vec<(usize, bool, Vec<Event>)>>(Vec::new());
    let catcher = |index: usize, heard: Signal<Vec<(usize, bool, Vec<Event>)>>| {
        move |input: ForwardedInput| {
            heard.update(|heard| heard.push((index, input.focused, flattened(input))))
        }
    };
    let document = build_node(move || {
        let first = catcher(0, heard);
        let last = catcher(1, heard);
        view! {
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                    <Interactive focusable=true on_forward={first}>
                        <Frame />
                    </Interactive>
                </CanvasItem>
                <CanvasItem x=200.0 y=0.0 width=100.0 height=100.0>
                    <Interactive focusable=true>
                        <Frame />
                    </Interactive>
                </CanvasItem>
                <CanvasItem x=400.0 y=0.0 width=100.0 height=100.0>
                    <Interactive focusable=true on_forward={last}>
                        <Frame />
                    </Interactive>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(50.0, 50.0));
    let f6 = |shift: bool| match shift {
        true => pressed_with(Key::F6, Modifiers::SHIFT),
        false => vec![key_event(Key::F6, true)],
    };

    heard.update(|heard| heard.clear());
    harness.frame(f6(false));
    let focused = |heard: &[(usize, bool, Vec<Event>)]| {
        heard
            .iter()
            .rev()
            .find(|(_, focused, _)| *focused)
            .map(|(index, _, _)| *index)
    };
    assert_eq!(
        focused(&heard.get_untracked()),
        Some(1),
        "F6 skips the plain focusable"
    );
    assert!(
        heard
            .get_untracked()
            .iter()
            .all(|(_, _, events)| events.is_empty()),
        "the F6 itself reaches no catcher"
    );

    heard.update(|heard| heard.clear());
    harness.frame(f6(true));
    assert_eq!(
        focused(&heard.get_untracked()),
        Some(0),
        "Shift+F6 goes back"
    );
}
