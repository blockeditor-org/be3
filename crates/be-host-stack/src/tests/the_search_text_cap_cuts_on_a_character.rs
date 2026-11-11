use crate::search::{MAX_TEXT, capped};

#[test]
fn the_search_text_cap_cuts_on_a_character() {
    let short = "é".repeat(10);
    assert_eq!(capped(short.clone()), short);
    let exact = "a".repeat(MAX_TEXT);
    assert_eq!(capped(exact.clone()), exact);

    // Two-byte characters put the cap in the middle of one when it is odd; three-byte ones
    // when it is not a multiple of three.
    for text in [
        "é".repeat(MAX_TEXT),
        "漢".repeat(MAX_TEXT),
        format!("x{}", "é".repeat(MAX_TEXT)),
        format!("xx{}", "𝄞".repeat(MAX_TEXT)),
    ] {
        let cut = capped(text.clone());
        assert!(
            cut.len() <= MAX_TEXT && cut.len() >= MAX_TEXT - 3,
            "{}",
            cut.len()
        );
        assert!(text.starts_with(&cut));
    }
}
