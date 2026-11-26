use be_block_keys::Admits;
use be_block_protocol::ErrorCode;
use be_type_text::{TextBlock, TextContent};

use super::a_share_link_grants_view_to_an_account_no_one_granted::personal;
use super::*;

/// In the personal organization, where every account is a member and a link may admit no one
/// from outside, a link that admits one is allowed all the same: the first account to open it
/// reads the block, and a third is refused.
#[tokio::test]
async fn a_share_link_admitting_one_in_the_personal_organization_opens_for_one_member() {
    let harness = Harness::start().await;
    let maker = personal(&harness, "maker@example.com").await;
    let first = personal(&harness, "first@example.com").await;
    let third = personal(&harness, "third@example.com").await;
    let team = workspace(&maker, "Plans").await;
    let note = maker
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    maker
        .save(note, &TextBlock::of("for one member"), None)
        .await
        .unwrap();
    let code = maker
        .make_share_link(team, Access::View, Admits::One)
        .await
        .unwrap();

    let (opened, _) = crate::share_links::open_share_link(
        &first.connection(),
        first.keys().identity(),
        first.account(),
        &code,
        None,
    )
    .await
    .unwrap();
    assert!(
        !opened.guest,
        "a member of the personal organization joins as no guest"
    );
    assert_eq!(text(&first, note).await, "for one member");

    let refused = crate::share_links::open_share_link(
        &third.connection(),
        third.keys().identity(),
        third.account(),
        &code,
        None,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(
            refused,
            ClientError::Refused(ErrorCode::PermissionDenied, _)
        ),
        "a third account is refused: {refused}"
    );

    harness.stop().await;
}
