use super::*;

#[test]
fn a_saved_mode_wins_while_the_monitor_offers_it() {
    let candidates = gaming_monitor();
    assert_eq!(choose(&candidates, Some(mode(1920, 1080, 60_000))), Some(4));
    assert_eq!(choose(&candidates, Some(mode(3840, 2160, 30_000))), Some(3));
    assert_eq!(
        choose(&candidates, Some(mode(5120, 2880, 60_000))),
        Some(1),
        "a mode the monitor does not offer falls back to the default"
    );
    assert_eq!(
        choose(&candidates, Some(mode(1920, 1080, 120_000))),
        Some(1),
        "interlaced modes are not offered"
    );

    assert_eq!(
        listed(&candidates),
        vec![
            mode(3840, 2160, 30_000),
            mode(2560, 1440, 239_970),
            mode(2560, 1440, 143_998),
            mode(2560, 1440, 59_951),
            mode(1920, 1080, 60_000),
        ],
        "the list is largest first, fastest first, once each"
    );
}
