use super::*;

#[test]
fn bytes_a_reader_and_a_hasher_agree() {
    let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    assert_eq!(sha256(b"abc"), abc);
    assert_eq!(sha256_read(&b"abc"[..]).unwrap(), (abc.to_owned(), 3));
    let mut hasher = Sha256::new();
    hasher.update(b"a");
    hasher.update(b"bc");
    assert_eq!(finish(hasher), abc);
}
