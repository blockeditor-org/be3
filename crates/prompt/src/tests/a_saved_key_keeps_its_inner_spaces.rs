use super::*;

#[test]
fn a_saved_key_keeps_its_inner_spaces() {
    let checkout = std::env::temp_dir().join(format!("prompt-saved-{}", std::process::id()));
    std::fs::create_dir_all(&checkout).unwrap();
    std::fs::write(checkout.join(".prompt-test-key"), "  Bearer abc def\r\n").unwrap();
    let saved = Saved {
        variable: "BE3_PROMPT_TEST_KEY_THAT_IS_NEVER_SET",
        name: "prompt-test-key",
        what: "test key",
        checkout: Some(&checkout),
    };
    assert_eq!(saved.stored().as_deref(), Some("Bearer abc def"));
    std::fs::remove_dir_all(&checkout).unwrap();
}
