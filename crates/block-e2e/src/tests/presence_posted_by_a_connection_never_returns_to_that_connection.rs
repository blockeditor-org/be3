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

    let (endpoint, carrier) = block_client::tunnel_channel();
    let guest = BlockClient::tunneled(account_id, workspace_id, endpoint, || {});
    let pump = tokio::spawn(carry(host.open_tunnel(|| {}), carrier));
    let guest_block = guest.get_block::<Counter>(block_id);
    timeout(guest_block.loaded()).await;

    host.set_presence(
        block_id,
        Some(&UserActive {
            color: PresenceColor::Red,
        }),
    );
    guest.set_presence(block_id, Some(&Cursor { offset: 7 }));
    timeout(host.synchronized()).await;
    timeout(guest.synchronized()).await;
    settle().await;

    assert_eq!(guest.presence::<UserActive>(block_id), Vec::new());
    assert_eq!(guest.presence::<Cursor>(block_id), Vec::new());
    assert_eq!(host.presence::<UserActive>(block_id), Vec::new());
    assert_eq!(host.presence::<Cursor>(block_id), Vec::new());

    drop(guest_block);
    drop(guest);
    pump.abort();
    let _ = pump.await;
    drop(host_block);
    drop(host);
    server.abort();
    let _ = server.await;
    fs::remove_dir_all(data_dir).await.unwrap();
}
