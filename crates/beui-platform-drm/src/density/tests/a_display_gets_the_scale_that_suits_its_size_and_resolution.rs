use super::*;

#[test]
fn a_display_gets_the_scale_that_suits_its_size_and_resolution() {
    let laptop_4k = natural_scale((3840, 2160), Some((344, 194)));
    let laptop_hidpi = natural_scale((2880, 1800), Some((302, 189)));
    let laptop_1080 = natural_scale((1920, 1080), Some((294, 165)));
    let monitor_4k = natural_scale((3840, 2160), Some((597, 336)));
    let monitor_1440 = natural_scale((2560, 1440), Some((597, 336)));
    let monitor_1080 = natural_scale((1920, 1080), Some((527, 296)));
    assert_eq!(laptop_4k, 2.5, "a 15.6 inch 4K laptop panel");
    assert_eq!(laptop_hidpi, 2.25, "a 14 inch 2880x1800 laptop panel");
    assert_eq!(laptop_1080, 1.5, "a 13.3 inch 1080p laptop panel");
    assert_eq!(monitor_4k, 1.5, "a 27 inch 4K monitor");
    assert_eq!(monitor_1440, 1.0, "a 27 inch 1440p monitor");
    assert_eq!(monitor_1080, 1.0, "a 24 inch 1080p monitor");
}
