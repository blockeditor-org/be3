use super::*;

use be_block::{Counter, CounterContent};

#[tokio::test]
async fn a_follower_that_takes_over_keeps_what_it_typed_before_its_first_save() {
    let harness = Harness::start().await;
    let phone = Arc::new(harness.owner("phone@example.com").await);
    let block = phone
        .create::<CounterContent>(BlockParent::Root)
        .await
        .unwrap();
    let laptop = harness.shared(&phone).await;
    let mut on_phone = Live::<_, CounterContent>::join(Arc::clone(&phone), block)
        .await
        .unwrap();
    let mut on_laptop = Live::<_, CounterContent>::join(Arc::clone(&laptop), block)
        .await
        .unwrap();

    on_phone.edit(Counter::add(1)).await.unwrap();
    until(
        &mut [&mut on_phone, &mut on_laptop],
        "shared the first add",
        |sessions| sessions[1].content().root().value() == 1,
    )
    .await;
    on_phone.seal().await.unwrap().published().unwrap();
    on_laptop.edit(Counter::add(10)).await.unwrap();
    until(
        &mut [&mut on_phone, &mut on_laptop],
        "sequenced the laptop's add",
        |sessions| sessions[0].content().root().value() == 11 && sessions[1].is_clean(),
    )
    .await;
    let sealed = on_phone.seal().await.unwrap().published().unwrap();
    until(&mut [&mut on_laptop], "heard about the seal", |sessions| {
        sessions[0].head() == Some(sealed)
    })
    .await;

    phone.leave_session(block).await.unwrap();
    until(
        &mut [&mut on_laptop],
        "handed the session over",
        |sessions| sessions[0].is_owner(),
    )
    .await;
    on_laptop.edit(Counter::add(100)).await.unwrap();

    let head = on_laptop
        .seal()
        .await
        .unwrap()
        .published()
        .expect("the new owner's first save was refused");
    assert_ne!(head, sealed);
    assert_eq!(
        laptop
            .open::<CounterContent>(block)
            .await
            .unwrap()
            .unwrap()
            .root()
            .value(),
        111
    );

    harness.stop().await;
}
