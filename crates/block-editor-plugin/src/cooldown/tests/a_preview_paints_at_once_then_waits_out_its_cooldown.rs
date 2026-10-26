use super::*;

#[test]
fn a_preview_paints_at_once_then_waits_out_its_cooldown() {
    let mut cooldown = Cooldown::default();
    let start = Duration::from_secs(3);
    assert_eq!(
        cooldown.wait(7, start),
        None,
        "a preview never painted paints at once"
    );
    cooldown.painted(7, start, false);
    assert_eq!(
        cooldown.wait(7, start + Duration::from_millis(200)),
        Some(PREVIEW_COOLDOWN - Duration::from_millis(200)),
        "a change soon after waits for the rest of the cooldown"
    );
    assert_eq!(
        cooldown.wait(8, start),
        None,
        "another preview has a cooldown of its own"
    );
    assert_eq!(
        cooldown.wait(7, start + PREVIEW_COOLDOWN),
        None,
        "once the cooldown is over the preview paints again"
    );
    cooldown.keep(|surface| surface != 7);
    assert_eq!(
        cooldown.wait(7, start),
        None,
        "a surface that went away forgets when it was painted"
    );
}
