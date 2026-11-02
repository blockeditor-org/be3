use super::*;

fn saved(id: Uuid, parent: Option<usize>) -> Saved {
    Saved {
        id,
        block_type: Uuid::nil(),
        parent,
        metadata: be_block::BlockMetadata::default(),
        references: Vec::new(),
        content: None,
    }
}

fn edit(block: Uuid) -> Command {
    Command::Operate(block, Vec::new(), std::time::Instant::now())
}

fn subjects(commands: Vec<Command>) -> Vec<Option<Uuid>> {
    commands.iter().map(Command::subject).collect()
}

#[test]
fn a_move_waits_until_everything_under_it_is_uploaded() {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let mut uploads = Uploads::new(&shared);
    let (destination, top, inner, leaf) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    uploads.add(vec![
        (saved(top, None), BlockParent::Detached),
        (saved(inner, Some(0)), BlockParent::Block(top)),
        (saved(leaf, Some(1)), BlockParent::Block(inner)),
    ]);
    assert!(
        uploads.defer(edit(inner)).is_none(),
        "a block still uploading takes no edits yet"
    );
    uploads.queue.borrow_mut().finish(top);
    assert!(uploads.runnable().is_empty());

    assert!(
        uploads.defer(edit(top)).is_some(),
        "an uploaded block takes edits"
    );
    assert!(
        uploads
            .defer(Command::SetParent {
                block: top,
                parent: BlockParent::Block(destination),
            })
            .is_none(),
        "a move rekeys everything below, so it waits for all of it"
    );
    assert!(
        uploads.defer(edit(destination)).is_some(),
        "the folder a block moves into is not held up by the move"
    );
    assert!(
        uploads.defer(edit(top)).is_none(),
        "the moved block's later edits stay behind its move"
    );

    uploads.queue.borrow_mut().finish(inner);
    assert_eq!(subjects(uploads.runnable()), vec![Some(inner)]);
    uploads.queue.borrow_mut().finish(leaf);
    assert_eq!(subjects(uploads.runnable()), vec![Some(top), Some(top)]);
    assert!(uploads.is_empty());
}
