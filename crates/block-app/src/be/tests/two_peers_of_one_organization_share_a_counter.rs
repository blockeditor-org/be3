use super::*;

#[test]
fn two_peers_of_one_organization_share_a_counter() {
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
        organization: harness.organization,
        identity: harness.identity(),
        display_name: "Counter".to_owned(),
        email: "counter@example.com".to_owned(),
        data_dir: elsewhere.clone(),
        clock: be_client::Clock::real(),
    });
    open(block, CounterContent::CONTENT_TYPE);

    wait_for_count(block, 7);
    stop_and_wait();
    let _ = std::fs::remove_dir_all(&elsewhere);
}
