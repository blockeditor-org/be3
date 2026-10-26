use super::*;

#[test]
fn a_new_scroll_soon_after_a_lift_scrolls_where_the_system_adds_no_momentum() {
    let gesture = [
        (0, TouchPhase::Started),
        (8, TouchPhase::Moved),
        (16, TouchPhase::Ended),
        (40, TouchPhase::Started),
        (48, TouchPhase::Moved),
        (56, TouchPhase::Ended),
    ];
    let start = Instant::now();
    assert_eq!(
        phases(&mut Wheel::new(false), start, &gesture),
        [
            Wheeled::StartAndScroll,
            Wheeled::Scroll,
            Wheeled::ScrollAndEnd,
            Wheeled::StartAndScroll,
            Wheeled::Scroll,
            Wheeled::ScrollAndEnd
        ],
        "without system momentum a quick second swipe is the fingers"
    );

    let mut wheel = Wheel::new(true);
    let slow = phases(
        &mut wheel,
        start,
        &[
            (0, TouchPhase::Started),
            (8, TouchPhase::Ended),
            (300, TouchPhase::Started),
            (308, TouchPhase::Moved),
        ],
    );
    assert_eq!(
        slow,
        [
            Wheeled::StartAndScroll,
            Wheeled::ScrollAndEnd,
            Wheeled::StartAndScroll,
            Wheeled::Scroll
        ],
        "fingers lifted without momentum leave the next swipe to scroll"
    );

    let mouse = phases(&mut wheel, start, &[(400, TouchPhase::Moved); 3]);
    assert_eq!(
        mouse,
        [Wheeled::Scroll; 3],
        "a wheel without phases always scrolls"
    );
}
