use super::*;
use block_client::presence::{PresenceColor, UserActive};

#[tokio::test]
async fn presence_a_plugin_publishes_never_comes_back_to_it() {
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
    let plugin = BlockClient::tunneled(account_id, workspace_id, endpoint, || {});
    let pump = tokio::spawn(carry(host.open_tunnel(|| {}), carrier));
    let plugin_block = plugin.get_block::<Counter>(block_id);
    timeout(plugin_block.loaded()).await;

    plugin.set_presence(
        block_id,
        Some(&UserActive {
            color: PresenceColor::Red,
        }),
    );
    plugin.set_presence(block_id, Some(&Cursor { offset: 7 }));
    timeout(plugin.synchronized()).await;
    settle().await;

    assert_eq!(plugin.presence::<UserActive>(block_id), Vec::new());
    assert_eq!(plugin.presence::<Cursor>(block_id), Vec::new());

    drop(plugin_block);
    drop(plugin);
    pump.abort();
    let _ = pump.await;
    drop(host_block);
    drop(host);
    server.abort();
    let _ = server.await;
    fs::remove_dir_all(data_dir).await.unwrap();
}
