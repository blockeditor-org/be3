use super::*;
use block_client::presence::{PresenceColor, UserActive};

#[tokio::test]
async fn presence_from_one_connection_is_reported_under_a_single_client_id() {
    let data_dir = std::env::temp_dir().join(format!("block-e2e-presence-peer-{}", Uuid::new_v4()));
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
    host.connect(url.clone(), token.clone());
    let host_block = host.create_block(Counter { count: 0 });
    let block_id = host_block.id();
    timeout(host_block.loaded()).await;

    let (endpoint, carrier) = block_client::tunnel_channel();
    let guest = BlockClient::tunneled(account_id, workspace_id, endpoint, || {});
    let pump = tokio::spawn(carry(host.open_tunnel(|| {}), carrier));
    let guest_block = guest.get_block::<Counter>(block_id);
    timeout(guest_block.loaded()).await;

    let peer = BlockClient::new(account_id, workspace_id);
    peer.connect(url, token);
    let peer_block = peer.get_block::<Counter>(block_id);
    timeout(peer_block.loaded()).await;

    let (peer_endpoint, peer_carrier) = block_client::tunnel_channel();
    let peer_guest = BlockClient::tunneled(account_id, workspace_id, peer_endpoint, || {});
    let peer_pump = tokio::spawn(carry(peer.open_tunnel(|| {}), peer_carrier));
    let peer_guest_block = peer_guest.get_block::<Counter>(block_id);
    timeout(peer_guest_block.loaded()).await;

    peer.set_presence(
        block_id,
        Some(&UserActive {
            color: PresenceColor::Blue,
        }),
    );
    peer_guest.set_presence(block_id, Some(&Cursor { offset: 12 }));
    timeout(peer.synchronized()).await;
    timeout(peer_guest.synchronized()).await;
    settle().await;

    let viewers = guest.presence::<UserActive>(block_id);
    let cursors = guest.presence::<Cursor>(block_id);
    let [(viewer, user)] = viewers.as_slice() else {
        panic!("expected exactly one remote viewer, got {viewers:?}");
    };
    let [(author, cursor)] = cursors.as_slice() else {
        panic!("expected exactly one remote cursor, got {cursors:?}");
    };
    assert_eq!(
        *user,
        UserActive {
            color: PresenceColor::Blue
        }
    );
    assert_eq!(*cursor, Cursor { offset: 12 });
    assert_eq!(viewer, author);
    assert_eq!(host.presence::<UserActive>(block_id), viewers);
    assert_eq!(host.presence::<Cursor>(block_id), cursors);

    drop(guest_block);
    drop(guest);
    drop(peer_guest_block);
    drop(peer_guest);
    pump.abort();
    let _ = pump.await;
    peer_pump.abort();
    let _ = peer_pump.await;
    drop(host_block);
    drop(host);
    drop(peer_block);
    drop(peer);
    server.abort();
    let _ = server.await;
    fs::remove_dir_all(data_dir).await.unwrap();
}
