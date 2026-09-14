use uuid::Uuid;

use super::{BlockClient, history_test_support::DisabledHistoryBlock};

#[test]
fn no_history_policy_disables_undo() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(DisabledHistoryBlock);
    block.operate(());
    assert!(!block.supports_history());
    assert!(!block.can_undo());
    assert!(!block.can_redo());
}
