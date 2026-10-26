use super::*;

#[tokio::test]
async fn files_made_on_disk_become_blocks() {
    let harness = Harness::start().await;
    let peer = harness.peer().await;
    let code = workspace(&peer).await;
    let checkout = harness.checkout("checkout");
    write(&checkout, "README.txt", "hello\n");
    write(&checkout, "crates/app/src/lib.rs", "pub fn app() {}\n");
    write(&checkout, "scripts/verify", "#!/bin/sh\necho ok\n");
    std::fs::write(checkout.join("snapshot.paint"), [0u8, 1, 2, 255]).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            checkout.join("scripts/verify"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }

    let report = sync(&peer, &checkout, code).await;
    assert_eq!(report.created, 8, "{report}");

    let readme = child(&peer, code, "README.txt").await;
    assert_eq!(readme.block_type, TextContent::CONTENT_TYPE);
    assert_eq!(text_of(&peer, readme.id).await, "hello\n");
    let paint = child(&peer, code, "snapshot.paint").await;
    assert_eq!(paint.block_type, FileContent::CONTENT_TYPE);
    let stored = peer.open::<FileContent>(paint.id).await.unwrap().unwrap();
    assert_eq!(stored.data(), &[0u8, 1, 2, 255]);
    let scripts = child(&peer, code, "scripts").await;
    assert_eq!(scripts.block_type, FolderContent::CONTENT_TYPE);
    let verify = child(&peer, scripts.id, "verify").await;
    assert_eq!(verify.executable, cfg!(unix));
    let crates = child(&peer, code, "crates").await;
    let app = child(&peer, crates.id, "app").await;
    let src = child(&peer, app.id, "src").await;
    let lib = child(&peer, src.id, "lib.rs").await;
    assert_eq!(text_of(&peer, lib.id).await, "pub fn app() {}\n");
    let listed = peer.open::<FolderContent>(src.id).await.unwrap().unwrap();
    assert_eq!(listed.root().blocks(), vec![lib.id]);

    let again = sync(&peer, &checkout, code).await;
    assert!(!again.changed(), "{again}");

    let elsewhere = harness.checkout("elsewhere");
    sync(&peer, &elsewhere, code).await;
    assert_eq!(
        read(&elsewhere, "crates/app/src/lib.rs"),
        "pub fn app() {}\n"
    );
    assert_eq!(
        std::fs::read(elsewhere.join("snapshot.paint")).unwrap(),
        [0u8, 1, 2, 255]
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(elsewhere.join("scripts/verify"))
            .unwrap()
            .permissions()
            .mode();
        assert_ne!(mode & 0o100, 0, "{mode:o}");
        let mode = std::fs::metadata(elsewhere.join("README.txt"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0, "{mode:o}");
    }
    harness.stop().await;
}
