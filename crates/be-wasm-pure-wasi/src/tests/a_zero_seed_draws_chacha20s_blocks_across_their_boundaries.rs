use super::*;

#[test]
fn a_zero_seed_draws_chacha20s_blocks_across_their_boundaries() {
    // 10 and 54 end at block 0's end; 60 and 8 cross block 1's start; 60 more end inside it.
    let drawn = drawn([0; 32], &[10, 54, 60, 8, 60]);
    assert_eq!(vectors::hex(&drawn[..64]), vectors::ZERO_BLOCK_0);
    assert_eq!(vectors::hex(&drawn[64..128]), vectors::ZERO_BLOCK_1);
}
