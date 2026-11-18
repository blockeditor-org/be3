use super::*;
use crate::WindowId;
use crate::drag::{self, Carry};
use crate::unstyled::{place_on, use_drop_site};
use crate::{Window, WindowHandle};

#[test]
fn a_carry_pressed_in_one_window_and_let_go_in_another_drops_on_a_site_there() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let dropped = create_signal(None::<(u32, Pos2)>);
    let beneath = create_signal(false);
    let document = build_node(move || {
        let (under, inside) = (NodeRef::new(), NodeRef::new());
        use_drop_site(
            Func::new(|_: u32| true),
            Func::new(move |_: u32| {
                drag::closest([
                    place_on(under, None).map_or(drag::floating(), drag::fixed),
                    drag::custom(inside, |at: Pos2, _| Some(Some(at))),
                ])
            }),
            Callback::new(move |(payload, at): (u32, Option<Pos2>)| match at {
                Some(at) => dropped.set(Some((payload, at))),
                None => beneath.set(true),
            }),
        );
        view! {
            <List spacing=0.0>
                <Interactive drag={Func::new(|_: PointerPress| Some(Carry::new(7_u32)))}>
                    <Frame width=40.0 height=40.0 />
                </Interactive>
                <Frame @node_ref=&under width=200.0 height=200.0 />
                <Window id={window} size={vec2(160.0, 120.0)}>|_: WindowHandle|
                    <Frame @node_ref=&inside />
                </Window>
            </List>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let press = |pressed| Input::PointerButton {
        pos: pos2(20.0, 20.0),
        button: PointerButton::Primary,
        pressed,
    };
    harness.frame(vec![Input::PointerMoved(pos2(20.0, 20.0)), press(true)]);
    harness.frame(vec![Input::PointerMoved(pos2(30.0, 60.0))]);
    harness.frame(vec![Input::PointerMoved(pos2(30.0, 100.0))]);
    harness.frame_in(window, vec![Input::PointerMoved(pos2(70.0, 60.0))]);
    harness.frame_in(window, vec![Input::PointerMoved(pos2(80.0, 70.0))]);
    harness.frame_in(
        window,
        vec![Input::PointerButton {
            pos: pos2(80.0, 70.0),
            button: PointerButton::Primary,
            pressed: false,
        }],
    );

    assert_eq!(
        dropped.get_untracked(),
        Some((7, pos2(80.0, 70.0))),
        "the drop lands on the site in the window the pointer is over, where it is in that window"
    );
    assert!(
        !beneath.get_untracked(),
        "the place in the main window under the same coordinates takes nothing"
    );
}
