use super::*;

#[test]
fn turning_every_display_off_keeps_the_main_one_lit() {
    let off = [(LAPTOP, DisplayUse::Off), (LEFT, DisplayUse::Off)];
    assert_eq!(
        plan(&[LAPTOP, LEFT], &config(&off, Some(LEFT))),
        [(1, DisplayUse::Desktop), (0, DisplayUse::Off)]
    );
    assert_eq!(
        plan(&[LAPTOP, LEFT], &config(&off, None)),
        [(0, DisplayUse::Desktop), (1, DisplayUse::Off)],
        "with no main display, the first one found"
    );
    assert_eq!(
        plan(&[LAPTOP], &config(&off, None)),
        [(0, DisplayUse::Desktop)]
    );
    assert_eq!(
        plan(
            &[LAPTOP, LEFT],
            &config(
                &[
                    (LAPTOP, DisplayUse::Mirror(LEFT.to_owned())),
                    (LEFT, DisplayUse::Mirror(LAPTOP.to_owned()))
                ],
                None
            )
        ),
        [(0, DisplayUse::Desktop), (1, DisplayUse::Mirror(LAPTOP.to_owned()))],
        "two displays mirroring each other leave the first on the desktop"
    );
}
