use super::super::bootstrap::set;

#[test]
fn a_binary_the_pins_do_not_name_replaces_its_pin() {
    let pins = "# a comment\nbootstrap_sha256_x86_64_unknown_linux_gnu='old'\nbuck2_version='1'\n";
    let values = [(
        "bootstrap_sha256_x86_64_unknown_linux_gnu".to_owned(),
        "new".to_owned(),
    )];
    assert_eq!(
        set(pins, &values).unwrap(),
        "# a comment\nbootstrap_sha256_x86_64_unknown_linux_gnu='new'\nbuck2_version='1'\n"
    );
    let missing = [("bootstrap_sha256_riscv".to_owned(), "new".to_owned())];
    assert!(set(pins, &missing).is_err());
}
