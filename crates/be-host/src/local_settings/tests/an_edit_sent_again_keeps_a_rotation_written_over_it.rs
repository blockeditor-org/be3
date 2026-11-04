use be_block::display_settings::Rotation;

use super::*;

#[test]
fn an_edit_sent_again_keeps_a_rotation_written_over_it() {
    let mut outbox = Outbox::default();
    outbox.queue(slow_mode());
    outbox.send(4, &showing(FAST));

    let mut turned = Document::new(&showing(FAST));
    turned.apply(&showing(FAST).set_rotation(MONITOR, Some(Rotation::Clockwise180)));
    let (read, resent) = outbox.overlay(5, turned.root());
    assert_eq!(
        read.mode(MONITOR),
        Some(SLOW),
        "the kept mode is read on top"
    );
    assert_eq!(
        read.rotation(MONITOR),
        Rotation::Clockwise180,
        "the rotation written to the same screen after the mode was sent is read"
    );

    for edit in &resent {
        turned.apply(edit);
    }
    assert_eq!(
        turned.root(),
        read,
        "sending the mode again leaves the rotation in the block"
    );
    assert_eq!(
        outbox.overlay(6, turned.root()),
        (read, Vec::new()),
        "once the block holds both, it is read as it is"
    );
}
