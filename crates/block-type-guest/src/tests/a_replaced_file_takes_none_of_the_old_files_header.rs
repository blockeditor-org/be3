use super::*;

use be_block::blob::BlobOp;

#[test]
fn a_replaced_file_takes_none_of_the_old_files_header() {
    let mut sim = Sim::<FileContent>::new(2, file_module);
    sim.publish_elsewhere(&FileContent::new("old.png".into(), vec![1; 64]));
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.operate(0, 1, BlobOp::SetHeader("old.png 800x600".into()));
    sim.settle();
    let replacement = FileContent::new("new.png 1920x1080".into(), vec![2; 128]);
    sim.send_from_editor(1, 1, FromEditor::Replace(replacement.encode()));
    sim.settle();
    assert_eq!(sim.head_content(), replacement);
    for device in 0..2 {
        assert_eq!(sim.content(device), replacement);
        assert_eq!(sim.mirror(device, 1), &replacement);
    }
    assert!(sim.kept(|content| content.header() == "old.png 800x600"));
    assert!(
        sim.take_received(0, 1)
            .iter()
            .any(|message| matches!(message, ToEditor::Snapshot(snapshot) if snapshot.bytes == replacement.encode()))
    );
}
