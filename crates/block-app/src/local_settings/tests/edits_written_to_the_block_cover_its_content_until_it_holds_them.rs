use super::*;

#[test]
fn edits_written_to_the_block_cover_its_content_until_it_holds_them() {
    let edit = DisplaySettings::set_mode(MONITOR, Some(SLOW));
    let mut outbox = Outbox::default();
    outbox.queue(vec![edit.clone()]);
    assert_eq!(outbox.send(4), vec![edit.clone()]);
    assert_eq!(outbox.send(4), Vec::new(), "an edit is sent once");

    assert_eq!(
        outbox.overlay(4, showing(FAST)),
        (showing(SLOW), Vec::new()),
        "the revision the edit was sent at is read with the edit on top"
    );
    assert_eq!(
        outbox.overlay(5, showing(FAST)),
        (showing(SLOW), vec![edit.clone()]),
        "a later revision without the edit is read with it on top, and it is sent again"
    );
    assert_eq!(
        outbox.overlay(6, showing(SLOW)),
        (showing(SLOW), Vec::new()),
        "a revision holding the edit is read as it is"
    );
    assert_eq!(
        outbox.overlay(7, showing(FAST)),
        (showing(FAST), Vec::new()),
        "after that, later changes in the block win"
    );
}
