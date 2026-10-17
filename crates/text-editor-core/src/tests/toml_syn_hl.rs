use super::*;

#[test]
fn toml_syn_hl() {
    let mut tester = EditorTester::with_language(
        "# remaps\n[[group]]\nname = \"Cairn\" # game\nrules = [{ on = \"Q\", hold_ms = 40, keep = true }]\nx = [1.5, -2, 1979-05-27T07:32:00Z, 'lit']",
        TextLanguage::Toml,
    );
    assert_eq!(
        rendered_highlight(&mut tester, 0),
        concat!(
            "<comment># remaps\n",
            "<punctuation>[[<keyword_storage>group<punctuation>]]\n",
            "<variable>name <punctuation_important>= <punctuation>\"<literal_string>Cairn<punctuation>\" <comment># game\n",
            "<variable>rules <punctuation_important>= <punctuation>[{ <variable>on <punctuation_important>= <punctuation>\"<literal_string>Q<punctuation>\", <variable>hold_ms <punctuation_important>= <literal>40<punctuation>, <variable>keep <punctuation_important>= <literal>true <punctuation>}]\n",
            "<variable>x <punctuation_important>= <punctuation>[<literal>1.5<punctuation>, <literal>-2<punctuation>, <literal>1979-05-27T07:32:00Z<punctuation>, '<literal_string>lit<punctuation>']",
        )
    );
}
