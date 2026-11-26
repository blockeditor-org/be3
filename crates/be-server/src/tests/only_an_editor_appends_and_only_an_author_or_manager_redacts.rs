use super::*;
use be_block_graph::Access;

/// Appending needs Edit, so a viewer's append is refused; reading needs View. Redacting is for
/// an entry's author or a manager of its block: an editor cannot take another's entry out, and
/// a manager can.
#[tokio::test]
async fn only_an_editor_appends_and_only_an_author_or_manager_redacts() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    owner.personal().await;
    let mut editor = harness.member("editor@example.com").await;
    let editor_id = editor.id();
    editor.personal().await;
    let mut viewer = harness.member("viewer@example.com").await;
    let viewer_id = viewer.id();
    viewer.personal().await;
    for (guest, id) in [(&mut editor, editor_id), (&mut viewer, viewer_id)] {
        let link = owner.link().await;
        let _ = guest.accept(link, &[]).await;
        let _ = id;
    }
    let chat = owner.create_block(BlockParent::Root).await;
    assert!(owner.grant(chat, editor_id, Access::Edit).await.is_ok());
    assert!(owner.grant(chat, viewer_id, Access::View).await.is_ok());

    owner
        .append(chat, vec![log_entry(b"the owner's")])
        .await
        .unwrap();
    editor
        .append(chat, vec![log_entry(b"the editor's")])
        .await
        .unwrap();
    let refused = viewer.append(chat, vec![log_entry(b"the viewer's")]).await;
    assert!(
        matches!(&refused, Err(error) if error.code == ErrorCode::PermissionDenied),
        "{refused:?}"
    );
    let page = viewer.log_page(chat, 0, None, false).await.unwrap();
    assert_eq!(page.head.size, 2, "a viewer reads the log");
    assert!(verifies(&page, 0));

    let redact = |seq| Op::Redact { block: chat, seq };
    let refused = editor.call(|peer| peer.write::<()>(redact(0))).await;
    assert!(
        matches!(&refused, Err(error) if error.code == ErrorCode::PermissionDenied),
        "an editor redacted another's entry: {refused:?}"
    );
    let refused = viewer.call(|peer| peer.write::<()>(redact(1))).await;
    assert!(
        matches!(&refused, Err(error) if error.code == ErrorCode::PermissionDenied),
        "a viewer redacted another's entry: {refused:?}"
    );
    let own = editor.call(|peer| peer.write::<()>(redact(1))).await;
    assert!(own.is_ok(), "an author could not redact its entry: {own:?}");
    let managed = owner.call(|peer| peer.write::<()>(redact(1))).await;
    assert!(managed.is_ok(), "{managed:?}");
    owner.append(chat, vec![log_entry(b"again")]).await.unwrap();
    let managed = owner.call(|peer| peer.write::<()>(redact(0))).await;
    assert!(managed.is_ok(), "{managed:?}");
    let page = owner.log_page(chat, 0, None, false).await.unwrap();
    assert_eq!(
        page.entries
            .iter()
            .map(|entry| entry.payload.is_some())
            .collect::<Vec<_>>(),
        [false, false, true]
    );
    assert!(verifies(&page, 0));

    let stranger = harness.member("stranger@example.com").await;
    let mut stranger = stranger;
    let organization = stranger.personal().await;
    let _ = organization;
    let refused = stranger.log_page(chat, 0, None, false).await;
    assert!(
        matches!(&refused, Err(error) if error.code == ErrorCode::BlockNotFound),
        "{refused:?}"
    );
    harness.stop().await;
}
