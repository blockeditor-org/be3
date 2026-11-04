use super::*;

#[test]
fn a_viewport_destination_is_the_size_the_surface_is_drawn_at() {
    let at = pane(0.0, 0.0, 400.0, 300.0, 1.0);
    assert_eq!(
        drawn(
            surface(200.0, 100.0),
            surface(100.0, 50.0),
            (100.0, 50.0),
            &at
        ),
        Drawn {
            pixels: [0.0, 0.0, 200.0, 100.0],
            uv: [0.0, 0.0, 1.0, 1.0],
        },
        "the whole buffer fills the destination the client asked for"
    );
    assert_eq!(
        drawn(
            surface(200.0, 100.0),
            Rectangle::new((50.0, 0.0).into(), (50.0, 25.0).into()),
            (100.0, 50.0),
            &at
        ),
        Drawn {
            pixels: [0.0, 0.0, 200.0, 100.0],
            uv: [0.5, 0.0, 1.0, 0.5],
        },
        "a source rectangle picks that part of the buffer"
    );
}
