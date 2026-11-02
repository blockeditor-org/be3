use beui::native::{Context, WindowHosting, WindowSize};
use beui::{Window, WindowHandle};
use beui_host::native::testing::{Showing, Stepped};
use beui_host::native::{FreetypeFonts, build};

use super::*;

#[component]
fn Probe(held: Signal<Option<Memo<WindowId>>>) -> NodeId {
    held.set(Some(host_window()));
    view! {
        <Text>"probe"</Text>
    }
}

#[test]
fn the_host_window_follows_a_window_that_becomes_real() {
    let _root = RootScope::new();
    let held = create_signal(None::<Memo<WindowId>>);
    let id = WindowId::next();
    let document = build(move || {
        view! {
            <Root title="Block">
                <Window id={Some(id)} size={vec2(160.0, 120.0)}>|_: WindowHandle|
                    <Probe held />
                </Window>
            </Root>
        }
    });
    let window = WindowSize {
        size: vec2(400.0, 300.0),
        scale: 1.0,
    };
    let mut stepped = Stepped::new(
        Context::new(FreetypeFonts::default()),
        Showing(document),
        window,
        None,
    );
    stepped.frame(Vec::new());
    let host = held.get_untracked().expect("the probe was built");
    assert_eq!(
        host.get_untracked(),
        WindowId::MAIN,
        "a window the host does not open is drawn in the main window"
    );
    stepped.context().set_window_hosting(WindowHosting::Any);
    stepped.frame(Vec::new());
    assert_eq!(
        host.get_untracked(),
        id,
        "once the host opens the window, what was built in it is in that window, not the main one"
    );
}
