use super::*;

#[test]
fn a_be3_format_is_a_reverse_dns_pasteboard_type() {
    assert_eq!(pasteboard_type(formats::BLOCKS), "org.be3.clip.blocks");
    assert_eq!(
        pasteboard_type(formats::TEXT_FRAGMENT),
        "org.be3.clip.text-fragment"
    );
    assert_eq!(
        pasteboard_type("be3/odd name_1"),
        "org.be3.clip.odd-name-1",
        "only the characters a UTI allows"
    );
}
