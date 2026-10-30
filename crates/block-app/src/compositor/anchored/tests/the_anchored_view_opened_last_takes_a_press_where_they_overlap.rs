use super::*;

#[test]
fn the_anchored_view_opened_last_takes_a_press_where_they_overlap() {
    let menu = Rect::from_min_size(pos2(100.0, 100.0), vec2(100.0, 100.0));
    let popover = Rect::from_min_size(pos2(150.0, 150.0), vec2(100.0, 100.0));
    let mut layers = vec![shown(2, 1, 9, popover), shown(1, 1, 4, menu)];
    layers.sort_by_key(|shown| shown.anchored.opened);

    assert_eq!(
        topmost(&layers, pos2(175.0, 175.0)),
        Some((2, ViewId(1))),
        "where they overlap, the layer opened later is hit first, whichever region opened it"
    );
    assert_eq!(topmost(&layers, pos2(120.0, 120.0)), Some((1, ViewId(1))));
    assert_eq!(topmost(&layers, pos2(10.0, 10.0)), None);
}
