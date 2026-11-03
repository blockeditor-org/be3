use super::*;

#[test]
fn the_main_display_comes_first_and_those_off_or_mirroring_last() {
    let ids = [LAPTOP, LEFT, RIGHT];
    assert_eq!(
        plan(&ids, &DisplayConfig::default()),
        [
            (0, DisplayUse::Desktop),
            (1, DisplayUse::Desktop),
            (2, DisplayUse::Desktop)
        ],
        "with nothing chosen every display is on the desktop, in the order they were found"
    );
    assert_eq!(
        plan(&ids, &config(&[], Some(RIGHT))),
        [
            (2, DisplayUse::Desktop),
            (0, DisplayUse::Desktop),
            (1, DisplayUse::Desktop)
        ]
    );
    assert_eq!(
        plan(
            &ids,
            &config(
                &[
                    (LAPTOP, DisplayUse::Off),
                    (LEFT, DisplayUse::Mirror(RIGHT.to_owned()))
                ],
                None
            )
        ),
        [
            (2, DisplayUse::Desktop),
            (1, DisplayUse::Mirror(RIGHT.to_owned())),
            (0, DisplayUse::Off)
        ]
    );
    assert_eq!(
        plan(&ids, &config(&[(RIGHT, DisplayUse::Off)], Some(RIGHT))),
        [
            (0, DisplayUse::Desktop),
            (1, DisplayUse::Desktop),
            (2, DisplayUse::Off)
        ],
        "a main display turned off gives way to the first one on"
    );
}
