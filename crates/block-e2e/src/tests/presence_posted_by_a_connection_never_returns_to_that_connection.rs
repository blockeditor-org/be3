use super::*;
use block_client::presence::{PresenceColor, UserActive};

#[tokio::test]
async fn presence_posted_by_a_connection_never_returns_to_that_connection() {
    let data_dir = std::env::temp_dir().join(format!("block-e2e-presence-self-{}", Uuid::new_v4()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server_data_dir = data_dir.clone();
    let server = tokio::spawn(async move {
        block_server::serve(listener, server_data_dir)
            .await
            .unwrap();
    });
    let url = format!("http://{address}");
    let (account_id, token, workspace_id) = test_identity(&url).await;

    let host = BlockClient::new(account_id, workspace_id);
    host.connect(url, token);
    let host_block = host.create_block(Counter { count: 0 });
    let block_id = host_block.id();
    timeout(host_block.loaded()).await;

    let (editor_endpoint, editor_carrier) = block_client::tunnel_channel();
    let editor = BlockClient::tunneled(account_id, workspace_id, editor_endpoint, || {});
    let editor_pump = tokio::spawn(carry(host.open_tunnel(|| {}), editor_carrier));
    let editor_block = editor.get_block::<Counter>(block_id);
    timeout(editor_block.loaded()).await;

    let (shell_endpoint, shell_carrier) = block_client::tunnel_channel();
    let shell = BlockClient::tunneled(account_id, workspace_id, shell_endpoint, || {});
    let shell_pump = tokio::spawn(carry(host.open_tunnel(|| {}), shell_carrier));
    let shell_block = shell.get_block::<Counter>(block_id);
    timeout(shell_block.loaded()).await;

    editor.set_presence(
        block_id,
        Some(&UserActive {
            color: PresenceColor::Red,
        }),
    );
    editor.set_presence(block_id, Some(&Cursor { offset: 7 }));
    timeout(editor.synchronized()).await;
    settle().await;

    assert_eq!(editor.presence::<UserActive>(block_id), Vec::new());
    assert_eq!(editor.presence::<Cursor>(block_id), Vec::new());
    assert_eq!(shell.presence::<UserActive>(block_id), Vec::new());
    assert_eq!(shell.presence::<Cursor>(block_id), Vec::new());
    assert_eq!(host.presence::<UserActive>(block_id), Vec::new());
    assert_eq!(host.presence::<Cursor>(block_id), Vec::new());

    drop(editor_block);
    drop(editor);
    drop(shell_block);
    drop(shell);
    editor_pump.abort();
    let _ = editor_pump.await;
    shell_pump.abort();
    let _ = shell_pump.await;
    drop(host_block);
    drop(host);
    server.abort();
    let _ = server.await;
    fs::remove_dir_all(data_dir).await.unwrap();
}
