use super::*;

fn queue(queued: &mut Vec<SeqOp<u8>>, separate: &mut Sequence<u8>, op: SeqOp<u8>) {
    separate.apply(&op);
    let leftover = match queued.last_mut() {
        Some(last) => last.absorb(op),
        None => Some(op),
    };
    queued.extend(leftover);
}

#[test]
fn typing_and_deleting_absorb_into_one_operation() {
    let mut separate = loaded("ab");
    let mut merged = separate.clone();
    let mut queued = Vec::new();
    for (index, byte) in b"xyz".iter().enumerate() {
        let op = separate.insert(ALICE, 1 + index, vec![*byte]).unwrap();
        queue(&mut queued, &mut separate, op);
    }
    assert_eq!(queued.len(), 1, "typing forward is one insert");

    for index in [3, 2] {
        let op = separate.delete(index..index + 1).unwrap();
        queue(&mut queued, &mut separate, op);
    }
    assert_eq!(
        queued.len(),
        2,
        "backspacing is one delete after the insert"
    );
    let op = separate.insert(ALICE, 1, b"w".to_vec()).unwrap();
    queue(&mut queued, &mut separate, op);
    assert_eq!(
        queued.len(),
        3,
        "an insert elsewhere starts a new operation"
    );

    for op in &queued {
        merged.apply(op);
    }
    assert_eq!(merged.items(), separate.items());
    assert_eq!(merged.items(), b"awxb");
    assert_eq!(merged.pos(1), separate.pos(1));
    assert_eq!(merged.pos(2), separate.pos(2));
}
