use super::*;

#[test]
fn outside_a_session_input_is_not_offered() {
    let mut elsewhere = open(EditorHost::default());
    assert!(
        !elsewhere.shown("input.layout"),
        "only a Linux session takes input settings"
    );
    assert!(!elsewhere.shown("input.remaps"));
    elsewhere.snapshot("outside_a_session_input_is_not_offered");

    let session = editor(Vec::new());
    assert!(session.shown("input.layout"));
    assert!(session.shown("input.remaps"));
}
