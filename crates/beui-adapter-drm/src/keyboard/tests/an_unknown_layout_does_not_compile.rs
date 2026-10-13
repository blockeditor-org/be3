use super::*;

#[test]
fn an_unknown_layout_does_not_compile() {
    let unknown = InputConfig {
        layout: "no-such-layout".to_owned(),
        ..InputConfig::default()
    };

    assert!(Keyboard::new(&unknown).is_none());
}
