use super::*;

#[test]
fn the_fingerprint_follows_the_bytes() {
    let hash = PaintSnapshot::fingerprint(&[7; 32]);
    assert_eq!(hash.len(), 64);
    assert_eq!(hash, PaintSnapshot::fingerprint(&[7; 32]));
    assert_ne!(hash, PaintSnapshot::fingerprint(&[7; 33]));
}
