use super::*;

#[test]
fn choosing_a_scale_stores_it() {
    let mut settings = Harness::new();
    assert!(settings.says("Default (100%)"));

    settings.choose(SCALE, 5);
    assert_eq!(settings.content().root().scale_percent, Some(125));
    assert_eq!(settings.content().root().scale(), 1.25);

    settings.choose(SCALE, 0);
    assert_eq!(settings.content().root().scale_percent, None);
}
