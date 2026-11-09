use super::*;
use crate::{PART, Stored, read_stored, to_cbor};

fn name(part: &[u8]) -> Stored {
    Stored::Tag(
        PART,
        Box::new(Stored::Bytes(
            be_block_store::Hash::of(part).as_bytes().to_vec(),
        )),
    )
}

fn stream(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

/// A chain of `depth` parts below the root, each naming the next, the last
/// holding bytes.
fn chain(depth: usize) -> Vec<u8> {
    let mut parts = vec![to_cbor(&Stored::Bytes(b"leaf".to_vec()))];
    for _ in 0..depth {
        let below = parts.last().expect("there is a part below");
        parts.push(to_cbor(&name(below)));
    }
    parts.reverse();
    parts.concat()
}

#[test]
fn stored_parts_are_found_by_place_and_checked() {
    let leaf = to_cbor(&Stored::Bytes(b"shared".to_vec()));
    let left = to_cbor(&Stored::Array(vec![name(&leaf)]));
    let right = to_cbor(&Stored::Array(vec![name(&leaf), name(&leaf)]));
    let root = to_cbor(&Stored::Array(vec![name(&left), name(&right)]));
    let shared = Stored::Bytes(b"shared".to_vec());
    assert_eq!(
        read_stored(&stream(&[&root, &left, &leaf, &right, &leaf])).ok(),
        Some(Stored::Array(vec![
            Stored::Array(vec![shared.clone()]),
            Stored::Array(vec![shared.clone(), shared]),
        ])),
        "a part named under two parents follows each, and one named twice by a part follows it once"
    );
    assert!(
        read_stored(&stream(&[&root, &right, &leaf, &left, &leaf])).is_err(),
        "parts out of their order are refused"
    );
    assert!(
        read_stored(&stream(&[&root, &left, &leaf, &right, &leaf, &leaf])).is_err(),
        "bytes after the last part are refused"
    );
    assert!(
        read_stored(&stream(&[&root, &left, &leaf, &right])).is_err(),
        "a missing part is refused"
    );

    assert!(read_stored(&chain(crate::parts::DEEPEST_PARTS - 1)).is_ok());
    assert!(
        read_stored(&chain(crate::parts::DEEPEST_PARTS)).is_err(),
        "parts nested past the limit are refused rather than recursed into"
    );

    let mut document = note("hello");
    typed_into(&mut document, 5, " world");
    let mut bytes = document.to_bytes();
    bytes.extend(to_cbor(&Stored::Null));
    assert!(Document::<Note>::from_bytes(&bytes).is_err());
}
