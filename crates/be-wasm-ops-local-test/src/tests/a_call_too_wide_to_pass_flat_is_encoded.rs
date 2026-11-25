use super::*;

#[test]
fn a_call_too_wide_to_pass_flat_is_encoded() {
    let mut caller = Caller::new();
    let first: (u32, u32, u32, u32, u32, u32, u32, u32, u32, u32) = (1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
    let bytes = be_wasm_ops::encode(&first);
    let end = caller.put(0, &bytes);
    // The ten-word tuple is encoded (a pointer and a length), the eight-word one stays flat.
    let answered = embed::frames::glue::wide(
        &mut caller, 0, end, 11, 12, 13, 14, 15, 16, 17, 18, 19,
    );
    assert_eq!(answered, 19);
    assert_eq!(
        caller.host.wide,
        [((1..=18).collect::<Vec<u32>>(), 19)]
    );
    assert!(caller.refused.is_empty(), "{:?}", caller.refused);

    // The record holding a handle is the widest, so it is the one encoded, handle and all.
    let shape = embed::shapes::glue::make(&mut caller, 6);
    let framing = frames::Framing {
        shape: shapes::shapes::Shape(shape),
        sizes: (1, 2, 3, 4, 5, 6, 7, 8, 9),
    };
    let bytes = be_wasm_ops::encode(&framing);
    let end = caller.put(512, &bytes);
    let answered =
        embed::frames::glue::framed(&mut caller, 512, end - 512, 0, 0, 0, 0, 0, 0, 0, 10);
    assert_eq!(answered, 19);
    assert_eq!(caller.host.shown, [6], "the handle crossed inside the record");
    assert!(caller.refused.is_empty(), "{:?}", caller.refused);
}
