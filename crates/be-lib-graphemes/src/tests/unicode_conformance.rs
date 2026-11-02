use crate::is_grapheme_boundary;

#[test]
fn unicode_conformance() {
    let mut cases = 0;
    for (line_number, line) in include_str!("GraphemeBreakTest.txt").lines().enumerate() {
        let source = line.split('#').next().unwrap();
        let mut bytes = Vec::new();
        let mut boundaries = Vec::new();
        for token in source.split_whitespace() {
            match token {
                "÷" => boundaries.push(bytes.len()),
                "×" => {}
                _ => {
                    let c = char::from_u32(u32::from_str_radix(token, 16).unwrap()).unwrap();
                    bytes.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
                }
            }
        }
        if boundaries.is_empty() {
            continue;
        }
        cases += 1;
        for index in 0..=bytes.len() {
            assert_eq!(
                is_grapheme_boundary(&bytes, index),
                boundaries.contains(&index),
                "line {}, byte {index}",
                line_number + 1
            );
        }
    }
    assert!(cases > 700);
}
