use super::*;
use crate::drag::{self, Carry};
use crate::unstyled::{place_on, use_drop_site};

fn said(harness: &Harness) -> Option<String> {
    harness
        .accessible()
        .into_iter()
        .find(|node| node.role() == accesskit::Role::Status && node.label().is_some())
        .and_then(|node| node.label().map(str::to_owned))
}

/// A carry's places are each in the window the core says their node is in. With a dock window
/// of its own open, a keyboard carry, which no pointer tracks, is lifted over no place though
/// the dock window's covers its source in that window's own points, and steps and counts only
/// the places in its source's window; a pointer's carry lands on the place in the window it is
/// let go in.
#[test]
fn a_carry_lifted_with_a_dock_window_open_drops_in_the_window_it_is_let_go_in() {
    let _root = crate::reactive::RootScope::new();
    let dropped = create_signal(Vec::<(u32, &'static str)>::new());
    let (dock, source, under, inside) =
        (NodeRef::new(), NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        use_drop_site(
            Func::new(|_: u32| true),
            Func::new(move |_: u32| {
                drag::closest([place_on(under, "main"), place_on(inside, "window")].into_iter().flatten())
            }),
            Callback::new(move |(payload, place): (u32, &'static str)| {
                dropped.update(|dropped| dropped.push((payload, place)));
            }),
        );
        let layout = dock::DockingLayout::<u64>::new();
        view! {
            <dock_host::styled::DockHost @node_ref=&dock>
                <dock::Docking layout>
                    <dock::DockPane id="main">
                        <dock::DockTab id=1u64 title="Tab 1">
                            <List spacing=0.0 align=Align::Start>
                                <Interactive
                                    @node_ref=&source
                                    focusable=true
                                    drag={Func::new(|_: PointerPress| Some(Carry::new(7_u32)))}
                                >
                                    <Frame width=40.0 height=40.0 />
                                </Interactive>
                                <Frame height=100.0 />
                                <Frame @node_ref=&under width=40.0 height=200.0 />
                            </List>
                        </dock::DockTab>
                    </dock::DockPane>
                    <dock::DockWindow
                        id="floating"
                        rect={Rect::from_min_size(pos2(500.0, 60.0), vec2(360.0, 240.0))}
                    >
                        <dock::DockPane id="floating">
                            <dock::DockTab id=2u64 title="Tab 2">
                                <Frame @node_ref=&inside />
                            </dock::DockTab>
                        </dock::DockPane>
                    </dock::DockWindow>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let (dock, source, under, inside) = (dock.get(), source.get(), under.get(), inside.get());
    let main = window_showing(&harness, dock, "Tab 1");
    let floating = window_showing(&harness, dock, "Tab 2");
    assert_ne!(main, floating, "the dock window is a window of its own");
    let from = harness.center(source);
    let below = harness.center(under);
    let window_place = harness.rect(inside);
    assert!(
        window_place.contains(from),
        "in its own points, the dock window's place covers the source"
    );

    press_in(&mut harness, main, from, true);
    press_in(&mut harness, main, from, false);
    harness.key(Key::Space, Modifiers::NONE);
    assert_eq!(
        said(&harness).as_deref(),
        Some("Lifted, 1 place to drop it"),
        "lifted over no place, with only the main window's to step to"
    );
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(
        said(&harness).as_deref(),
        Some("Lifted, 1 place to drop it"),
        "no place is to the right in the main window, though the dock window's is in its points"
    );
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(
        said(&harness).as_deref(),
        Some("Over a place, place 1 of 1")
    );
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    press_in(&mut harness, main, from, true);
    harness.frame_in(floating, vec![Input::PointerMoved(below)]);
    harness.frame_in(floating, vec![Input::PointerMoved(window_place.center())]);
    press_in(&mut harness, floating, window_place.center(), false);
    harness.frame(Vec::new());

    assert_eq!(
        dropped.get_untracked(),
        [(7, "main"), (7, "window")],
        "the keyboard steps to the main window's place, and the pointer lets go on the dock window's"
    );
}
