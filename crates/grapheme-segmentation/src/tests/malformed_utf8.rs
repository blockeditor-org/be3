use crate::is_grapheme_boundary;

#[test]
fn malformed_utf8() {
    let cases: &[(&[u8], &[usize])] = &[
        (b"\xff\xcc\x81a", &[0, 3, 4]),
        (b"\xe0\x80\x80", &[0, 1, 2, 3]),
        (b"\xed\xa0\x80", &[0, 1, 2, 3]),
        (b"\xf4\x90\x80\x80", &[0, 1, 2, 3, 4]),
        (b"\xf0\x9f", &[0, 1, 2]),
        (b"\x80\xc3\xa9\x80", &[0, 1, 3, 4]),
        (b"\xe2\xc3\xa9", &[0, 1, 3]),
        (b"\xc3\xa9\xa9", &[0, 2, 3]),
        (b"\r\n\xff", &[0, 2, 3]),
        (b"", &[0]),
    ];
    for &(bytes, boundaries) in cases {
        for index in 0..=bytes.len() {
            assert_eq!(
                is_grapheme_boundary(bytes, index),
                boundaries.contains(&index),
                "{bytes:x?}, byte {index}"
            );
        }
    }
}
