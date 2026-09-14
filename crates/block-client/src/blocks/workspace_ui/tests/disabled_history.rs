use super::*;

#[tokio::test]
async fn disabled_history() {
    let client = crate::BlockClient::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let workspace = client.create_block(WorkspaceUi::new());
    assert!(!workspace.supports_history());
}
