use crate::{grapheme_boundaries, is_grapheme_boundary};

fn agree(bytes: &[u8], what: &str) {
    let queried: Vec<usize> = (0..=bytes.len())
        .filter(|&index| is_grapheme_boundary(bytes, index))
        .collect();
    let walked: Vec<usize> = grapheme_boundaries(bytes).collect();
    assert_eq!(walked, queried, "{what}");
}

#[test]
fn walking_forward_finds_every_boundary_a_query_finds() {
    for (line_number, line) in include_str!("GraphemeBreakTest.txt").lines().enumerate() {
        let source = line.split('#').next().unwrap();
        let mut bytes = Vec::new();
        for token in source.split_whitespace() {
            if token != "÷" && token != "×" {
                let c = char::from_u32(u32::from_str_radix(token, 16).unwrap()).unwrap();
                bytes.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
            }
        }
        agree(&bytes, &format!("line {}", line_number + 1));
    }
    let malformed: &[&[u8]] = &[
        b"\xff\xcc\x81a",
        b"\xe0\x80\x80",
        b"\xed\xa0\x80",
        b"\xf4\x90\x80\x80",
        b"\xf0\x9f",
        b"\x80\xc3\xa9\x80",
        b"\xe2\xc3\xa9",
        b"\xc3\xa9\xa9",
        b"\r\n\xff",
        b"",
    ];
    for bytes in malformed {
        agree(bytes, &format!("{bytes:x?}"));
    }
    let extends = "\u{301}".repeat(64);
    for text in [
        format!("a\n👩{extends}\u{200d}👩z"),
        format!("a{extends}\u{200d}👩"),
        format!("क{extends}्{extends}क"),
        format!("क{extends}क"),
        format!("a{}z{}", "🇦".repeat(65), "🇦".repeat(64)),
        "Wrap /usr/local and 日本語のテキスト, then 👩‍👩‍👧 and e\u{301}.".to_owned(),
    ] {
        agree(text.as_bytes(), &text);
    }
}
