use beui::reactive::{List, NodeRef, build, with_reactive_scope};
use beui::{Context, Event, ForwardedInput, FreetypeFonts, Modifiers, RawInput};

use super::*;

#[test]
fn escape_stops_a_presentation_rather_than_reaching_the_editor() {
    let _root = beui::reactive::RootScope::new();
    let stopped = Rc::new(Cell::new(0));
    let heard = Rc::new(RefCell::new(Vec::<Event>::new()));
    let catcher = NodeRef::new();
    let item = NodeRef::new();
    let (menu, set_menu) = create_signal(false);
    let mut document = build({
        let stopped = Rc::clone(&stopped);
        let heard = Rc::clone(&heard);
        move || {
            let overlay = NodeRef::new();
            stop_on_escape(overlay);
            view! {
                <Overlay
                    @node_ref=&overlay
                    anchor=OverlayAnchor::Point(Pos2::ZERO)
                    placement=Placement::Fill
                    open=true
                    on_dismiss={move |_: Dismiss| stopped.set(stopped.get() + 1)}
                >
                    <List spacing=0.0>
                        <Interactive
                            @node_ref={&catcher}
                            focusable=true
                            on_forward={move |input: ForwardedInput| heard.borrow_mut().extend(input.inputs().cloned())}
                        >
                            <Frame width=100.0 height=100.0 />
                        </Interactive>
                        <Overlay
                            anchor=OverlayAnchor::Point(Pos2::new(120.0, 10.0))
                            placement=Placement::At
                            open={menu}
                            on_dismiss={move |_: Dismiss| set_menu.set(false)}
                        >
                            <Interactive @node_ref={&item} focusable=true>
                                <Frame width=40.0 height=20.0 />
                            </Interactive>
                        </Overlay>
                    </List>
                </Overlay>
            }
        }
    });
    let context = Context::new(FreetypeFonts::default());
    let frame = |document: &mut beui::Document, events: Vec<Event>| {
        context.run(RawInput { events }, |context| {
            document.show(context, Rect::from_min_size(Pos2::ZERO, vec2(200.0, 200.0)));
        });
    };
    frame(&mut document, Vec::new());
    document.focus_focusable(catcher.get());
    let key = |key, pressed| {
        Event::key(beui::KeyPress {
            key,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        })
    };

    frame(&mut document, vec![key(Key::A, true), key(Key::A, false)]);
    assert_eq!(stopped.get(), 0, "other keys leave the presentation alone");
    assert_eq!(
        heard.borrow().len(),
        2,
        "and reach the presented editor: {:?}",
        heard.borrow()
    );

    with_reactive_scope(&mut document, move || set_menu.set(true));
    frame(&mut document, Vec::new());
    document.focus_focusable(item.get());
    frame(
        &mut document,
        vec![key(Key::Escape, true), key(Key::Escape, false)],
    );
    frame(&mut document, Vec::new());
    assert!(!menu.get_untracked(), "Escape closes a menu open over it");
    assert_eq!(stopped.get(), 0, "and leaves the presentation up");

    document.focus_focusable(catcher.get());
    heard.borrow_mut().clear();
    frame(&mut document, vec![key(Key::Escape, true)]);
    frame(&mut document, vec![key(Key::Escape, false)]);
    assert_eq!(
        stopped.get(),
        1,
        "Escape from the presented editor asks the presentation's layer to close"
    );
    assert!(
        !heard
            .borrow()
            .iter()
            .filter_map(Event::press)
            .any(|press| press.key == Key::Escape),
        "and neither its press nor its release reaches the editor: {:?}",
        heard.borrow()
    );
}
