use super::*;

#[test]
fn options_of_lists_and_strings_pass_flat() {
    let mut caller = Caller::new();
    caller.put(0, b"square");
    let offsets: Vec<u8> = [3u32, 4]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    caller.put(16, &offsets);
    caller.put(32, &[9, 8, 7]);
    // Each option is its tag, then its pointer and length.
    let named = embed::shapes::glue::name(&mut caller, 1, 0, 6, 1, 16, 2, 1, 32, 3);
    assert_eq!(named, 1);
    let absent = embed::shapes::glue::name(&mut caller, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    assert_eq!(absent, 2);
    assert_eq!(
        caller.host.named,
        [
            (
                Some("square".to_owned()),
                Some(vec![3, 4]),
                Some(vec![9, 8, 7])
            ),
            (None, None, None),
        ]
    );
    assert!(caller.refused.is_empty(), "{:?}", caller.refused);
}
