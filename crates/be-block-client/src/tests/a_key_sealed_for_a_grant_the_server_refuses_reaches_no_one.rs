use super::*;

use be_block_graph::Access;
use be_type_workspace::FolderContent;

/// An editor whose device believes it manages a folder (the server's summary says so) shares
/// it with a contact. Its device seals the folder's key to the contact before the grant, so the
/// contact's devices hold the key once the grant tells them; the server refuses the grant, as
/// sharing takes Manage, and the key goes with it: the server keeps no wrap of it for the
/// contact. A device served such a wrap all the same takes a verified contact's key without a
/// grant (`KeyBook::sender_may` checks the sender, never the recipient), so a key sealed for a
/// grant must never outlast its refusal.
#[tokio::test]
async fn a_key_sealed_for_a_grant_the_server_refuses_reaches_no_one() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let editor = harness.member(&admin, "editor@example.com").await;
    let guest = harness.member(&admin, "guest@example.com").await;
    befriend(&editor, &guest).await;
    let team = workspace(&admin, "Team").await;
    let folder = admin
        .create::<FolderContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let identity = admin.verified_identity(editor.account()).unwrap();
    admin
        .share(team, editor.account(), Access::Edit, &identity)
        .await
        .unwrap();
    editor.refresh_keys(None).await.unwrap();
    harness.server.lie(editor.account(), move |page| {
        if let be_block_protocol::Page::Summaries(summaries) = page {
            for summary in summaries.iter_mut().flatten() {
                if summary.id == folder {
                    summary.access = Access::Manage;
                }
            }
        }
    });

    let identity = editor.verified_identity(guest.account()).unwrap();
    let refused = editor
        .share(folder, guest.account(), Access::View, &identity)
        .await
        .unwrap_err();
    assert!(
        matches!(
            refused,
            ClientError::Refused(be_block_protocol::ErrorCode::PermissionDenied, _)
        ),
        "the grant was not refused: {refused}"
    );
    let key = editor.keys().scope_key(folder).unwrap();
    assert_eq!(
        sealed_to(&harness, key, guest.account()),
        0,
        "the folder's key sealed for the refused grant outlasted it"
    );
    guest.refresh_keys(None).await.unwrap();
    assert_eq!(guest.keys().scope_key(folder), None);

    // What a server that kept the wrap could serve: the guest's device takes it.
    let row = editor.keys().row(key).unwrap();
    let wrap = editor
        .keys()
        .seal_to(key, guest.account(), &identity.public)
        .unwrap();
    harness.server.lie(guest.account(), move |page| {
        if let be_block_protocol::Page::Keys(page) = page {
            page.keys.keys.push(row);
            page.keys.wrapped.push(wrap.clone());
        }
    });
    guest.refresh_keys(None).await.unwrap();
    assert_eq!(
        guest.keys().scope_key(folder),
        Some(key),
        "a contact's key served without a grant was refused: the doc above is out of date"
    );

    harness.stop().await;
}
