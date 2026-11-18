use super::*;

/// Two contacts take new phrases at once: each seals its key change to the other's old key,
/// as the other's change had not reached it yet. Each account keeps the secret its new phrase
/// replaced in its own block (`AccountBlock::retired`), so each follows the other's change all
/// the same, and neither shows the other as not confirmed (guides/blocks/keys.md, "Contacts").
#[tokio::test]
async fn two_contacts_taking_new_phrases_at_once_still_follow_each_other() {
    let harness = Harness::start().await;
    let rosa = harness.owner("rosa@example.com").await;
    let guest = harness.member(&rosa, "guest@example.com").await;
    rosa.refresh_contacts().await.unwrap();
    guest.refresh_contacts().await.unwrap();

    // Rosa's key change reaches the guest's mailbox only after the guest took its own phrase.
    let withheld = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let held_back = std::sync::Arc::clone(&withheld);
    let from = rosa.account();
    harness.server.lie(guest.account(), move |page| {
        if let be_block_protocol::Page::Mailbox(messages) = page
            && held_back.load(std::sync::atomic::Ordering::SeqCst)
        {
            messages.retain(|message| {
                !(message.from == Some(from)
                    && matches!(message.letter, be_block_protocol::Letter::KeyChange(_)))
            });
        }
    });
    let rosa_new = new_phrase(&rosa.connection(), rosa.account(), rosa.keys().identity()).await;
    let guest_new = new_phrase(&guest.connection(), guest.account(), guest.keys().identity()).await;
    withheld.store(false, std::sync::atomic::Ordering::SeqCst);

    let key_of = |contacts: Vec<crate::Contact>, account: Uuid| {
        contacts
            .into_iter()
            .find(|contact| contact.account == account)
            .and_then(|contact| contact.public)
    };
    let rosas = crate::contacts::contacts(
        &rosa.connection(),
        rosa.account(),
        &rosa_new,
        &std::collections::HashMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        key_of(rosas, guest.account()),
        Some(guest_new.public()),
        "rosa did not follow the change the guest sealed to her old key"
    );
    let guests = crate::contacts::contacts(
        &guest.connection(),
        guest.account(),
        &guest_new,
        &std::collections::HashMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        key_of(guests, rosa.account()),
        Some(rosa_new.public()),
        "the guest did not follow the change rosa sealed to its old key"
    );

    harness.stop().await;
}
