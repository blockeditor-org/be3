use super::*;

#[test]
fn refresh_rates_keep_their_fractions() {
    let timing = |clock_khz, htotal, vtotal| Timing {
        clock_khz,
        htotal,
        vtotal,
        vscan: 0,
        interlaced: false,
        double_scan: false,
    };
    assert_eq!(refresh_millihertz(&timing(148_500, 2200, 1125)), 60_000);
    assert_eq!(refresh_millihertz(&timing(148_352, 2200, 1125)), 59_940);
    assert_eq!(refresh_millihertz(&timing(645_000, 2720, 1481)), 160_116);
    assert_eq!(
        refresh_millihertz(&Timing {
            interlaced: true,
            ..timing(74_250, 2200, 1125)
        }),
        60_000
    );
    assert_eq!(refresh_millihertz(&timing(148_500, 0, 1125)), 0);
}
