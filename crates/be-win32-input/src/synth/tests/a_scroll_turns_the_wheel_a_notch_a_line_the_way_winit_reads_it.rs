use super::*;

#[test]
fn a_scroll_turns_the_wheel_a_notch_a_line_the_way_winit_reads_it() {
    assert_eq!(wheel_units(ScrollDelta::Lines(vec2(0.0, 1.0))), (0, 120));
    assert_eq!(wheel_units(ScrollDelta::Lines(vec2(0.0, -2.5))), (0, -300));
    assert_eq!(wheel_units(ScrollDelta::Lines(vec2(1.0, 0.0))), (-120, 0));
    assert_eq!(
        wheel_units(ScrollDelta::Pixels(vec2(0.0, SCROLL_LINE / 2.0))),
        (0, 60)
    );
    let synths = pointer(
        DESKTOP,
        PixelPoint::new(5, 5),
        PointerAction::Scroll(ScrollDelta::Lines(vec2(-1.0, 1.0))),
    );
    let flags: Vec<_> = synths.iter().map(|synth| moved_to(*synth).2).collect();
    assert_eq!(
        flags,
        [
            MOUSE_MOVE | MOUSE_ABSOLUTE | MOUSE_VIRTUAL_DESK,
            MOUSE_WHEEL,
            MOUSE_HORIZONTAL_WHEEL
        ]
    );
    let Synth::Mouse { data, .. } = synths[2] else {
        unreachable!()
    };
    assert_eq!(data, 120);
}
