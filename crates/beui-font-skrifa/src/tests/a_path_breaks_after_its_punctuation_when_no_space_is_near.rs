use super::*;

#[test]
fn a_path_breaks_after_its_punctuation_when_no_space_is_near() {
    let mut fonts = fonts();
    let start = "see ";
    let path = "/usr/local/share/applications/somewhere/deep_inside/the.tree:with,parts";
    let text = format!("{start}{path}");
    for width in [160.0, 200.0, 240.0, 280.0] {
        let lines = lines(&mut fonts, &text, width);
        assert!(lines.len() > 1, "at {width} the path wraps");
        let first = &lines[0];
        assert!(
            width - width_of(&mut fonts, start) > zone(),
            "at {width} the space lies before the zone"
        );
        assert!(
            first.ends_with(['/', '.', '_', ':', ',']),
            "at {width} the line breaks after punctuation in the path: {lines:?}"
        );
        assert!(
            width - width_of(&mut fonts, first) <= zone(),
            "at {width} the break lies inside the zone: {lines:?}"
        );
    }
    let call = "call_something(first_argument_value)";
    let width = width_of(&mut fonts, "call_something(first") + 5.0;
    let lines = lines(&mut fonts, &format!("{call}{call}"), width);
    assert!(
        lines[0].ends_with('_') || lines[1].starts_with('('),
        "a call breaks before its bracket or after an underscore: {lines:?}"
    );
}
