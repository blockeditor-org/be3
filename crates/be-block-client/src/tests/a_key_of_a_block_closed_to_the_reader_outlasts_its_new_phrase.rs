use super::*;

use be_block_graph::Access;
use be_type_workspace::FolderContent;

/// The admin shares a folder with the guest, and the guest's membership of the organization
/// then ends while the key stays sealed to it there: taken out of the server's database
/// straight, as a server whose leaving or revoking path kept the wraps would hold it (the ones
/// it has delete them). A new phrase drops every key sealed to the account that it does not
/// seal again, so the server shows it every one, in every organization, whether or not the
/// account may still view the block, and the key outlasts the move, still as the admin's.
#[tokio::test]
async fn a_key_of_a_block_closed_to_the_reader_outlasts_its_new_phrase() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let guest = harness.member(&admin, "guest@example.com").await;
    let team = workspace(&admin, "Team").await;
    let folder = admin
        .create::<FolderContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let identity = admin.verified_identity(guest.account()).unwrap();
    admin
        .share(folder, guest.account(), Access::View, &identity)
        .await
        .unwrap();
    let key = admin.keys().scope_key(folder).unwrap();
    assert_eq!(sealed_to(&harness, key, guest.account()), 1);
    harness
        .database()
        .execute(
            "DELETE FROM memberships WHERE organization_id = ?1 AND account_id = ?2",
            [
                admin.organization().to_string(),
                guest.account().to_string(),
            ],
        )
        .unwrap();

    new_phrase(
        &guest.connection(),
        guest.account(),
        guest.keys().identity(),
    )
    .await;
    assert_eq!(
        sealed_to(&harness, key, guest.account()),
        1,
        "a new phrase dropped the key of a block in an organization the account left"
    );
    let (sender, _) = wrap_to(&harness, key, guest.account());
    assert_eq!(
        sender,
        admin.account(),
        "the admin's key sealed again as another's"
    );

    harness.stop().await;
}
