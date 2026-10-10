use beui::reactive::{NodeRef, build};
use beui::{Context, Event, ForwardedInput, FreetypeFonts, Modifiers, RawInput};

use super::*;

#[test]
fn escape_stops_a_presentation_rather_than_reaching_the_editor() {
    let stopped = Rc::new(Cell::new(0));
    let heard = Rc::new(RefCell::new(Vec::<Event>::new()));
    let catcher = NodeRef::new();
    let mut document = build({
        let stopped = Rc::clone(&stopped);
        let heard = Rc::clone(&heard);
        let catcher = catcher.clone();
        move || {
            stop_on_escape(move || stopped.set(stopped.get() + 1));
            view! {
                <Interactive
                    @node_ref={&catcher}
                    focusable=true
                    on_forward={move |input: ForwardedInput| heard.borrow_mut().extend(input.events)}
                >
                    <Frame width=100.0 height=100.0 />
                </Interactive>
            }
        }
    });
    let context = Context::new(FreetypeFonts::default());
    let mut frame = |document: &mut beui::Document, events: Vec<Event>| {
        context.run(RawInput { events }, |context| {
            document.show(context, Rect::from_min_size(Pos2::ZERO, vec2(200.0, 200.0)));
        });
    };
    frame(&mut document, Vec::new());
    document.focus_focusable(catcher.get());
    let key = |key, pressed| Event::Key {
        key,
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    };

    frame(&mut document, vec![key(Key::A, true), key(Key::A, false)]);
    assert_eq!(stopped.get(), 0, "other keys leave the presentation alone");
    assert_eq!(
        heard.borrow().len(),
        2,
        "and reach the presented editor: {:?}",
        heard.borrow()
    );

    frame(&mut document, vec![key(Key::Escape, true)]);
    frame(&mut document, vec![key(Key::Escape, false)]);
    assert_eq!(stopped.get(), 1, "Escape stops presenting");
    assert!(
        !heard
            .borrow()
            .iter()
            .any(|event| matches!(event, Event::Key { key: Key::Escape, .. })),
        "and neither its press nor its release reaches the editor: {:?}",
        heard.borrow()
    );
}
