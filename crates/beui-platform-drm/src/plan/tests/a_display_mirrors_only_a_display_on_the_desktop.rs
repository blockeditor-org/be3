use super::*;

#[test]
fn a_display_mirrors_only_a_display_on_the_desktop() {
    let ids = [LAPTOP, LEFT, RIGHT];
    let mirrored = |of: &str| DisplayUse::Mirror(of.to_owned());
    assert_eq!(
        plan(
            &ids,
            &config(&[(LAPTOP, mirrored(LEFT)), (LEFT, DisplayUse::Off)], None)
        ),
        [
            (0, DisplayUse::Desktop),
            (2, DisplayUse::Desktop),
            (1, DisplayUse::Off)
        ],
        "a display mirroring one that is off shows the desktop itself"
    );
    assert_eq!(
        plan(&ids, &config(&[(LAPTOP, mirrored("DEL|P2419H|9"))], None)),
        [
            (0, DisplayUse::Desktop),
            (1, DisplayUse::Desktop),
            (2, DisplayUse::Desktop)
        ],
        "or one that is not connected"
    );
    assert_eq!(
        plan(
            &ids,
            &config(&[(LAPTOP, mirrored(LEFT)), (RIGHT, mirrored(LAPTOP))], None)
        ),
        [
            (1, DisplayUse::Desktop),
            (2, DisplayUse::Desktop),
            (0, mirrored(LEFT))
        ],
        "nor one that mirrors another"
    );
}
