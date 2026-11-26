use super::*;

#[test]
fn fuzzed_inputs_never_panic_and_built_values_read_back() {
    for seed in 0..2000u64 {
        let data = noise(seed, (seed % 300) as usize);
        crate::fuzz::round_trip(&data);
        crate::fuzz::decode_any(&data);
        // A valid encoding, cut short and with a byte flipped, is the input most likely to
        // reach deep into a decoder.
        let mut valid = bytes(&crate::fuzz::FuzzRecord::built(&data));
        crate::fuzz::decode_any(&valid);
        if !valid.is_empty() {
            let at = (seed as usize * 7919) % valid.len();
            valid[at] ^= 1 << (seed % 8);
            crate::fuzz::decode_any(&valid);
            valid.truncate(at);
            crate::fuzz::decode_any(&valid);
        }
    }
}
