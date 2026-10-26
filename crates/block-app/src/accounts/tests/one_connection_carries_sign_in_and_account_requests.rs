use super::*;
#[tokio::test]
async fn one_connection_carries_sign_in_and_account_requests() {
    build_id::install("test");
    let directory = std::env::temp_dir().join(format!("block-app-one-link-{}", Uuid::new_v4()));
    let server = crate::platform::start_embedded_server(directory.clone()).unwrap();
    let target = server.url.split_once("://").unwrap().1.to_owned();
    let relay = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", relay.local_addr().unwrap());
    let accepted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = std::sync::Arc::clone(&accepted);
    tokio::spawn(async move {
        while let Ok((mut inbound, _)) = relay.accept().await {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let mut outbound = tokio::net::TcpStream::connect(&target).await.unwrap();
            tokio::spawn(async move {
                let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await;
            });
        }
    });
    let session = create_local_profile(url.clone(), "Local".to_owned())
        .await
        .unwrap();
    organizations(url.clone(), session.token.clone())
        .await
        .unwrap();
    keys(url.clone(), session.token.clone()).await.unwrap();
    create_organization(url.clone(), session.token.clone(), "Shared".to_owned())
        .await
        .unwrap();
    assert_eq!(accepted.load(std::sync::atomic::Ordering::SeqCst), 1);
    drop(server);
    let _ = std::fs::remove_dir_all(directory);
}
