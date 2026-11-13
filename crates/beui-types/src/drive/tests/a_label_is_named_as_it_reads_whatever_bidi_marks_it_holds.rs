use super::*;

#[test]
fn a_label_is_named_as_it_reads_whatever_bidi_marks_it_holds() {
    let lines = [describe(
        0,
        Description {
            role: "AlertDialog",
            label: Some("Leave \u{201c}\u{2068}Team\u{2069}\u{201d}?"),
            value: None,
            toggled: None,
            disabled: false,
            selected: false,
            focused: false,
            test_ids: Vec::new(),
            bounds: None,
            clip: Rect::NOTHING,
        },
        (),
    )];
    assert_eq!(lines[0].text, "AlertDialog \"Leave \u{201c}Team\u{201d}?\"");
    for named in [
        "AlertDialog \"Leave \u{201c}Team\u{201d}?\"",
        "AlertDialog \"Leave \u{201c}\u{2068}Team\u{2069}\u{201d}?\"",
        "Leave \u{201c}Te\u{200b}am",
    ] {
        assert!(waited(&lines, named, None).is_some(), "{named:?}");
        assert!(
            crate::target::line_with(named, &lines).is_ok(),
            "{named:?}"
        );
    }
}
