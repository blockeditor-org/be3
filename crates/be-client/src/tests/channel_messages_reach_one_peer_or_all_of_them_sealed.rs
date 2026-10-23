use super::*;

use be_block::CounterContent;

const CHANNEL: Uuid = Uuid::from_u128(0xc4a);

#[tokio::test]
async fn channel_messages_reach_one_peer_or_all_of_them_sealed() {
    let harness = Harness::start().await;
    let phone = Arc::new(harness.owner("phone@example.com").await);
    let block = phone
        .create::<CounterContent>(BlockParent::Root)
        .await
        .unwrap();
    let laptop = harness.shared(&phone).await;
    let tablet = harness.shared(&phone).await;
    let mut on_phone = Live::<_, CounterContent>::join(Arc::clone(&phone), block)
        .await
        .unwrap();
    let mut on_laptop = Live::<_, CounterContent>::join(Arc::clone(&laptop), block)
        .await
        .unwrap();
    let mut on_tablet = Live::<_, CounterContent>::join(Arc::clone(&tablet), block)
        .await
        .unwrap();
    until(
        &mut [&mut on_phone, &mut on_laptop, &mut on_tablet],
        "saw each other join",
        |sessions| {
            sessions
                .iter()
                .all(|session| session.state().participants.len() == 3)
        },
    )
    .await;

    let laptop_id = on_laptop.client();
    on_phone
        .send_channel(Some(laptop_id), CHANNEL, b"to the laptop".to_vec())
        .await
        .unwrap();
    on_phone
        .send_channel(None, CHANNEL, b"to everyone".to_vec())
        .await
        .unwrap();
    let mut laptop_heard = Vec::new();
    let mut tablet_heard = Vec::new();
    let heard = tokio::time::timeout(PATIENCE, async {
        while laptop_heard.len() < 2 || tablet_heard.is_empty() {
            tokio::select! {
                handled = on_laptop.wait() => { handled.unwrap(); }
                handled = on_tablet.wait() => { handled.unwrap(); }
            }
            laptop_heard.extend(on_laptop.take_channel());
            tablet_heard.extend(on_tablet.take_channel());
        }
    })
    .await;
    assert!(heard.is_ok(), "the phone's messages never arrived");

    let payloads = |heard: &[ChannelMessage]| {
        heard
            .iter()
            .map(|message| {
                assert_eq!(message.from, on_phone.client());
                assert_eq!(message.channel, CHANNEL);
                message.payload.clone()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        payloads(&laptop_heard),
        [b"to the laptop".to_vec(), b"to everyone".to_vec()],
        "a peer hears what was sent to it in the order it was sent"
    );
    assert_eq!(payloads(&tablet_heard), [b"to everyone".to_vec()]);
    on_phone.poll().await.unwrap();
    assert!(on_phone.take_channel().is_empty(), "a peer never hears itself");

    harness.stop().await;
}
