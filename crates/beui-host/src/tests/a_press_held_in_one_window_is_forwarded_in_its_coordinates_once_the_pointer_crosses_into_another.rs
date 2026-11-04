use super::*;
use crate::reactive::*;
use crate::{Forwarded, ForwardedInput, HostPlacement, WindowId, WindowPlacement, WindowReport};

#[test]
fn a_press_held_in_one_window_is_forwarded_in_its_coordinates_once_the_pointer_crosses_into_another()
 {
    let _root = crate::reactive::RootScope::new();
    let (left, right) = (WindowId::next(), WindowId::next());
    let forwarded = create_signal(Vec::<ForwardedInput>::new());
    let document = build_node(move || {
        let placed = |name: &str| WindowPlacement::Host(HostPlacement(name.to_owned()));
        view! {
            <Layers>
                <Window
                    id={Some(left)}
                    size={vec2(400.0, 300.0)}
                    placement={placed("left")}
                >|_handle: WindowHandle|
                    <Interactive
                        on_forward={move |input| forwarded.update(|forwarded| forwarded.push(input))}
                    >
                        <Frame />
                    </Interactive>
                </Window>
                <Window
                    id={Some(right)}
                    size={vec2(400.0, 300.0)}
                    placement={placed("right")}
                >|_handle: WindowHandle|
                    <Frame />
                </Window>
            </Layers>
        }
    });
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    for (window, origin) in [(left, pos2(0.0, 0.0)), (right, pos2(400.0, 50.0))] {
        harness.report_window(WindowReport::Moved { window, origin });
    }
    harness.frame(Vec::new());

    press_in(&mut harness, left, pos2(300.0, 100.0), true);
    forwarded.set(Vec::new());
    harness.frame_in(right, vec![Input::PointerMoved(pos2(20.0, 100.0))]);
    let moved = forwarded.with_untracked(|forwarded| {
        forwarded
            .iter()
            .flat_map(|input| &input.events)
            .find_map(|event| match event {
                Forwarded::Pointer { event, .. } => Some((event.window, event.pos)),
                _ => None,
            })
    });
    assert_eq!(
        moved,
        Some((left, pos2(420.0, 150.0))),
        "the window holding the press hears the pointer where it is, in its own coordinates"
    );
}
