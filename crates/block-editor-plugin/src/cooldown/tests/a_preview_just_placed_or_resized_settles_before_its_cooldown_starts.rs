use super::*;

#[test]
fn a_preview_just_placed_or_resized_settles_before_its_cooldown_starts() {
    let mut cooldown = Cooldown::default();
    let start = Duration::from_secs(3);
    let soon = start + Duration::from_millis(100);
    cooldown.painted(7, start, true);
    assert_eq!(
        cooldown.wait(7, soon),
        None,
        "what a new size settles into paints at once"
    );
    cooldown.painted(7, soon, false);
    assert_eq!(
        cooldown.wait(7, soon + Duration::from_millis(100)),
        None,
        "it keeps painting at once until the cooldown since the new size has passed"
    );
    let settled = start + PREVIEW_COOLDOWN;
    cooldown.painted(7, settled, false);
    assert_eq!(
        cooldown.wait(7, settled + Duration::from_millis(100)),
        Some(PREVIEW_COOLDOWN - Duration::from_millis(100)),
        "once settled, the next change starts the cooldown"
    );
}
