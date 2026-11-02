use std::cell::RefCell;
use std::rc::Rc;

use beui::WindowEdge;
use smithay::reexports::wayland_protocols::xdg::shell::client::xdg_toplevel::ResizeEdge;

use super::*;
use crate::state::Grab;
use crate::view::Carrier;

thread_local! {
    static BEGUN: RefCell<Vec<Grab>> = const { RefCell::new(Vec::new()) };
    static STEERED: RefCell<usize> = const { RefCell::new(0) };
}

#[component]
fn Carried(windows: Windows) -> NodeId {
    let ids = create_memo(clone_list(&windows));
    view! {
        <Layers>
            <ForEach keys={ids}>|id: WindowId| {
                let carrier = Carrier {
                    begin: Rc::new(|grab| {
                        BEGUN.with(|begun| begun.borrow_mut().push(grab));
                        true
                    }),
                    steer: Rc::new(|input| {
                        STEERED.with(|steered| *steered.borrow_mut() += input.events.len());
                        input
                    }),
                };
                view! {
                    <Frame width={Size::Custom(SHOWN.x)} height={Size::Custom(SHOWN.y)}>
                        <WindowView windows id carrier={Some(carrier)} />
                    </Frame>
                }
            }</ForEach>
        </Layers>
    }
}

fn begun() -> Vec<Grab> {
    BEGUN.with(|begun| std::mem::take(&mut *begun.borrow_mut()))
}

#[test]
fn a_programs_move_resize_menu_and_minimize_requests_reach_the_shell() {
    let mut harness = Harness::showing(None, |windows| {
        view! {
            <Carried windows />
        }
    });
    let _pointer = harness.client.pointer();
    let (window, id) = harness.open();
    let toplevel = window.toplevel.clone().expect("the window is a toplevel");
    let seat = harness.client.seat.clone().expect("the client has a seat");
    let at = harness.window(id).min + Vec2::new(40.0, 10.0);

    harness.frame(vec![
        Input::PointerMoved(at),
        Input::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: true,
        },
    ]);
    harness.settle();
    assert!(
        STEERED.with(|steered| *steered.borrow()) > 0,
        "the window's input passes through the shell that carries it"
    );
    let serial = harness
        .client
        .received
        .serial
        .expect("the press reached the program");
    toplevel._move(&seat, serial);
    harness.settle();
    assert_eq!(
        begun(),
        [Grab::Move],
        "a move asked while the button is held begins one"
    );
    toplevel.resize(&seat, serial, ResizeEdge::Right);
    harness.settle();
    assert_eq!(
        begun(),
        [Grab::Resize(WindowEdge::Right)],
        "a resize from the right edge begins one there"
    );

    harness.frame(vec![Input::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed: false,
    }]);
    harness.settle();
    toplevel._move(&seat, serial);
    harness.settle();
    assert!(
        begun().is_empty(),
        "a move asked after the press was let go carries nothing"
    );

    toplevel.show_window_menu(&seat, serial, 30, 20);
    harness.settle();
    toplevel.show_window_menu(&seat, serial, 50, 60);
    harness.settle();
    assert_eq!(
        harness.compositor().windows().menu(id),
        Some((2, Vec2::new(50.0, 60.0))),
        "each ask for the window menu counts up and says where"
    );

    toplevel.set_minimized();
    harness.settle();
    assert_eq!(
        harness.compositor().windows().take_minimized(),
        [id],
        "a minimized window is handed to the shell to hide"
    );
}
