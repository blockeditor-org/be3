use super::*;

/// An invitation's share link is made first; when its contact link then fails (here, the maker
/// already has as many contact links open as the server keeps), the share link is revoked, so
/// no invitation half is left to show as one not opened yet.
#[tokio::test]
async fn an_invitation_whose_contact_link_fails_leaves_no_share_link() {
    let harness = Harness::start().await;
    let maker = harness.owner("maker@example.com").await;
    let team = workspace(&maker, "Team").await;
    {
        let database = harness.database();
        for _ in 0..be_server::limits::MAX_CONTACT_LINKS_PER_ACCOUNT {
            database
                .execute(
                    "INSERT INTO links (id, owner_id, card) VALUES (?1, ?2, x'')",
                    [Uuid::new_v4().to_string(), maker.account().to_string()],
                )
                .unwrap();
        }
    }

    let card = crate::contacts::card(
        maker.account(),
        maker.keys().identity(),
        "maker@example.com",
        "maker@example.com",
    );
    let refused = maker.make_invitation(&card, team, Access::View).await;
    assert!(refused.is_err(), "the contact link is refused: {refused:?}");
    let links = maker.share_links(team).await.unwrap();
    assert!(
        links.is_empty(),
        "the share link made for it was revoked: {links:?}"
    );

    harness.stop().await;
}
