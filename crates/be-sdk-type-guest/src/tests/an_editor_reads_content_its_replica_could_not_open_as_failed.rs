use super::*;
use crate::editor::{EditorMirror, Snapshot};

/// An editor hears that its replica could not open the content, or is sent a snapshot that does
/// not decode in this build: either way it reads the content as failed, with why, until a
/// snapshot that opens arrives.
#[test]
fn an_editor_reads_content_its_replica_could_not_open_as_failed() {
    let mut mirror = EditorMirror::<ChecklistContent>::default();
    assert_eq!(mirror.failure(), None);

    mirror.receive(ToEditor::Failed("its type module failed".to_owned()));
    assert_eq!(mirror.failure(), Some("its type module failed"));
    assert!(!mirror.is_loaded());
    let failed_at = mirror.revision();

    let snapshot = |bytes: Vec<u8>| {
        ToEditor::Snapshot(Snapshot {
            content_type: ChecklistContent::CONTENT_TYPE,
            length: bytes.len() as u64,
            bytes,
            session: Vec::new(),
            unsettled: Vec::new(),
            applied: 0,
        })
    };
    mirror.receive(snapshot(vec![0xff, 0x00, 0x13]));
    let failure = mirror.failure().map(str::to_owned).unwrap_or_default();
    assert!(
        failure.starts_with("The content could not be read"),
        "an undecodable snapshot is a failure too: {failure}"
    );
    assert!(
        mirror.revision() > failed_at,
        "readers hear of the new failure"
    );

    mirror.receive(snapshot(ChecklistContent::default().encode()));
    assert!(mirror.is_loaded());
    assert_eq!(
        mirror.failure(),
        None,
        "a snapshot that opens clears the failure"
    );
}
