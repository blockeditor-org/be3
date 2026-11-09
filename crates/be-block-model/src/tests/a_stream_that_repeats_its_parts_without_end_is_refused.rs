use crate::{PART, Stored, to_cbor};

fn name(part: &[u8]) -> Stored {
    Stored::Tag(
        PART,
        Box::new(Stored::Bytes(
            be_block_store::Hash::of(part).as_bytes().to_vec(),
        )),
    )
}

#[test]
fn a_stream_that_repeats_its_parts_without_end_is_refused() {
    let mut below = to_cbor(&Stored::Bytes(vec![7; 1024]));
    let mut parts = vec![below.clone()];
    for _ in 0..40 {
        let part = to_cbor(&Stored::Array(vec![name(&below), name(&below)]));
        parts.push(part.clone());
        below = part;
    }
    parts.reverse();
    assert!(
        crate::parts::read_within(&parts.concat(), 1 << 20).is_err(),
        "doubling at every level would join 2^40 KiB"
    );
}
