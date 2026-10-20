use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::{ForwardedInput, PasteKind, PasteRequest, Pasted, PickedFile};

#[test]
fn ctrl_v_asks_for_what_the_focus_accepts_and_the_paste_reaches_it() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::new());
    let forwarded = create_signal(Vec::new());
    let document = build(move || {
        view! {
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                    <Interactive
                        focusable=true
                        pastes={vec![PasteKind::Image]}
                        on_paste={move |paste: Pasted| heard.update(|heard| heard.push(format!("{:?}", paste.kind())))}
                        on_text={move |text: String| heard.update(|heard| heard.push(text))}
                    >
                        <Frame />
                    </Interactive>
                </CanvasItem>
                <CanvasItem x=200.0 y=0.0 width=100.0 height=100.0>
                    <Interactive
                        focusable=true
                        pastes={vec![PasteKind::Text, PasteKind::Image]}
                        on_forward={move |input: ForwardedInput| forwarded.update(|forwarded| forwarded.extend(flattened(input)))}
                    >
                        <Frame />
                    </Interactive>
                </CanvasItem>
            </Canvas>
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
    assert!(
        output.window.paste_kinds.is_empty(),
        "nothing is focused yet"
    );

    harness.click(pos2(50.0, 50.0));
    let output = harness.frame(vec![ctrl_v()]);
    assert_eq!(output.paste, Some(PasteRequest::Shortcut));
    assert_eq!(output.window.paste_kinds, [PasteKind::Image]);

    let image = PickedFile {
        name: "Pasted Image.png".to_owned(),
        data: vec![1, 2, 3],
    };
    harness.frame(vec![
        Event::Paste(Pasted::Image(image.clone())),
        Event::Paste(Pasted::Text("typed".to_owned())),
    ]);
    assert_eq!(
        heard.get_untracked(),
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
    assert_eq!(
        output.window.paste_kinds,
        [PasteKind::Text, PasteKind::Image]
    );
    forwarded.update(|forwarded| forwarded.clear());
    harness.frame(vec![Event::Paste(Pasted::Image(image.clone()))]);
    assert_eq!(
        forwarded.get_untracked(),
        [Event::Paste(Pasted::Image(image))]
    );
}
