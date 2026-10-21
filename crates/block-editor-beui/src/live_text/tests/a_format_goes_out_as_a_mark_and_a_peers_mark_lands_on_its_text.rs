use super::*;

#[test]
fn a_format_goes_out_as_a_mark_and_a_peers_mark_lands_on_its_text() {
    let mut content = note("hello world");
    let mine = live(&content);
    assert_eq!(
        mine.read().map(|read| read.language()),
        Some(text_editor_core::TextLanguage::RichText)
    );

    mine.edit(Vec::new(), &mut |edit| {
        edit.format(0..5, &change(BOLD, Some(Value::Bool(true))));
    });
    let sent = send(&mine, &mut content);
    assert!(
        matches!(
            sent.as_slice(),
            [be_block::Edit(changes)] if matches!(changes.as_slice(), [be_block::be_model::Change::Mark { .. }])
        ),
        "formatting is sent as one mark: {sent:?}"
    );
    assert!(formatted(&mine, 2, BOLD));
    let saved = content.root().words;
    assert_eq!(saved.seeds().len(), 1);

    let peer = live(&content);
    peer.edit(Vec::new(), &mut |edit| {
        edit.format(6..11, &change(ITALIC, Some(Value::Bool(true))));
    });
    for edit in send(&peer, &mut content) {
        assert!(mine.apply(&edit), "a mark on the text is applied in place");
    }
    assert!(mine.take_external_edit());
    assert!(formatted(&mine, 8, ITALIC));
    assert!(formatted(&mine, 2, BOLD));
}
