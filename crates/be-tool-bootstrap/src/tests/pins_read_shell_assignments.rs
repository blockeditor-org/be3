use super::*;

#[test]
fn pins_read_shell_assignments() {
    let pins = Pins::parse("# a comment\nbuck2_version='2026-09-15'\nbe_sha256_x='' \nempty=\n");
    assert_eq!(
        pins.get("buck2_version").map(String::as_str),
        Some("2026-09-15")
    );
    assert_eq!(pins.get("be_sha256_x").map(String::as_str), Some(""));
    assert_eq!(pins.len(), 3);
}
