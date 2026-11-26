use super::*;

#[test]
fn it_is_an_alert_centred_over_its_window() {
    let _root = RootScope::new();
    let open = create_signal(true);
    let outcome = create_signal("");
    let mut test = DocumentTest::new(prompt(open, outcome), WIDE);
    test.frame(Vec::new());
    let prompt = test.rect_of("keep-changes");
    assert!(
        (prompt.center().x - WIDE.x / 2.0).abs() < 1.0
            && (prompt.center().y - WIDE.y / 2.0).abs() < 1.0,
        "the prompt is centred over its window: {prompt:?}"
    );
    test.snapshot("keep_changes_alert");
}
