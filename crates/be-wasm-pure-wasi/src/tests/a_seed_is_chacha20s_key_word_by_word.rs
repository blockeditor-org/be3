use super::*;

#[test]
fn a_seed_is_chacha20s_key_word_by_word() {
    let drawn = drawn(vectors::ONE, &[64, 64]);
    assert_eq!(vectors::hex(&drawn[64..]), vectors::ONE_BLOCK_1);
}
