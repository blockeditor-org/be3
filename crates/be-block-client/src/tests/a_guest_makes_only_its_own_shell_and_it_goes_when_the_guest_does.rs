use be_block::{LinkSharing, OrganizationPolicies, SettingsContent, SettingsProfileContent};
use be_block_keys::Admits;
use be_type_text::TextContent;
use be_block_content::BlockMetadata;
use be_block_protocol::BlockSummary;

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

/// A guest makes only its own shell's blocks (decisions/a_guest_makes_only_its_own_shell.md):
/// settings at the top that only it reads, and what lies in them. Nothing else, nothing a member
/// could read, nothing in another's block, and no grant of its own. They go when it leaves,
/// with all they held: the guest is charged nothing, no object is kept for them, and the
/// organization loads again after a restart.
#[tokio::test]
async fn a_guest_makes_only_its_own_shell_and_it_goes_when_the_guest_does() {
    let mut harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let outsider = harness.owner("outsider@example.com").await;
    set_policies(
        &admin,
        &[OrganizationPolicies::set_link_sharing(Some(
            LinkSharing::Anyone,
        ))],
    )
    .await;
    let team = workspace(&admin, "Team").await;
    let code = admin
        .make_share_link(team, Access::Edit, Admits::Anyone)
        .await
        .unwrap();
    crate::share_links::open_share_link(
        &outsider.connection(),
        outsider.keys().identity(),
        outsider.account(),
        &code,
        None,
    )
    .await
    .unwrap();
    let guest = Peer::connect(
        PeerConfig::new(
            &harness.url,
            outsider.keys().identity().clone(),
            Credentials::Token(outsider.token()),
        )
        .organization(Some(admin.organization()))
        .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    let make = |content_type: Uuid, parent: BlockParent, owner_only: bool| {
        let guest = &guest;
        async move {
            guest
                .create_block(
                    Uuid::now_v7(),
                    content_type,
                    parent,
                    &BlockMetadata::default(),
                    owner_only,
                )
                .await
        }
    };
    let refused = |made: &Result<BlockSummary, ClientError>| {
        matches!(made, Err(ClientError::Refused(ErrorCode::PermissionDenied, _)))
    };

    let settings = make(SettingsContent::CONTENT_TYPE, BlockParent::Root, true)
        .await
        .expect("a guest makes its own settings, readable by it alone")
        .id;
    let object_refs = |harness: &Harness| -> i64 {
        harness
            .database()
            .query_row("SELECT COUNT(*) FROM object_refs", [], |row| row.get(0))
            .unwrap()
    };
    // What the account pays for, in every organization (its own account block among them).
    let charged = |harness: &Harness| -> i64 {
        harness
            .database()
            .query_row(
                "SELECT COALESCE(SUM(held + pending), 0) FROM account_usage WHERE account_id = ?1",
                [outsider.account().to_string()],
                |row| row.get(0),
            )
            .unwrap()
    };
    let before = object_refs(&harness);
    let charged_before = charged(&harness);
    let profile = make(
        SettingsProfileContent::CONTENT_TYPE,
        BlockParent::Block(settings),
        false,
    )
    .await
    .expect("and what its shell keeps in them")
    .id;
    guest
        .save(profile, &SettingsProfileContent::default(), None)
        .await
        .unwrap();
    let mut held = SettingsContent::default();
    held.apply(&be_block::Settings::set_kind(
        SettingsProfileContent::CONTENT_TYPE,
        profile,
    ));
    guest.save(settings, &held, None).await.unwrap();
    assert!(object_refs(&harness) > before, "its shell holds objects");
    assert!(charged(&harness) > charged_before, "which its guest pays for");
    for (made, why) in [
        (
            make(TextContent::CONTENT_TYPE, BlockParent::Root, false).await,
            "any other block at the top",
        ),
        (
            make(TextContent::CONTENT_TYPE, BlockParent::Root, true).await,
            "any other block at the top, even one only it reads",
        ),
        (
            make(SettingsContent::CONTENT_TYPE, BlockParent::Root, false).await,
            "settings a member could read",
        ),
        (
            make(TextContent::CONTENT_TYPE, BlockParent::Block(team), false).await,
            "a block in another's block, though its link lets it edit there",
        ),
    ] {
        assert!(refused(&made), "a guest made {why}: {made:?}");
    }
    let granted = guest
        .grant(settings, member.account(), Access::View)
        .await;
    assert!(
        granted.is_err(),
        "a guest grants no one its shell's blocks: {granted:?}"
    );

    guest
        .grant(team, guest.account(), Access::None)
        .await
        .unwrap();
    let kept: i64 = harness
        .database()
        .query_row(
            "SELECT COUNT(*) FROM blocks WHERE organization_id = ?1 AND author = ?2",
            [
                admin.organization().to_string(),
                outsider.account().to_string(),
            ],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(kept, 0, "a guest's shell goes with it");
    assert_eq!(
        charged(&harness),
        charged_before,
        "and nothing it held is charged to it"
    );
    assert_eq!(
        object_refs(&harness),
        before,
        "and no object is kept for it"
    );

    harness.server.restart().await;
    harness.url = format!("ws://{}", harness.server.address);
    harness.reconnect(&admin, admin.token()).await;
    assert!(
        admin.summary(team).await.is_ok(),
        "the organization loads again after its guest's shell went"
    );

    harness.stop().await;
}
