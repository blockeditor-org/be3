use super::*;

/// ChaCha20's keystream for the zero key and nonce, block 0 (RFC 7539, appendix A.1, test vector
/// 1), which guest.js's generator draws too.
const BLOCK_ZERO: &str = "76b8e0ada0f13d90405d6ae55386bd28bdd219b8a08ded1aa836efcc8b770dc7\
                          da41597c5157488d7724e03fb8d84a376a43b8f41518a11cc387b669b2ee6586";

#[test]
fn a_zero_seed_draws_chacha20s_own_stream() {
    let mut wasi = Wasi::new([0; 32]);
    let mut memory = Bytes(vec![0; 64]);
    let mut drawn = Vec::new();
    for (at, length) in [(0, 10), (10, 54)] {
        let answered = wasi.call("random_get", &[at, length], &mut memory, &mut |_| {});
        assert_eq!(answered, Ok(SUCCESS));
        drawn.extend_from_slice(&memory.0[at as usize..(at + length) as usize]);
    }
    let hex: String = drawn.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(hex, BLOCK_ZERO);
    assert!(matches!(
        wasi.call("random_get", &[60, 8], &mut memory, &mut |_| {}),
        Err(Stopped::Fault(_))
    ));
}
