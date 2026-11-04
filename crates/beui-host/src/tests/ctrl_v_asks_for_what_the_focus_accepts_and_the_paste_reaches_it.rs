use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::clipboard::formats;
use crate::{ClipItem, ForwardedInput, native::PasteRequest};

#[test]
fn ctrl_v_asks_for_what_the_focus_accepts_and_the_paste_reaches_it() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::new());
    let forwarded = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                    <Interactive
                        focusable=true
                        pastes={vec![formats::PNG.to_owned()]}
                        on_paste={move |paste: ClipItem| heard.update(|heard| heard.push(paste.format))}
                        on_text={move |text: String| heard.update(|heard| heard.push(text))}
                    >
                        <Frame />
                    </Interactive>
                </CanvasItem>
                <CanvasItem x=200.0 y=0.0 width=100.0 height=100.0>
                    <Interactive
                        focusable=true
                        pastes={vec![formats::TEXT.to_owned(), formats::PNG.to_owned()]}
                        on_forward={move |input: ForwardedInput| forwarded.update(|forwarded| forwarded.extend(flattened(input)))}
                    >
                        <Frame />
                    </Interactive>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    let ctrl_v = || pressed_with(Key::V, Modifiers::CTRL);
    let output = harness.frame(Vec::new());
    assert_eq!(output.paste, None);
    assert!(
        output.main().window.requests.pastes.is_empty(),
        "nothing is focused yet"
    );

    harness.click(pos2(50.0, 50.0));
    let output = harness.frame(ctrl_v());
    assert_eq!(output.paste, Some(PasteRequest::Shortcut { plain: false }));
    assert_eq!(output.main().window.requests.pastes, [formats::PNG]);

    let image = ClipItem::png(vec![1, 2, 3]);
    harness.frame(vec![
        Input::Paste(image.clone()),
        Input::Paste(ClipItem::text("typed")),
    ]);
    assert_eq!(
        heard.get_untracked(),
        ["image/png", "typed"],
        "a kind the focus did not ask for still reaches its text handler"
    );

    harness.click(pos2(250.0, 50.0));
    let output = harness.frame(ctrl_v());
    assert_eq!(
        output.paste,
        Some(PasteRequest::Shortcut { plain: false }),
        "a forwarding focus pastes through its host"
    );
    assert_eq!(
        output.main().window.requests.pastes,
        [formats::TEXT, formats::PNG]
    );
    forwarded.update(|forwarded| forwarded.clear());
    harness.frame(vec![Input::Paste(image.clone())]);
    assert_eq!(
        forwarded.get_untracked(),
        [Event::Paste(image)]
    );
}
