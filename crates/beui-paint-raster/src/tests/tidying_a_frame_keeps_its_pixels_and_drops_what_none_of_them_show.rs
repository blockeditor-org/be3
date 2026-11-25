use super::*;

fn rect(clip: [f32; 4], rect: [f32; 4], color: [u8; 4]) -> Primitive {
    Primitive {
        clip,
        fade: Fade::NONE,
        rounds: Vec::new(),
        content: Content::RoundedRect(RoundedRect {
            rect,
            corner_radius: 0.0,
            stroke_width: 0.0,
            color,
            turn: Turn::NONE,
        }),
    }
}

#[test]
fn tidying_a_frame_keeps_its_pixels_and_drops_what_none_of_them_show() {
    let whole = [0.0, 0.0, 20.0, 20.0];
    let background = [10, 10, 10, 255];
    let (red, blue) = ([200, 0, 0, 255], [0, 0, 200, 255]);
    let mut turned_by_nothing = rect(whole, [14.0, 14.0, 18.0, 18.0], [0, 200, 0, 128]);
    if let Content::RoundedRect(shape) = &mut turned_by_nothing.content {
        shape.turn.pivot = [3.0, 4.0];
    }
    let mut frame = Frame {
        size: [20, 20],
        pixels_per_point: 1.0,
        background,
        primitives: vec![
            // The background's colour on the background.
            rect(whole, [0.0, 0.0, 20.0, 20.0], background),
            // Under the red rect drawn after it.
            rect(whole, [3.0, 3.0, 6.0, 6.0], blue),
            // Clipped to nothing in the frame.
            rect([30.0, 30.0, 40.0, 40.0], [0.0, 0.0, 40.0, 40.0], blue),
            rect(whole, [0.0, 0.0, 0.0, 0.0], blue),
            rect(whole, [1.0, 1.0, 9.0, 9.0], [0, 0, 0, 0]),
            // A clip larger than the frame that cuts nothing.
            rect([-5.0, -5.0, 25.0, 25.0], [2.0, 2.0, 10.0, 10.0], red),
            // Red on the red rect.
            rect(whole, [4.0, 4.0, 8.5, 8.5], [200, 0, 0, 90]),
            // A clip that cuts the rect, kept.
            rect([0.0, 0.0, 16.0, 20.0], [12.0, 1.0, 19.0, 6.0], blue),
            turned_by_nothing,
        ],
    };
    let before = rasterize(&frame, background, |_| None).unwrap();

    frame.tidy();

    assert_eq!(rasterize(&frame, background, |_| None).unwrap(), before);
    let kept: Vec<([f32; 4], [f32; 4], Turn)> = frame
        .primitives
        .iter()
        .map(|primitive| match &primitive.content {
            Content::RoundedRect(shape) => (primitive.clip, shape.rect, shape.turn),
            other => panic!("only rects were drawn, not {other:?}"),
        })
        .collect();
    assert_eq!(
        kept,
        [
            (whole, [2.0, 2.0, 10.0, 10.0], Turn::NONE),
            ([0.0, 0.0, 16.0, 20.0], [12.0, 1.0, 19.0, 6.0], Turn::NONE),
            (whole, [14.0, 14.0, 18.0, 18.0], Turn::NONE),
        ]
    );
}
