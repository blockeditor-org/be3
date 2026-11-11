use super::*;
use be_block_protocol::NotifyMode;

#[tokio::test]
async fn a_mode_set_on_one_device_is_announced_to_the_others() {
    let harness = Harness::start().await;
    let mut phone = harness.client().await;
    phone.register("member@example.com").await;
    let organization = phone.personal().await;
    let block = phone.create_block(BlockParent::Root).await;
    let mut laptop = harness
        .second_connection(&phone.token.clone(), organization)
        .await;
    let mut changed = client::notifications::notify_modes_changed(&laptop.peer);
    // Calls are taken in order, so the stream is open once this is answered.
    let _ = laptop
        .call(|peer| client::notifications::notify_modes(peer))
        .await;

    phone.set_notify_mode(block, NotifyMode::Off).await;
    assert_eq!(changed.recv().await, Some(()));
    let modes = laptop
        .call(|peer| client::notifications::notify_modes(peer))
        .await
        .unwrap();
    assert_eq!(
        modes
            .iter()
            .map(|row| (row.block, row.mode))
            .collect::<Vec<_>>(),
        [(block, NotifyMode::Off)]
    );

    harness.stop().await;
}
