use super::*;

#[test]
fn a_line_inserted_with_text_starts_from_fresh_positions() {
    let mut document = note("");
    let line = document.root().lines[0].id;
    let words = document
        .text(line, Line::WORDS)
        .expect("the line has words");
    let typing = Line::WORDS.edit(
        line,
        words
            .insert(ALICE, 5, b"!".to_vec())
            .expect("there is something to insert"),
    );
    document.apply(&typing.into());
    let removal: Edit = Change::remove(line).into();
    let step = document
        .step(&removal)
        .expect("the removal changes something");
    document.apply(&removal);
    let mut peer = document.clone();

    let undo = step.undo();
    let sent: Edit = postcard::from_bytes(&postcard::to_stdvec(&undo).expect("the edit encodes"))
        .expect("the edit decodes");
    document.apply(&undo);
    peer.apply(&sent);

    let restored = document.text(line, Line::WORDS).expect("the line is back");
    assert!(restored.is_fresh());
    assert_eq!(restored.items(), b"first!");
    assert_eq!(
        peer.text(line, Line::WORDS)
            .expect("the line is back")
            .pos(5),
        restored.pos(5)
    );
}
