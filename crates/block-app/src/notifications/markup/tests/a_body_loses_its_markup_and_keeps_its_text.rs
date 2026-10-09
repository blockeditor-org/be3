use super::*;

#[test]
fn a_body_loses_its_markup_and_keeps_its_text() {
    assert_eq!(
        plain("<b>Mia</b> wrote:<br/>Fish &amp; chips at <i>noon</i>?"),
        "Mia wrote:\nFish & chips at noon?"
    );
    assert_eq!(
        plain(r#"<a href="https://example.com">a link</a> and <img src="x.png" alt="a cat"/>"#),
        "a link and a cat"
    );
    assert_eq!(plain("1 < 2 & 3 &#62; 2 &#x41;"), "1 < 2 & 3 > 2 A");
    assert_eq!(plain("&unknown; stays"), "&unknown; stays");
    assert_eq!(clip("héllo", 2), "h");
}
