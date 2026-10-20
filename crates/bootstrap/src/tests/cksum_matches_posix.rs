use super::*;

#[test]
fn cksum_matches_posix() {
    assert_eq!(server::relay::cksum(b""), 4_294_967_295);
    assert_eq!(server::relay::cksum(b"123456789"), 930_766_865);
}
