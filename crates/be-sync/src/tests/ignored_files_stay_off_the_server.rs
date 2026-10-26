use super::*;

#[tokio::test]
async fn ignored_files_stay_off_the_server() {
    let harness = Harness::start().await;
    let peer = harness.peer().await;
    let code = workspace(&peer).await;
    let checkout = harness.checkout("checkout");
    write(&checkout, "src/main.rs", "fn main() {}\n");
    write(&checkout, "target/debug/app", "built");
    write(&checkout, "buck-out/v2/log", "built");
    write(&checkout, ".git/HEAD", "ref: refs/heads/main\n");
    write(&checkout, ".gitignore", "/secrets/\n*.log\n");
    write(&checkout, ".beignore", "scratch.txt\n!keep.log\n");
    write(&checkout, "secrets/token", "hunter2");
    write(&checkout, "debug.log", "noise");
    write(&checkout, "keep.log", "kept");
    write(&checkout, "scratch.txt", "scratch");
    write(&checkout, crate::ERRORS_FILE, "stale");
    sync(&peer, &checkout, code).await;
    let mut names: Vec<String> = children(&peer, code)
        .await
        .into_iter()
        .map(|listed| listed.name)
        .collect();
    names.sort();
    assert_eq!(names, [".beignore", ".gitignore", "keep.log", "src"]);
    assert!(!checkout.join(crate::ERRORS_FILE).exists());

    text_block(&peer, code, "target", "a block named like build output").await;
    let report = sync(&peer, &checkout, code).await;
    assert_eq!(report.skipped.len(), 1, "{report}");
    assert_eq!(read(&checkout, "target/debug/app"), "built");
    harness.stop().await;
}
