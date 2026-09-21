use super::*;

#[test]
fn two_peers_of_one_workspace_share_a_counter() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    add(block, 7);
    wait_for_count(block, 7);
    flush();
    stop();

    let elsewhere = std::env::temp_dir().join(format!("block-app-be-peer-{}", Uuid::new_v4()));
    start(Config {
        server_url: harness.url.clone(),
        token: harness.token.clone(),
        account: harness.account,
        workspace: harness.workspace,
        data_dir: elsewhere.clone(),
        context: eframe::egui::Context::default(),
    });
    open(block, CounterContent::CONTENT_TYPE);

    wait_for_count(block, 7);
    stop();
    let _ = std::fs::remove_dir_all(&elsewhere);
}
