use super::*;

use be_block_graph::Access;
use be_block_keys::Record;

/// An editor who administers nothing is served a proof path of the organization's log
/// (guides/blocks/keys.md, "Who is served what"), and still signs into it at its head: the
/// member record of an agent it makes, and the retirement of its keys as it takes a new phrase.
/// Every device checks the log after both, and the editor is still served no other person.
#[tokio::test]
async fn an_editor_served_part_of_the_organizations_log_signs_into_it() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let carol = harness.member(&admin, "carol@example.com").await;
    let _bob = harness.member(&admin, "bob@example.com").await;
    let mine = workspace(&carol, "Mine").await;
    let organization = carol.organization_log().await.unwrap();
    assert!(
        organization
            .iter()
            .any(|step| matches!(step, be_block_keys::Step::Reduced(_))),
        "the editor is served a proof path"
    );

    let secret = be_block_keys::RecoveryPhrase::generate().secret();
    let agent = Uuid::from_bytes(be_block_keys::agent_account(
        carol.account().into_bytes(),
        &secret.identity(),
    ));
    let bound = be_block_protocol::BoundAgent {
        block: mine,
        signed: carol
            .grant_record(mine, agent, Access::Edit, secret.identity())
            .await
            .unwrap(),
    };
    let member = carol.agent_record(agent, secret.identity()).await.unwrap();
    assert!(member.is_some(), "the editor signs its agent in");
    crate::ask(
        be_block_protocol::write::<be_block_protocol::CreatedAgent, _>(
            &carol.connection(),
            Some(carol.organization()),
            be_block_protocol::Op::CreateAgent {
                account: agent,
                name: "Helper".into(),
                recovery: secret.public().0,
                signing: secret.signing().0,
                contact: Vec::new(),
                record: Vec::new(),
                escrow: Vec::new(),
                bound_to: Some(bound),
                member,
            },
        ),
    )
    .await
    .unwrap();

    let old = carol.keys().identity().clone();
    take_phrase(
        &carol.connection(),
        carol.account(),
        &old,
        &be_block_keys::RecoveryPhrase::generate().secret(),
    )
    .await;

    let whole = admin.organization_log().await.unwrap();
    let records = |steps: &[be_block_keys::Step]| -> Vec<Record> {
        steps
            .iter()
            .filter_map(be_block_keys::Step::entry)
            .map(|entry| entry.record.clone())
            .collect()
    };
    let signed = records(&whole);
    assert!(
        signed.iter().any(|record| matches!(
            record,
            Record::Member(member) if member.account == agent.into_bytes()
        )),
        "the agent's member record is in the log"
    );
    assert!(
        matches!(signed.last(), Some(Record::Retirement(_))),
        "the editor's retirement ends the log: {:?}",
        signed.last()
    );

    let laptop = harness.second(&admin).await;
    laptop
        .organization_log()
        .await
        .expect("another device checks the log the editor signed into");
    let people: Vec<[u8; 16]> = records(&carol.organization_log().await.unwrap())
        .into_iter()
        .filter_map(|record| match record {
            Record::Member(member) if !member.agent => Some(member.account),
            _ => None,
        })
        .collect();
    assert_eq!(
        people,
        vec![carol.account().into_bytes()],
        "the editor is served another person"
    );

    harness.stop().await;
}
