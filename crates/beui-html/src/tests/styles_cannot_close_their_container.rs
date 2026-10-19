use super::*;

#[test]
#[should_panic(expected = "closing style tag")]
fn styles_cannot_close_their_container() {
    stylesheet("</STYLE><p>injected");
}
