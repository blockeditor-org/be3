use block_type_api::{
    Failure, Merged, Plain, PlainFile, embed::exports::replicas, guest::exports::replicas::Guest,
    ops::OpError,
};

use super::*;

crate::be_block_types![ChecklistContent { undo }, FileContent { file }];

#[test]
fn a_module_answers_stateless_requests() {
    let mut outbound = Vec::new();
    let mut guest =
        <BlockTypes as crate::abi::Guest>::start(crate::abi::Waker::new(|| {}), &mut outbound)
            .unwrap();
    assert!(outbound.is_empty());
    let mut host = Connection::new(Side::Host);
    let started = replicas::start(&mut host, vec![5; 32]);
    assert_eq!(
        crate::abi::Guest::step(&mut guest, host.take_outbound(), false, &mut outbound),
        Ok(crate::abi::Flow::Running)
    );
    let mut types = None;
    for frame in outbound.drain(..) {
        host.receive(&frame).unwrap();
    }
    while let Some(event) = host.next_event() {
        if let Incoming::Completed { op, outcome } = event
            && op == started.op()
        {
            types = Some(started.decode(outcome).unwrap());
        }
    }
    let types = types.expect("start lists the module's types");
    assert_eq!(types.len(), 2);
    assert!(types[0].undo && types[0].plain == Plain::None);
    assert!(!types[1].undo && types[1].plain == Plain::File);

    let module = block_types();
    assert_eq!(
        pollster::block_on(module.blank(ChecklistContent::CONTENT_TYPE)),
        Ok(ChecklistContent::default().encode())
    );
    let mut ours = ChecklistContent::default();
    ours.apply(&add("ours"));
    let mut theirs = ChecklistContent::default();
    theirs.apply(&add("theirs"));
    let Ok(Merged { bytes, conflicts }) = pollster::block_on(module.merge3(
        ChecklistContent::CONTENT_TYPE,
        None,
        Some(ours.encode()),
        Some(theirs.encode()),
    )) else {
        panic!("a merge");
    };
    assert_eq!(conflicts, 0);
    assert_eq!(texts(&ChecklistContent::decode(&bytes).unwrap()).len(), 2);
    let description = pollster::block_on(module.describe(
        ChecklistContent::CONTENT_TYPE,
        ours.encode(),
        ORGANIZATION,
    ))
    .expect("a description");
    assert_eq!(description.derived, be_block::derived_metadata(&ours));
    let text = FileContent::new("notes.txt".into(), b"hello".to_vec());
    assert_eq!(
        pollster::block_on(module.to_file(FileContent::CONTENT_TYPE, text.encode())),
        Ok(Some(PlainFile {
            bytes: b"hello".to_vec(),
            name: Some("notes.txt".to_owned()),
            extension: "txt".to_owned(),
            media_type: "application/octet-stream".to_owned(),
        }))
    );
    assert_eq!(
        pollster::block_on(module.to_file(ChecklistContent::CONTENT_TYPE, ours.encode())),
        Ok(None)
    );
    assert_eq!(
        pollster::block_on(module.claims("notes.txt".into(), b"hello".to_vec(), false, true)),
        Some(FileContent::CONTENT_TYPE)
    );
    assert_eq!(
        pollster::block_on(module.claims("photos".into(), Vec::new(), true, false)),
        None
    );
    assert_eq!(
        pollster::block_on(module.blank(Uuid::nil())),
        Err(Failure::UnknownType)
    );

    let content = replicas::Replica::from_op(9).content(&mut host);
    crate::abi::Guest::step(&mut guest, host.take_outbound(), false, &mut outbound).unwrap();
    for frame in outbound.drain(..) {
        host.receive(&frame).unwrap();
    }
    assert_eq!(
        host.next_event(),
        Some(Incoming::Completed {
            op: content.op(),
            outcome: Outcome::Err(OpError::Canceled),
        })
    );
}
