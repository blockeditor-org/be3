use super::*;

#[test]
fn a_segment_read_is_checked_against_the_index() {
    let chat = closed(&long(120));
    let part = chat
        .parts()
        .flatten()
        .expect("a chat that holds its segments flattens");
    let header = chat.encode();
    let bytes = part[header.len()..].to_vec();

    let messages = chat
        .read_segment(0, &bytes)
        .expect("the part is the segment");
    assert_eq!(messages.len() as u64, chat.segments()[0].count);

    let mut forged = bytes.clone();
    let last = forged.len() - 1;
    forged[last] ^= 1;
    assert!(
        chat.read_segment(0, &forged).is_none(),
        "bytes that hash differently from the index are not the segment"
    );
    assert!(
        chat.read_segment(1, &bytes).is_none(),
        "and there is no second segment"
    );
}
