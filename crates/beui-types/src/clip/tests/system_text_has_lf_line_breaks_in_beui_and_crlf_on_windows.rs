use super::*;

#[test]
fn system_text_has_lf_line_breaks_in_beui_and_crlf_on_windows() {
    assert_eq!(
        text_from_system("one\r\ntwo\rthree\nfour\r\n".to_owned()),
        "one\ntwo\nthree\nfour\n",
        "a CRLF and a lone CR each become one LF"
    );
    assert_eq!(
        text_from_system("\r\r\n\n".to_owned()),
        "\n\n\n",
        "a CR before a CRLF is a line break of its own"
    );
    assert_eq!(text_from_system("plain".to_owned()), "plain");
    assert_eq!(text_with_crlf("one\ntwo\n"), "one\r\ntwo\r\n");
    assert_eq!(
        text_with_crlf("one\r\ntwo"),
        "one\r\ntwo",
        "a CRLF already there does not double its CR"
    );
    let text = "one\ntwo\n\nthree";
    assert_eq!(
        text_from_system(text_with_crlf(text)),
        text,
        "a copy to Windows pastes back as it was"
    );
}
