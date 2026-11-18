use super::*;
use be_block_protocol::NotifyMode;

#[tokio::test]
async fn a_mute_set_on_one_device_is_announced_to_the_others() {
    let harness = Harness::start().await;
    let mut phone = harness.member("member@example.com").await;
    let organization = phone.personal().await;
    let block = phone.create_block(BlockParent::Root).await;
    let mut laptop = harness
        .second_connection(&phone.token.clone(), organization)
        .await;
    let followed = laptop
        .call(|peer| peer.ask::<Vec<be_block_protocol::BlockMute>>(Query::Mutes, true))
        .await;
    assert!(followed.is_ok(), "{followed:?}");

    phone.set_mute(block, NotifyMode::Off).await;
    laptop.heard_changed(Topic::Mutes).await;
    let modes = laptop
        .call(|peer| {
            peer.read::<Vec<be_block_protocol::BlockMute>>(be_block_protocol::Query::Mutes)
        })
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
