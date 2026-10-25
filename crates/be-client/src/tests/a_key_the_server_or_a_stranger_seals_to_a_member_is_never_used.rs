use super::*;

use be_graph::Access;
use be_keys::Party;
use be_protocol::WrappedBy;
use text_block_types::{TextBlock, TextContent};

fn inject(
    harness: &Harness,
    organization: Uuid,
    block: Uuid,
    epoch: i64,
    to: Uuid,
    sender: Uuid,
    seal: impl FnOnce(Uuid) -> Vec<u8>,
) -> Uuid {
    let key = Uuid::new_v4();
    let database = harness.database();
    database
        .execute(
            "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
             VALUES (?1, ?2, ?3, 'scope', ?4, 0)",
            rusqlite::params![
                organization.to_string(),
                key.to_string(),
                block.to_string(),
                epoch
            ],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                organization.to_string(),
                key.to_string(),
                format!("account:{to}"),
                sender.to_string(),
                seal(key)
            ],
        )
        .unwrap();
    key
}

#[tokio::test]
async fn a_key_the_server_or_a_stranger_seals_to_a_member_is_never_used() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let victim = harness.member(&owner, "victim@example.com").await;
    let stranger = harness.member(&owner, "stranger@example.com").await;
    let organization = owner.organization();
    let team = workspace(&owner, "Team").await;
    let public = owner.keys().contact(victim.account()).unwrap();
    owner
        .share(team, victim.account(), Access::Edit, &public)
        .await
        .unwrap();
    let honest = owner.keys().scope_key(team).unwrap();

    let known_to_the_server = [9u8; 32];
    let victim_public = victim.keys().identity().public();
    let anonymous = inject(
        &harness,
        organization,
        team,
        5,
        victim.account(),
        owner.account(),
        |_| victim_public.seal_anonymous(&known_to_the_server),
    );
    let from_a_stranger = inject(
        &harness,
        organization,
        team,
        6,
        victim.account(),
        stranger.account(),
        |key| {
            stranger.keys().identity().seal_for(
                stranger.account().into_bytes(),
                &Party {
                    account: victim.account().into_bytes(),
                    public: victim_public,
                },
                key.as_bytes(),
                &known_to_the_server,
            )
        },
    );

    let reopened = harness.second(&victim).await;
    for injected in [anonymous, from_a_stranger] {
        assert!(
            !reopened.keys().has(injected),
            "a key sealed by someone this device never verified was taken"
        );
    }
    assert_eq!(reopened.keys().scope_key(team), Some(honest));

    let note = reopened
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    reopened
        .save(note, &TextBlock::of("for the owner only"), None)
        .await
        .unwrap();
    let key = reopened.current_key(note).unwrap();
    let wrapped = reopened.keys().wrapped_by(key);
    assert!(wrapped.contains(&WrappedBy::Key(honest)), "{wrapped:?}");
    assert!(
        !wrapped.contains(&WrappedBy::Key(anonymous))
            && !wrapped.contains(&WrappedBy::Key(from_a_stranger)),
        "the new block's key was wrapped under a key the server knows"
    );
    assert_eq!(text(&owner, note).await, "for the owner only");

    harness.stop().await;
}
