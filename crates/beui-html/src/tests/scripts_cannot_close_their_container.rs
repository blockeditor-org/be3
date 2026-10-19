use super::*;

#[test]
#[should_panic(expected = "closing script tag")]
fn scripts_cannot_close_their_container() {
    script("module", "</ScRiPt><p>injected");
}
