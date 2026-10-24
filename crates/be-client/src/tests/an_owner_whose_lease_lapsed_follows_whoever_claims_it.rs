use super::*;

use be_block::{TextBlock, TextContent};

#[tokio::test]
async fn an_owner_whose_lease_lapsed_follows_whoever_claims_it() {
    let harness = Harness::lapsing().await;
    let phone = Arc::new(harness.owner("phone@example.com").await);
    let block = phone
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    phone
        .save(block, &TextBlock::of("notes\n"), None)
        .await
        .unwrap();
    let laptop = harness.shared(&phone).await;
    let mut on_phone = Live::<_, TextContent>::join(Arc::clone(&phone), block)
        .await
        .unwrap();
    let mut on_laptop = Live::<_, TextContent>::join(Arc::clone(&laptop), block)
        .await
        .unwrap();
    type_at(&mut on_phone, 6, "a\n").await;
    until(
        &mut [&mut on_phone, &mut on_laptop],
        "shared the phone's first line",
        |sessions| sessions[1].content().to_text() == "notes\na\n",
    )
    .await;

    assert!(
        on_laptop.claim().await.unwrap(),
        "the lapsed lease was not taken"
    );
    assert!(on_laptop.is_owner());
    type_at(&mut on_phone, 8, "typed before the phone heard\n").await;
    type_at(&mut on_laptop, 8, "typed by the new owner\n").await;
    until(
        &mut [&mut on_phone, &mut on_laptop],
        "moved the phone to following",
        |sessions| !sessions[0].is_owner() && sessions[1].content().to_text().len() > 40,
    )
    .await;
    assert!(
        on_phone.seal().await.unwrap().published().is_none(),
        "the old owner still saved"
    );

    let head = on_laptop.seal().await.unwrap().published().unwrap();
    until(
        &mut [&mut on_phone, &mut on_laptop],
        "agreed after the new owner saved",
        |sessions| {
            sessions[0].head() == Some(head)
                && sessions[0].content().to_text() == sessions[1].content().to_text()
        },
    )
    .await;
    let text = on_laptop.content().to_text();
    assert!(text.contains("typed before the phone heard\n"), "{text:?}");
    assert!(text.contains("typed by the new owner\n"), "{text:?}");

    type_at(&mut on_laptop, 0, "laptop: ").await;
    type_at(&mut on_phone, 0, "phone: ").await;
    until(
        &mut [&mut on_phone, &mut on_laptop],
        "kept both following the new owner",
        |sessions| {
            let text = sessions[0].content().to_text();
            sessions[0].is_clean()
                && text == sessions[1].content().to_text()
                && text.contains("laptop: ")
                && text.contains("phone: ")
        },
    )
    .await;
    assert!(on_phone.is_clean() && !on_phone.is_owner());
    let saved = on_laptop.seal().await.unwrap().published().unwrap();
    assert_eq!(
        phone
            .open::<TextContent>(block)
            .await
            .unwrap()
            .unwrap()
            .to_text(),
        on_laptop.content().to_text()
    );
    assert_ne!(saved, head);

    harness.stop().await;
}
