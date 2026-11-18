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

/// A member with View is served a proof path of the block's log (guides/blocks/keys.md, "Proof
/// paths"): the root and its own grant whole (and the grant of whoever sealed it a key, here the
/// creator, whose right is the root), every other entry reduced to what chains it, so no other
/// member's grant reaches it; and of the organization's log, no other member's record. The path
/// still checks from the root it pinned, the key sealed to it opens and reads, and a revocation
/// it is served only reduced still outdates the epochs named before it. An editor is served
/// every grant.
#[tokio::test]
async fn a_member_with_view_is_served_no_other_members_grant() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let carol = harness.member(&admin, "carol@example.com").await;
    let bob = harness.member(&admin, "bob@example.com").await;
    let vera = harness.member(&admin, "vera@example.com").await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    admin
        .save(note, &TextBlock::of("hello"), None)
        .await
        .unwrap();
    for (member, access) in [
        (&carol, Access::Edit),
        (&bob, Access::Edit),
        (&vera, Access::View),
    ] {
        admin.grant(team, member.account(), access).await.unwrap();
    }
    admin.tend().await.unwrap();
    vera.refresh_keys(None).await.unwrap();
    assert_eq!(text(&vera, note).await, "hello", "the viewer reads");

    let (steps, checked) = vera.checked_access(team).await.unwrap();
    assert!(checked.partial, "the viewer is served a proof path");
    assert_eq!(
        granted(&steps),
        vec![vera.account().into_bytes()],
        "the viewer is served no other member's grant"
    );
    assert_eq!(
        steps
            .iter()
            .filter(|step| matches!(step, Step::Reduced(_)))
            .count(),
        2,
        "the editors' grants are reduced"
    );
    assert!(
        !checked.grants.contains_key(&carol.account().into_bytes())
            && !checked.grants.contains_key(&bob.account().into_bytes())
    );

    carol.refresh_keys(None).await.unwrap();
    let (steps, checked) = carol.checked_access(team).await.unwrap();
    assert!(!checked.partial, "an editor is served the whole log");
    assert_eq!(granted(&steps).len(), 3);

    let members = |steps: &[Step]| -> Vec<[u8; 16]> {
        steps
            .iter()
            .filter_map(Step::entry)
            .filter_map(|entry| match &entry.record {
                Record::Member(member) => Some(member.account),
                _ => None,
            })
            .collect()
    };
    let organization = vera.organization_log().await.unwrap();
    assert_eq!(
        members(&organization),
        vec![vera.account().into_bytes()],
        "the organization's log names the viewer no other member"
    );
    assert!(
        members(&carol.organization_log().await.unwrap()).len() == 3,
        "an editor is served the organization's whole log"
    );

    // Bob's revocation reaches the viewer reduced, yet still outdates what came before it.
    let before = checked.head;
    admin
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();
    let (steps, checked) = vera.checked_access(team).await.unwrap();
    assert_eq!(granted(&steps), vec![vera.account().into_bytes()]);
    assert!(
        checked.outdates(&before),
        "a reduced revocation outdates the epochs named before it"
    );
    assert!(checked.current_at(&checked.head));
}
