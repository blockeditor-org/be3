use super::*;
use crate::version_control::{Action, OPERATION_LIMIT, Operation, Repository, RepositoryContent};

fn operation(id: u128, action: Action) -> Operation {
    Operation {
        id: Uuid::from_u128(id),
        action,
        ..Operation::default()
    }
}

#[test]
fn a_repository_keeps_its_latest_operations_and_knows_what_was_undone() {
    let mut repository = Repository::default();
    for id in 0..OPERATION_LIMIT as u128 + 5 {
        repository.record(operation(id + 1, Action::Pulled));
    }
    assert_eq!(repository.operations.len(), OPERATION_LIMIT);
    assert_eq!(repository.operations[0].value.id, Uuid::from_u128(6));

    let undo = |id: u128, target: u128| {
        operation(
            id,
            Action::Undid {
                operation: Uuid::from_u128(target),
                what: String::new(),
            },
        )
    };
    repository.record(undo(1000, 100));
    repository.record(undo(1001, 101));
    repository.record(undo(1002, 1001));
    assert_eq!(
        repository.undone().into_iter().collect::<Vec<_>>(),
        vec![Uuid::from_u128(100), Uuid::from_u128(1001)],
        "undoing an undo brings back what it undid"
    );

    let content = RepositoryContent::new(&repository);
    let decoded = RepositoryContent::decode(&content.encode()).unwrap();
    assert_eq!(
        decoded.root().operation(Uuid::from_u128(1002)),
        repository.operation(Uuid::from_u128(1002))
    );
}
