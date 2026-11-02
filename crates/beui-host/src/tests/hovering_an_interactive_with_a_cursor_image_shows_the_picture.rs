use super::*;
use beui::{CursorIcon, CursorImage, Image};

#[test]
fn hovering_an_interactive_with_a_cursor_image_shows_the_picture() {
    let picture = CursorImage {
        image: Image::from_rgba(2, 2, vec![255; 16]),
        size: vec2(2.0, 2.0),
        hotspot: vec2(1.0, 0.0),
    };
    let shown = picture.clone();
    let (document, [area]) = toolbar_of(move || {
        [view! {
            <Interactive cursor=CursorIcon::Text cursor_image={shown}>
                <Frame width=40.0 height=30.0 />
            </Interactive>
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let over = harness.rect(area).center();

    let output = harness.frame(vec![Input::PointerMoved(over)]);
    let window = &output.main().window;
    assert_eq!(window.cursor_image.as_ref(), Some(&picture));
    assert_eq!(
        window.requests.cursor,
        CursorIcon::Text,
        "the icon stays what platforms without pictures show"
    );

    let output = harness.frame(vec![Input::PointerMoved(pos2(1000.0, 1000.0))]);
    assert_eq!(output.main().window.cursor_image, None);

    harness.context().set_touch_emulation(true);
    let output = harness.frame(vec![Input::PointerMoved(over)]);
    let window = &output.main().window;
    assert_eq!(window.requests.cursor, CursorIcon::Touch);
    assert_eq!(window.cursor_image, None, "touch emulation shows the finger");
}
