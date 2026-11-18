use super::*;

/// A block's log holding a grant an administrator signed on the organization's authority takes
/// a new phrase's retirement of its grantee whether or not the server checked the log before:
/// the retirement is checked with the authority the log's records name, as every check of it is.
#[tokio::test]
async fn a_new_phrase_retires_keys_in_a_log_whose_grant_names_the_organization() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("team").await;
    let mut maker = harness.member("maker@example.com").await;
    let mut granted = harness.member("granted@example.com").await;
    for member in [&mut maker, &mut granted] {
        let joined = owner.invite(organization).await.unwrap();
        assert!(matches!(member.accept(joined, &[]).await, Ok(())));
        member.open(organization).await;
    }
    let maker_id = maker.id();
    let workspace = maker.scope_root(maker_id, BlockParent::Root).await;
    let granted_id = granted.id();
    let grant = owner
        .grant(workspace.block, granted_id, be_block_graph::Access::View)
        .await;
    assert!(matches!(grant, Ok(())), "{grant:?}");
    let log = decode_steps(owner.access_log(Some(workspace.block)).await.unwrap());
    assert!(
        log.iter()
            .filter_map(be_block_keys::Step::entry)
            .any(|entry| matches!(
                &entry.record,
                be_block_keys::Record::Grant(grant)
                    if matches!(grant.authority, be_block_keys::Authority::Organization(_))
            )),
        "the administrator granted on the organization's authority"
    );

    // Nothing kept: the log is checked from its root.
    crate::access::forget_checked();
    granted.new_phrase(granted_id).await;
    // Kept: checked on from its head.
    let again = owner
        .grant(workspace.block, granted_id, be_block_graph::Access::Edit)
        .await;
    assert!(matches!(again, Ok(())), "{again:?}");
    granted.new_phrase(granted_id).await;
}
