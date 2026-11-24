use super::*;

#[test]
fn a_child_taking_its_placers_frame_is_asked_for_that_frame_with_its_place_as_content() {
    // The placer: a view of its own frame, drawn with a top bar, its chrome already withdrawn
    // while a child of its takes it over.
    let mut owner = placed(rect((0.0, 0.0), (300.0, 200.0)), ChildMode::Live);
    owner.own_frame = true;
    owner.top_bar = TopBar::Shown;
    let mut placer = child_screen(&owner, EditorInstanceId(1), 2.0);
    if let Some(frame) = placer.frame.as_mut() {
        frame.chrome = FrameChrome::None;
    }
    let mut card = placed(rect((40.0, 30.0), (140.0, 90.0)), ChildMode::Active);
    card.clip = rect((40.0, 30.0), (120.0, 90.0));
    let screen = framing_screen(&placer, &card, EditorInstanceId(9));
    assert_eq!(screen.instance, EditorInstanceId(9));
    assert_eq!(screen.view, ViewId::MAIN);
    // The whole of its placer's frame, at its raster, with that frame's chrome drawn and what
    // shows of its place as the content, where it draws itself as placed.
    assert_eq!(
        (screen.metrics.logical.x, screen.metrics.logical.y),
        (300.0, 200.0)
    );
    assert_eq!(
        (screen.metrics.pixel_width, screen.metrics.pixel_height),
        (600, 400)
    );
    let frame = screen
        .frame
        .expect("a child taking a frame is asked to draw it");
    assert_eq!(frame.chrome, FrameChrome::Drawn);
    assert_eq!(frame.top_bar, TopBar::Shown);
    assert_eq!(frame.content, Some(rect((40.0, 30.0), (120.0, 90.0))));
}
