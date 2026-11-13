use super::*;

#[test]
fn a_text_fragment_from_another_program_keeps_only_marks_that_fit_its_chunks() {
    let mark = |range: Range<usize>| TextMark {
        range,
        block: false,
        key: "bold".to_owned(),
        value: MarkValue::Bool(true),
    };
    let hostile = TextFragment {
        chunks: vec![
            TextChunk {
                bytes: b"bold".to_vec(),
                marks: vec![
                    mark(0..4),
                    mark(2..9),
                    mark(3..1),
                    mark(usize::MAX..usize::MAX),
                ],
            },
            TextChunk {
                bytes: b"x".to_vec(),
                marks: vec![mark(0..1)],
            },
        ],
        shape: TextShape::Chars,
        schema: Some("basic".to_owned()),
    };
    let decoded = hostile
        .encode()
        .decode::<TextFragment>()
        .expect("the fragment decodes");
    assert_eq!(
        decoded.chunks[0].marks,
        vec![mark(0..4)],
        "a mark past its chunk's end or ending before it starts is dropped"
    );
    assert_eq!(decoded.chunks[1].marks, vec![mark(0..1)]);
    assert_eq!(
        hostile.joined().marks[3].range,
        usize::MAX..usize::MAX,
        "joining an undecoded fragment never overflows"
    );
}
