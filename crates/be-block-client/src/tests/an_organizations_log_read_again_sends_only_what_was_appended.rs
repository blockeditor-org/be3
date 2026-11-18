use super::*;

use be_block_keys::Entry;

/// A device that read an organization's log whole reads it again by naming the last entry it
/// holds: the server sends only the entries after it, and the device keeps the prefix it
/// checked. A server that rewrote the entry the device holds sends the log from its root, which
/// is refused as forged.
#[tokio::test]
async fn an_organizations_log_read_again_sends_only_what_was_appended() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let organization = owner.organization();
    let hashes = |log: &[be_block_keys::Step]| -> Vec<be_block_keys::EntryHash> {
        log.iter().map(be_block_keys::Step::hash).collect()
    };
    let first = owner.organization_log().await.unwrap();
    let held = be_block_protocol::HeldEntry {
        place: first.len() as u64 - 1,
        hash: first.last().unwrap().hash(),
    };

    let _member = harness.member(&owner, "member@example.com").await;
    let page = crate::ask(be_block_protocol::read::<be_block_protocol::AccessLogPage, _>(
        &owner.connection(),
        Some(organization),
        be_block_protocol::Query::Log {
            log: None,
            held: Some(held),
        },
    ))
    .await
    .unwrap();
    assert_eq!(
        page.from,
        first.len() as u64,
        "only the entries after the one held are sent"
    );
    assert_eq!(page.entries.len(), 2, "the invitation and the member it signed in");

    let second = owner.organization_log().await.unwrap();
    assert_eq!(second.len(), first.len() + 2);
    assert_eq!(hashes(&second[..first.len()]), hashes(&first));
    let fresh = crate::ask(be_block_protocol::read::<be_block_protocol::AccessLogPage, _>(
        &owner.connection(),
        Some(organization),
        be_block_protocol::Query::Log {
            log: None,
            held: None,
        },
    ))
    .await
    .unwrap();
    assert_eq!(page.from, first.len() as u64);
    assert_eq!(
        fresh
            .entries
            .iter()
            .map(|step| be_block_keys::Step::decode(step).unwrap().hash())
            .collect::<Vec<_>>(),
        hashes(&second),
        "the log kept and extended is the log the server holds"
    );
    owner.checked_organization().await.unwrap();

    // The server rewrites the last entry the device holds: it no longer holds that entry there,
    // so it sends the log from its root, whose prefix is not what the device checked.
    let forged = Entry::root(
        owner.keys().identity(),
        owner.account().into_bytes(),
        be_block_keys::LogKind::Organization,
        organization.into_bytes(),
    );
    harness
        .database()
        .execute(
            "UPDATE organization_log SET entry = ?1 WHERE organization_id = ?2 AND position = ?3",
            rusqlite::params![
                forged.encode(),
                organization.to_string(),
                second.len() as i64 - 1
            ],
        )
        .unwrap();
    assert!(
        matches!(
            owner.checked_organization().await,
            Err(ClientError::Forged(forged)) if forged == organization
        ),
        "a rewritten log is refused as forged"
    );
}
