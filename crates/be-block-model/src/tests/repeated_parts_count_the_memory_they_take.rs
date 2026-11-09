use crate::parts::{most_repeated, read_within};
use crate::{SPLICE, Stored, to_cbor};

#[test]
fn repeated_parts_count_the_memory_they_take() {
    assert_eq!(
        most_repeated(0),
        64 << 20,
        "a small document may still repeat 64 MiB"
    );
    assert_eq!(
        most_repeated(10 << 20),
        160 << 20,
        "a large one 16 times its size"
    );

    let small = to_cbor(&Stored::Array(vec![Stored::Integer(0.into()); 2_000]));
    let name = Stored::Bytes(be_block_store::Hash::of(&small).as_bytes().to_vec());
    let stream = |times: usize| {
        let root = to_cbor(&Stored::Tag(
            SPLICE,
            Box::new(Stored::Array(vec![name.clone(); times])),
        ));
        [root, small.clone()].concat()
    };
    let copy = 2_001 * std::mem::size_of::<Stored>();

    assert!(read_within(&stream(4), 4 * copy).is_ok());
    assert!(
        read_within(&stream(20), 4 * copy).is_err(),
        "a part of small numbers is a few KiB stored but far more in memory, and that is what counts"
    );
}
