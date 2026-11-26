use super::a_share_link_grants_view_to_an_account_no_one_granted::personal;
use super::*;

use be_block_keys::Admits;
use be_type_text::{TextBlock, TextContent};

/// The scope key a link carries opens the epochs current when it was made. One opened after a
/// rotation reads what was written since only once an editor's key work seals the newer epoch to
/// the grant the opener signed, by the block's log: the two are not contacts, and the personal
/// organization has no log to vouch for the editor.
#[tokio::test]
async fn a_share_link_opened_after_a_rotation_unlocks_once_an_editor_tends() {
    let harness = Harness::start().await;
    let maker = personal(&harness, "maker@example.com").await;
    let early = personal(&harness, "early@example.com").await;
    let opener = personal(&harness, "opener@example.com").await;
    let team = workspace(&maker, "Plans").await;
    let note = maker
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let head = maker
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    let first = maker
        .make_share_link(team, Access::View, Admits::Members)
        .await
        .unwrap();
    let later = maker
        .make_share_link(team, Access::View, Admits::Members)
        .await
        .unwrap();
    crate::share_links::open_share_link(
        &early.connection(),
        early.keys().identity(),
        early.account(),
        &first,
        None,
    )
    .await
    .unwrap();
    assert_eq!(text(&early, note).await, "before");

    // Revoking the first link takes back the grant made through it, and the block rotates.
    let rows = maker.share_links(team).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter().all(|(_, code)| code.is_some()),
        "the maker's block keeps the codes of the links it made"
    );
    let (row, _) = rows
        .iter()
        .find(|(row, _)| row.id == crate::share_links::link_id(&first))
        .unwrap();
    maker.revoke_share_link(team, row).await.unwrap();
    assert!(early.summary(note).await.is_err(), "its opener lost access");
    let old = maker.keys().scope_key(team).unwrap();
    maker.tend().await.unwrap();
    maker
        .save(note, &TextBlock::of("after"), head)
        .await
        .unwrap()
        .published()
        .expect("the maker writes under the new epoch");
    assert_ne!(
        maker.keys().scope_key(team),
        Some(old),
        "the block rotated as for any lost access"
    );

    crate::share_links::open_share_link(
        &opener.connection(),
        opener.keys().identity(),
        opener.account(),
        &later,
        None,
    )
    .await
    .unwrap();
    let reader = harness.second(&opener).await;
    let locked = reader.open::<TextContent>(note).await;
    assert!(
        matches!(locked, Err(ClientError::Locked(_))),
        "the link carries only the epoch it was made under: {locked:?}"
    );

    maker.tend().await.unwrap();
    let newest = maker.keys().scope_key(team).unwrap();
    assert_eq!(
        sealed_to(&harness, newest, opener.account()),
        1,
        "the editor's key work seals the newer epoch to the opener's grant"
    );
    let reader = harness.second(&opener).await;
    assert_eq!(text(&reader, note).await, "after");

    harness.stop().await;
}
