use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::reactive::{Canvas, CanvasItem, Frame, Interactive, build, view};
use crate::{ForwardedInput, PasteKind, PasteRequest, Pasted, PickedFile};

#[test]
fn ctrl_v_asks_for_what_the_focus_accepts_and_the_paste_reaches_it() {
    let heard = Rc::new(RefCell::new(Vec::new()));
    let forwarded = Rc::new(RefCell::new(Vec::new()));
    let document = build({
        let (heard, forwarded) = (Rc::clone(&heard), Rc::clone(&forwarded));
        move || {
            let (pasted, typed) = (Rc::clone(&heard), Rc::clone(&heard));
            view! {
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                        <Interactive
                            focusable=true
                            pastes={vec![PasteKind::Image]}
                            on_paste={move |paste: Pasted| pasted.borrow_mut().push(format!("{:?}", paste.kind()))}
                            on_text={move |text: String| typed.borrow_mut().push(text)}
                        >
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                    <CanvasItem x=200.0 y=0.0 width=100.0 height=100.0>
                        <Interactive
                            focusable=true
                            pastes={vec![PasteKind::Text, PasteKind::Image]}
                            on_forward={move |input: ForwardedInput| forwarded.borrow_mut().extend(flattened(input))}
                        >
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                </Canvas>
            }
        }
    });
    let mut harness = Harness::new(document);
    let ctrl_v = || {
        Event::key(beui::KeyPress {
            key: Key::V,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        })
    };
    let output = harness.frame(Vec::new());
    assert_eq!(output.paste, None);
    assert!(output.paste_kinds.is_empty(), "nothing is focused yet");

    harness.click(pos2(50.0, 50.0));
    let output = harness.frame(vec![ctrl_v()]);
    assert_eq!(output.paste, Some(PasteRequest::Shortcut));
    assert_eq!(output.paste_kinds, [PasteKind::Image]);

    let image = PickedFile {
        name: "Pasted Image.png".to_owned(),
        data: vec![1, 2, 3],
    };
    harness.frame(vec![
        Event::Paste(Pasted::Image(image.clone())),
        Event::Paste(Pasted::Text("typed".to_owned())),
    ]);
    assert_eq!(
        *heard.borrow(),
        ["Image", "typed"],
        "a kind the focus did not ask for still reaches its text handler"
    );

    harness.click(pos2(250.0, 50.0));
    let output = harness.frame(vec![ctrl_v()]);
    assert_eq!(
        output.paste,
        Some(PasteRequest::Shortcut),
        "a forwarding focus pastes through its host"
    );
    assert_eq!(output.paste_kinds, [PasteKind::Text, PasteKind::Image]);
    forwarded.borrow_mut().clear();
    harness.frame(vec![Event::Paste(Pasted::Image(image.clone()))]);
    assert_eq!(*forwarded.borrow(), [Event::Paste(Pasted::Image(image))]);
}
