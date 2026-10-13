use super::*;

#[test]
fn positions_in_a_large_buffer_resolve_back_to_their_index() {
    let text: String = (0..20_000).map(|line| format!("line {line}\n")).collect();
    let buffer = TextBuffer::new(&text);
    let read = buffer.read().unwrap();
    let anchors: Vec<_> = (0..50)
        .map(|step| read.anchor(step * 1_000).unwrap())
        .collect();
    for (step, anchor) in anchors.iter().enumerate() {
        assert_eq!(read.anchor_index(*anchor), Some(step * 1_000));
    }
    assert_eq!(read.anchor(text.len()), None);
    assert_eq!(read.anchor(0), Some(anchors[0]));
}
