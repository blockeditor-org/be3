use super::*;

#[tokio::test]
async fn uploads_no_block_takes_are_collected_after_their_grace() {
    let directory = std::env::temp_dir().join(format!("be-server-test-{}", Uuid::new_v4()));
    let (profile, _) = ServerStore::open(&directory)
        .unwrap()
        .register("uploader@example.com", "Uploader", "correct horse battery")
        .await
        .unwrap();
    let lasting = ServerStore::open(&directory).unwrap();
    let identity = Identity {
        account: profile.account,
        organization: lasting.personal_organization().await.unwrap(),
        role: OrganizationRole::Editor,
    };
    let kept = b"an upload still within its grace".to_vec();
    let hashes = lasting
        .put_objects(identity, vec![kept.clone()])
        .await
        .unwrap();
    assert_eq!(lasting.collect_uploads().await.unwrap(), 0);
    assert_eq!(
        lasting.storage_usage(identity).await.unwrap().pending,
        kept.len() as u64,
        "an upload no block holds is charged to its uploader"
    );
    assert!(lasting.get_object(hashes[0]).await.unwrap().is_some());
    drop(lasting);

    let impatient = ServerStore::open_with(
        &directory,
        crate::store::StorageLimits {
            upload_grace: Duration::ZERO,
            ..crate::store::StorageLimits::DEFAULT
        },
        None,
    )
    .unwrap();
    assert_eq!(
        impatient.collect_uploads().await.unwrap(),
        1,
        "an upload past its grace is deleted"
    );
    assert!(impatient.get_object(hashes[0]).await.unwrap().is_none());
    assert_eq!(impatient.storage_usage(identity).await.unwrap().pending, 0);

    drop(impatient);
    let _ = std::fs::remove_dir_all(&directory);
}
