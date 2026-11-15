//! A CPU benchmark guest for ../v8 and ../wasmtime (results in ../FINDINGS.md).

use std::collections::HashMap;
/// Mixed CPU work: hashing, a hash map, sorting, float maths.
#[unsafe(no_mangle)]
pub extern "C" fn run(rounds: u32) -> u64 {
    let mut acc = 0u64;
    for round in 0..rounds {
        let mut map: HashMap<u64, u64> = HashMap::new();
        let mut v: Vec<u64> = Vec::with_capacity(20000);
        let mut x = 0x9E3779B97F4A7C15u64 ^ round as u64;
        for i in 0..20000u64 {
            x ^= x << 13; x ^= x >> 7; x ^= x << 17;
            v.push(x);
            *map.entry(x % 4096).or_default() += i;
        }
        v.sort_unstable();
        let mut f = 0.0f64;
        for (i, y) in v.iter().enumerate() { f += ((*y as f64).sqrt() * i as f64).sin(); }
        acc = acc.wrapping_add(v[v.len() / 2]).wrapping_add(map.len() as u64).wrapping_add(f.to_bits());
    }
    acc
}
