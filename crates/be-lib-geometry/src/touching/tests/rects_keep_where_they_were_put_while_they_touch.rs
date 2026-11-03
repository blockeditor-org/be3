use super::*;

#[test]
fn rects_keep_where_they_were_put_while_they_touch() {
    let rects = arrange(&[
        (vec2(1920.0, 1080.0), Some(pos2(1280.0, 0.0))),
        (vec2(1280.0, 800.0), Some(pos2(0.0, 500.0))),
        (vec2(1920.0, 1080.0), Some(pos2(1280.0, 1080.0))),
    ]);
    assert_eq!(
        rects,
        [
            Rect::from_min_size(pos2(1280.0, 0.0), vec2(1920.0, 1080.0)),
            Rect::from_min_size(pos2(0.0, 500.0), vec2(1280.0, 800.0)),
            Rect::from_min_size(pos2(1280.0, 1080.0), vec2(1920.0, 1080.0)),
        ],
        "a rect left of the first and one below it stay put"
    );
    assert_eq!(
        arrange(&[
            (vec2(100.0, 100.0), Some(pos2(500.0, 500.0))),
            (vec2(100.0, 100.0), Some(pos2(500.0, 400.0))),
        ]),
        [
            Rect::from_min_size(pos2(0.0, 100.0), vec2(100.0, 100.0)),
            Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)),
        ],
        "the whole arrangement moves so its corner is the origin"
    );
}
