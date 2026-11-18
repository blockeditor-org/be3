use super::*;

use be_block_graph::Access;
use be_block_keys::noise::{Responder, hosted_prologue};
use be_block_keys::{Entry, Grant, Level, LogKind, Record};
use be_block_protocol::MemberKind;

use crate::Event;

/// The next message on `block`'s host channel heard on `events`: who sent it, and what.
pub(super) async fn hosted(
    events: &mut tokio::sync::broadcast::Receiver<Event>,
    block: Uuid,
) -> (be_block_protocol::ClientId, Vec<u8>) {
    loop {
        if let Event::Hosted {
            block: of,
            from,
            payload,
            ..
        } = events.recv().await.unwrap()
            && of == block
        {
            return (from, payload);
        }
    }
}

/// A Join member checks the account the server names as a hosted block's host in the block's
/// access log before it speaks to it (plans/hosted_blocks.md, "The channel"): only an account
/// the log gives Edit or above may host, and only with the identity key that log names for it,
/// so a server that names an account without Edit, answers with a key of its own, or serves a
/// log of its own making with a host of its own, is refused. Holding no key of the block, the
/// member takes the log's root from a grant to it signed by a contact it verified. The honest
/// handshake goes over the session's host channel, and the server can neither read nor replay
/// what the channel then carries.
#[tokio::test]
async fn a_join_member_refuses_a_host_the_server_makes_up() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let viewer = harness.member(&owner, "viewer@example.com").await;
    let player = harness.member(&owner, "player@example.com").await;
    let stranger = harness.member(&owner, "stranger@example.com").await;
    player.refresh_contacts().await.unwrap();
    let games = workspace(&owner, "Games").await;
    let world = owner
        .create::<TextContent>(BlockParent::Block(games))
        .await
        .unwrap();
    owner
        .grant(world, viewer.account(), Access::View)
        .await
        .unwrap();
    owner
        .grant(world, player.account(), Access::Join)
        .await
        .unwrap();

    let mut on_owner = owner.connection().subscribe();
    let mut on_player = player.connection().subscribe();
    let hosting = owner
        .connection()
        .join(owner.organization(), world, BUILD)
        .await
        .unwrap();
    owner.connection().claim(world).await.unwrap();
    let joined = player
        .connection()
        .join(player.organization(), world, BUILD)
        .await
        .unwrap();
    assert_eq!(joined.joined_as, MemberKind::Join);
    let host = joined.host.expect("the owner hosts");
    assert_eq!(
        (host.client, host.account),
        (hosting.client, owner.account())
    );

    let (joining, first) = player.join_host(world, host.account).await.unwrap();
    assert_eq!(joining.host(), owner.account());
    player
        .connection()
        .send_host(world, None, first.clone())
        .await
        .unwrap();
    let (from, heard) = hosted(&mut on_owner, world).await;
    assert_eq!(heard, first);
    let (admitting, answer) = owner.admit(world, &heard).await.unwrap();
    assert_eq!(
        (admitting.account(), admitting.level()),
        (player.account(), Level::Join)
    );
    owner
        .connection()
        .send_host(world, Some(from), answer.clone())
        .await
        .unwrap();
    let (_, heard) = hosted(&mut on_player, world).await;
    let mut channel = joining.finish(&heard).unwrap();
    let sealed = channel.seal(b"move north");
    assert!(
        !sealed
            .windows(b"move north".len())
            .any(|window| window == b"move north"),
        "a message crosses the server sealed"
    );
    player
        .connection()
        .send_host(world, None, sealed)
        .await
        .unwrap();
    let (_, heard) = hosted(&mut on_owner, world).await;
    let (mut admitted, said) = admitting.confirm(&heard).unwrap();
    assert_eq!(said, b"move north", "the member's first message admits it");
    assert_eq!(channel.handshake_hash(), admitted.channel.handshake_hash());
    let back = admitted.channel.seal(b"you see a door");
    assert_eq!(channel.open(&back).unwrap(), b"you see a door");
    assert!(
        channel.open(&back).is_err(),
        "a message the server replays is refused"
    );

    // The server replays the player's first message: the host answers, but no one there holds
    // the keys to say anything on that channel, so no one is admitted.
    let (replayed, _) = owner.admit(world, &first).await.unwrap();
    assert!(
        replayed.confirm(&heard).is_err(),
        "a replayed handshake admitted someone"
    );

    // The server names as host an account the log gives only View, or nothing.
    for (who, named) in [
        ("a viewer", viewer.account()),
        ("a stranger", stranger.account()),
    ] {
        assert!(
            matches!(player.join_host(world, named).await, Err(ClientError::Forged(block)) if block == world),
            "{who} was taken as the host"
        );
    }

    // The server answers in the host's place: it cannot read the first message, made for the
    // host's key, and an answer of its own, or one replayed from another handshake, opens
    // nothing.
    let (joining, first) = player.join_host(world, owner.account()).await.unwrap();
    let (head, message) = first.split_at(32);
    let prologue = hosted_prologue(world.into_bytes(), head.try_into().unwrap());
    assert!(Responder::read(stranger.keys().identity(), &prologue, message).is_err());
    assert!(matches!(
        joining.finish(&answer),
        Err(ClientError::Forged(_))
    ));
    let (joining, _) = player.join_host(world, owner.account()).await.unwrap();
    let mut made_up = be_block_keys::RecoverySecret::from_bytes([7; 32])
        .public()
        .0
        .to_vec();
    made_up.extend([0; 16]);
    assert!(matches!(
        joining.finish(&made_up),
        Err(ClientError::Forged(_))
    ));

    // The server serves a log of its own making, rooted at a stranger it works with, who grants
    // the player Join and holds the block: no contact of the player's vouches for it.
    let root = Entry::root(
        stranger.keys().identity(),
        stranger.account().into_bytes(),
        LogKind::Block,
        world.into_bytes(),
    );
    let grant = Entry::sign(
        stranger.keys().identity(),
        stranger.account().into_bytes(),
        world.into_bytes(),
        Some(root.hash()),
        false,
        Record::Grant(Grant {
            grantee: player.account().into_bytes(),
            identity: player.keys().identity().identity(),
            level: Level::Join,
            authority: be_block_keys::Authority::Log,
        }),
    );
    let database = harness.database();
    database
        .execute(
            "DELETE FROM access_log WHERE block_id = ?1",
            rusqlite::params![world.to_string()],
        )
        .unwrap();
    for (position, entry) in [root, grant].iter().enumerate() {
        database
            .execute(
                "INSERT INTO access_log (organization_id, block_id, position, entry)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    owner.organization().to_string(),
                    world.to_string(),
                    position as i64,
                    entry.encode()
                ],
            )
            .unwrap();
    }
    drop(database);
    assert!(
        matches!(player.join_host(world, stranger.account()).await, Err(ClientError::Forged(block)) if block == world),
        "a log no contact vouches for was taken"
    );

    harness.stop().await;
}
