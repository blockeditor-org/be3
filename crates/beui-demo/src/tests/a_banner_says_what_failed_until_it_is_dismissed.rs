use super::*;

#[test]
fn a_banner_says_what_failed_until_it_is_dismissed() {
    let mut test = alone(WIDE, Page::Overlays);
    reveal(&mut test, "demo.banner.dismiss");
    test.snapshot("a_banner_says_what_failed_until_it_is_dismissed");
    test.click("demo.banner.dismiss");
    test.frame(Vec::new());
    assert!(!test.shown("demo.banner"));
    test.click("demo.banner.again");
    test.frame(Vec::new());
    assert!(test.shown("demo.banner"));
}
