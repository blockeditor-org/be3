use super::*;

#[test]
fn word_around_selects_one_class_of_clusters_within_a_line() {
    let cases: &[(&str, &str)] = &[
        ("abc| def", "[abc] def"),
        ("abc |def", "abc [def]"),
        ("|abc", "[abc]"),
        ("abc|", "[abc]"),
        ("snake_c|ase", "[snake_case]"),
        ("don|'t", "don[']t"),
        ("caf|é au lait", "[café] au lait"),
        ("He\u{301}l|lo", "[He\u{301}llo]"),
        ("a .|.. b", "a [...] b"),
        ("a  |  b", "a[    ]b"),
        ("abc\n|\ndef", "abc\n[]\ndef"),
        ("abc |\ndef", "abc[ ]\ndef"),
        ("日本|語 text", "[日本語] text"),
    ];
    for (input, expected) in cases {
        let caret = input.find('|').unwrap();
        let text = input.replace('|', "");
        let (start, end) = word_around(text.as_bytes(), caret, |_| false);
        let actual = format!("{}[{}]{}", &text[..start], &text[start..end], &text[end..]);
        assert_eq!(&actual, expected, "{input:?}");
    }
}
