use crate::unpinned::{pin_name, pinned};

#[test]
fn unpinned_compares_with_the_pins_lines() {
    assert_eq!(
        pin_name("x86_64-unknown-linux-musl"),
        "bootstrap_sha256_x86_64_unknown_linux_musl"
    );
    let pins = "# pins\nbootstrap_sha256_a_b='abc'\nother='def'\n";
    assert!(pinned(pins, "bootstrap_sha256_a_b", "abc"));
    assert!(!pinned(pins, "bootstrap_sha256_a_b", "ab"));
    assert!(!pinned(pins, "other", "abc"));
}
