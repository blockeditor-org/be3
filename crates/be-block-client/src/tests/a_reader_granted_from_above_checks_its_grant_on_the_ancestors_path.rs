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

/// A reader granted a note by a manager of the workspace above it, on `Authority::Inherited`,
/// checks that grant (guides/blocks/keys.md, "Parent records"): the note's signed parent record
/// places it in the workspace and names the root of its log, and the workspace, which the reader
/// may not view, is served to it as a proof path proving the granter's Manage, and no one else's
/// grant. The grant rests on no server word, so the note's key is sealed to the reader by it.
#[tokio::test]
async fn a_reader_granted_from_above_checks_its_grant_on_the_ancestors_path() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    let other = harness.member(&admin, "other@example.com").await;
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
        "the reader's grant checks against the workspace's path"
    );

    let served = crate::ask(be_block_protocol::read::<be_block_protocol::AccessLogPage, _>(
        &reader.connection(),
        Some(reader.organization()),
        be_block_protocol::Query::Log {
            log: Some(team),
            held: None,
        },
    ))
    .await
    .unwrap();
    let path: Vec<Step> = served
        .entries
        .iter()
        .map(|step| Step::decode(step).unwrap())
        .collect();
    assert_eq!(
        granted(&path),
        vec![deputy.account().into_bytes()],
        "the workspace's path proves the granter's Manage and no one else's grant"
    );
    assert!(
        path.iter().any(|step| matches!(step, Step::Reduced(_))),
        "the other member's grant is reduced"
    );

    harness.stop().await;
}
