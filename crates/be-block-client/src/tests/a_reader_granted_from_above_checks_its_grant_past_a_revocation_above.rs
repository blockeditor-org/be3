use super::*;

use be_block_graph::Access;
use be_block_keys::{Record, Step};

/// The accounts the whole grants among `steps` name.
fn granted(steps: &[Step]) -> Vec<[u8; 16]> {
    steps
        .iter()
        .filter_map(Step::entry)
        .filter_map(|entry| match &entry.record {
            Record::Grant(grant) => Some(grant.grantee),
            _ => None,
        })
        .collect()
}

/// A reader granted a note by a manager of the workspace above it checks that grant though
/// someone else was revoked on the workspace before it: the workspace's path, served to a
/// reader below, keeps that revocation whole (guides/blocks/keys.md, "Proof paths"), since
/// authority is taken from no path that hides a lowering, which could be the granter's own.
/// Grants that lowered no one stay reduced.
#[tokio::test]
async fn a_reader_granted_from_above_checks_its_grant_past_a_revocation_above() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    let other = harness.member(&admin, "other@example.com").await;
    let bystander = harness.member(&admin, "bystander@example.com").await;
    let reader = harness.member(&admin, "reader@example.com").await;
    let team = workspace(&admin, "Team").await;
    admin
        .grant(team, deputy.account(), Access::Manage)
        .await
        .unwrap();
    admin
        .grant(team, other.account(), Access::Edit)
        .await
        .unwrap();
    admin
        .grant(team, bystander.account(), Access::Edit)
        .await
        .unwrap();
    admin
        .grant(team, other.account(), Access::None)
        .await
        .unwrap();
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    admin
        .save(note, &TextBlock::of("from above"), None)
        .await
        .unwrap();
    admin.tend().await.unwrap();

    deputy.refresh_keys(None).await.unwrap();
    befriend(&deputy, &reader).await;
    let identity = deputy.verified_identity(reader.account()).unwrap();
    deputy
        .share(note, reader.account(), Access::View, &identity)
        .await
        .unwrap();
    let authority =
        deputy
            .access_log(note)
            .await
            .unwrap()
            .into_iter()
            .find_map(|entry| match entry.record {
                Record::Grant(grant) if grant.grantee == reader.account().into_bytes() => {
                    Some(grant.authority)
                }
                _ => None,
            });
    assert!(
        authority.as_ref().and_then(|authority| authority.above()) == Some(team.into_bytes()),
        "the deputy grants on its Manage of the workspace: {authority:?}"
    );

    deputy.tend().await.unwrap();
    reader.refresh_keys(None).await.unwrap();
    assert_eq!(
        text(&reader, note).await,
        "from above",
        "the note's key reached the reader by the grant from above"
    );
    let (_, checked) = reader.checked_access(note).await.unwrap();
    assert_eq!(
        checked
            .grants
            .get(&reader.account().into_bytes())
            .map(|(level, _)| *level),
        Some(be_block_keys::Level::View),
        "the reader's grant checks against the workspace's path past the revocation"
    );

    let served = crate::ask(
        be_block_protocol::read::<be_block_protocol::AccessLogPage, _>(
            &reader.connection(),
            Some(reader.organization()),
            be_block_protocol::Query::Log {
                log: Some(team),
                held: None,
            },
        ),
    )
    .await
    .unwrap();
    let path: Vec<Step> = served
        .entries
        .iter()
        .map(|step| Step::decode(step).unwrap())
        .collect();
    let other_id = other.account().into_bytes();
    assert_eq!(
        granted(&path),
        vec![deputy.account().into_bytes(), other_id, other_id],
        "the workspace's path proves the granter's Manage and keeps the revocation whole, with \
         the grants of whom it lowered"
    );
    assert!(
        path.iter().all(|step| match step {
            Step::Entry(_) => true,
            Step::Reduced(reduced) => !reduced.lowers,
        }),
        "the path reduces no entry that lowered anyone"
    );
    assert!(
        path.iter().any(|step| matches!(step, Step::Reduced(_))),
        "the bystander's grant is reduced"
    );

    harness.stop().await;
}
