use super::*;

#[tokio::test]
async fn an_invitation_joins_any_account_as_an_editor_once() {
    let harness = Harness::start().await;
    let mut admin = harness.member("admin@example.com").await;
    let organization = admin.organization("team").await;
    let link = admin
        .invite(organization)
        .await
        .expect("an administrator invites");
    // Bound to no email: whoever holds the link reads and accepts it.
    let mut anyone = harness.member("anyone@elsewhere.example").await;
    let found = anyone
        .call(|peer| peer.read::<be_block_protocol::Link>(be_block_protocol::Query::Link(link)))
        .await;
    assert!(
        matches!(&found, Ok(found) if found.organization_name == "team"),
        "{found:?}"
    );
    let accepted = anyone.accept(link, &[]).await;
    assert!(matches!(accepted, Ok(())), "{accepted:?}");
    let members = admin
        .call(|peer| peer.read::<Vec<be_block_protocol::Member>>(be_block_protocol::Query::Members))
        .await
        .expect("the members are listed");
    let joined = members
        .iter()
        .find(|member| member.email == "anyone@elsewhere.example")
        .expect("the accepter joined");
    assert_eq!(joined.role, OrganizationRole::Editor);
    // Opened a second time, by anyone, it is used up.
    let mut second = harness.member("second@example.com").await;
    let again = second
        .call(|peer| peer.read::<be_block_protocol::Link>(be_block_protocol::Query::Link(link)))
        .await;
    assert!(
        matches!(&again, Err(error) if error.code == ErrorCode::LinkNotFound
            && error.message.contains("already used")),
        "{again:?}"
    );
    let accepted = second
        .call(|peer| {
            peer.write::<()>(be_block_protocol::Op::AcceptLink {
                link: link,
                card: Vec::new(),
                contact: Vec::new(),
                key: Vec::new(),
                member: None,
            })
        })
        .await;
    assert!(
        refused_as(&accepted, ErrorCode::LinkNotFound),
        "{accepted:?}"
    );
    harness.stop().await;
}
