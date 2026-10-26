use beui::WindowSize;
use beui::testing::{Showing, Stepped};
use beui::{Context, Event, ForwardedInput, FreetypeFonts, Input, Keystroke};

use super::*;

#[test]
fn escape_stops_a_presentation_rather_than_reaching_the_editor() {
    let _root = beui::reactive::RootScope::new();
    let stopped = Rc::new(Cell::new(0));
    let heard = Rc::new(RefCell::new(Vec::<Event>::new()));
    let catcher = NodeRef::new();
    let item = NodeRef::new();
    let menu = create_signal(false);
    let document = build({
        let stopped = Rc::clone(&stopped);
        let heard = Rc::clone(&heard);
        move || {
            let overlay = NodeRef::new();
            stop_on_escape(overlay);
            let forward = Callback::new(move |input: ForwardedInput| {
                heard.borrow_mut().extend(input.inputs().cloned())
            });
            view! {
                <beui::styled::Root>
                    <Overlay
                        @node_ref=&overlay
                        anchor=OverlayAnchor::Point(Pos2::ZERO)
                        placement=Placement::Fill
                        open=true
                        on_dismiss={move |_: Dismiss| stopped.set(stopped.get() + 1)}
                    >
                        <List spacing=0.0>
                            <Interactive @node_ref={&catcher} focusable=true on_forward={forward}>
                                <Frame width=100.0 height=100.0 />
                            </Interactive>
                            <Overlay
                                anchor=OverlayAnchor::Point(Pos2::new(120.0, 10.0))
                                placement=Placement::At
                                open={menu}
                                on_dismiss={move |_: Dismiss| menu.set(false)}
                            >
                                <Interactive @node_ref={&item} focusable=true>
                                    <Frame width=40.0 height=20.0 />
                                </Interactive>
                            </Overlay>
                        </List>
                    </Overlay>
                </beui::styled::Root>
            }
        }
    });
    let window = WindowSize {
        size: vec2(200.0, 200.0),
        scale: 1.0,
    };
    let mut stepped = Stepped::new(
        Context::new(FreetypeFonts::default()),
        Showing(document),
        window,
        None,
    );
    stepped.frame(Vec::new());
    stepped.document_mut().focus_focusable(catcher.get());
    let key = |key, pressed| {
        Input::Key(Keystroke {
            key: Some(key),
            pressed,
            ..Keystroke::default()
        })
    };

    stepped.frame(vec![key(Key::A, true), key(Key::A, false)]);
    assert_eq!(stopped.get(), 0, "other keys leave the presentation alone");
    assert_eq!(
        heard.borrow().len(),
        2,
        "and reach the presented editor: {:?}",
        heard.borrow()
    );

    menu.set(true);
    stepped.frame(Vec::new());
    stepped.document_mut().focus_focusable(item.get());
    stepped.frame(vec![key(Key::Escape, true), key(Key::Escape, false)]);
    stepped.frame(Vec::new());
    assert!(!menu.get_untracked(), "Escape closes a menu open over it");
    assert_eq!(stopped.get(), 0, "and leaves the presentation up");

    stepped.document_mut().focus_focusable(catcher.get());
    heard.borrow_mut().clear();
    stepped.frame(vec![key(Key::Escape, true)]);
    stepped.frame(vec![key(Key::Escape, false)]);
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
