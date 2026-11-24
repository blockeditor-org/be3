use crate::target::line_holds;

#[test]
fn bare_text_finds_no_description_unless_it_names_one() {
    let line = r#"Button "More" description="Says \"more\"" #more at 0,0 size 10x10"#;
    assert!(line_holds(line, r#"Button "More""#));
    assert!(
        line_holds(line, r#"Button "More" #more"#),
        "a target may span the description"
    );
    assert!(
        !line_holds(line, "Says"),
        "a tooltip's text does not find its anchor"
    );
    assert!(line_holds(line, r#"description="Says \"more\"""#));
    assert!(line_holds(line, "size 10x10"));
}
