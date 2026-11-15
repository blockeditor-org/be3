use super::*;

#[test]
fn a_held_clip_with_crlf_text_is_still_ours_when_read_back_with_lf() {
    let mut held = HeldClip::default();
    held.copy(Clip::text("one\r\ntwo\rthree"));
    assert!(
        held.wrote(Some("one\ntwo\nthree")),
        "text written with CRs reads back as ours once its line breaks are LF"
    );
    assert!(held.wrote(Some("one\r\ntwo\r\nthree")));
    assert!(
        !held.wrote(Some("one\ntwo")),
        "other text is another program's"
    );
    assert!(!held.wrote(None));
}
