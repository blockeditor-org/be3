use be_block_graph::Access;
use be_block_protocol::{JoinedMember, JoinerChange, MemberKind, SessionHost};

use super::*;

/// What `client` heard of `block`'s session, once two round trips let every push queued for it
/// arrive; forgotten as it is read.
async fn heard(client: &mut TestClient, block: Uuid) -> Vec<Notice> {
    client.round_trip().await;
    client.round_trip().await;
    let (heard, rest) = std::mem::take(&mut client.notifications)
        .into_iter()
        .partition(|notice| match notice {
            Notice::Membership { block: of, .. }
            | Notice::Relayed { block: of, .. }
            | Notice::Sequenced { block: of, .. }
            | Notice::Hosted { block: of, .. }
            | Notice::Host { block: of, .. }
            | Notice::Joiner { block: of, .. } => *of == block,
            _ => false,
        });
    client.notifications = rest;
    heard
}

/// A Join member joins a block's session as a member of its own kind (guides/blocks/hosted.md):
/// it learns who hosts, and nothing of the membership or the order; only the lease holder hears
/// it came or went; what it sends on the host channel reaches the holder alone, and what the
/// holder sends it reaches it alone, so another Join member hears nothing of either, not even
/// that it was sent. Nobody else may send on the channel but to the holder, and what a member
/// sends the holder is rate limited.
#[tokio::test]
async fn a_join_members_message_reaches_only_the_host() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    owner.personal().await;
    let owner_id = owner.id();
    let mut viewer = harness.member("viewer@example.com").await;
    let mut first = harness.member("first@example.com").await;
    let mut second = harness.member("second@example.com").await;
    for other in [&mut viewer, &mut first, &mut second] {
        other.personal().await;
        let link = owner.link().await;
        let accepted = other.accept(link, &[]).await;
        assert!(accepted.is_ok(), "{accepted:?}");
    }
    let games = owner.create_block(BlockParent::Root).await;
    let world = owner.create_block(BlockParent::Block(games)).await;
    for (account, access) in [
        (viewer.id(), Access::View),
        (first.id(), Access::Join),
        (second.id(), Access::Join),
    ] {
        let granted = owner.grant(world, account, access).await;
        assert!(granted.is_ok(), "{granted:?}");
    }

    let joined = owner.join_as_member(world).await.expect("joining failed");
    let host = joined.client;
    owner.claim(world).await.expect("claiming failed");
    let hosting = Some(SessionHost {
        client: host,
        account: owner_id,
    });

    let mut members = Vec::new();
    for player in [&mut first, &mut second] {
        let joined = player.join_as_member(world).await.expect("joining failed");
        assert_eq!(joined.joined_as, MemberKind::Join);
        assert_eq!(joined.host, hosting, "a Join member learns who hosts");
        assert!(
            joined.state.participants.is_empty() && joined.journal.is_none() && !joined.may_edit,
            "a Join member is told nothing of the membership or the order"
        );
        members.push((joined.client, player.id()));
    }
    let [(first_client, first_id), (second_client, second_id)] = members[..] else {
        unreachable!();
    };
    let joined = viewer.join_as_member(world).await.expect("joining failed");
    assert_eq!(joined.joined_as, MemberKind::Participant);
    assert_eq!(joined.host, hosting);
    let viewer_client = joined.client;

    let told: Vec<JoinerChange> = heard(&mut owner, world)
        .await
        .into_iter()
        .filter_map(|notice| match notice {
            Notice::Joiner { change, .. } => Some(change),
            _ => None,
        })
        .collect();
    assert_eq!(
        told,
        vec![
            JoinerChange::Joined(JoinedMember {
                client: first_client,
                account: first_id,
            }),
            JoinerChange::Joined(JoinedMember {
                client: second_client,
                account: second_id,
            }),
        ],
        "the host hears each Join member join, with its account"
    );
    assert!(
        heard(&mut viewer, world)
            .await
            .iter()
            .all(|notice| !matches!(notice, Notice::Joiner { .. })),
        "a viewer heard a Join member join"
    );
    for player in [&mut first, &mut second] {
        assert!(
            heard(player, world).await.is_empty(),
            "a Join member heard of the viewer joining"
        );
    }

    let sent = first.send_to_host(world, None, b"move north".to_vec()).await;
    assert!(sent.is_ok(), "{sent:?}");
    assert_eq!(
        heard(&mut owner, world).await,
        vec![Notice::Hosted {
            block: world,
            from: first_client,
            payload: b"move north".to_vec(),
        }],
        "the host hears the Join member's message"
    );
    for (who, other) in [
        ("the other Join member", &mut second),
        ("a viewer", &mut viewer),
    ] {
        assert!(
            heard(other, world).await.is_empty(),
            "{who} heard of a message to the host"
        );
    }

    let sent = owner
        .send_to_host(world, Some(first_client), b"you see a door".to_vec())
        .await;
    assert!(sent.is_ok(), "{sent:?}");
    assert_eq!(
        heard(&mut first, world).await,
        vec![Notice::Hosted {
            block: world,
            from: host,
            payload: b"you see a door".to_vec(),
        }]
    );
    assert!(heard(&mut second, world).await.is_empty());
    assert!(heard(&mut viewer, world).await.is_empty());

    // A viewer speaks to the host on the same channel, and its relays to everyone reach no
    // Join member.
    let sent = viewer.send_to_host(world, None, b"a viewer".to_vec()).await;
    assert!(sent.is_ok(), "{sent:?}");
    assert!(matches!(
        &heard(&mut owner, world).await[..],
        [Notice::Hosted { from, .. }] if *from == viewer_client
    ));
    let relayed = viewer.relay(world, None, b"presence".to_vec()).await;
    assert!(relayed.is_ok(), "{relayed:?}");
    for player in [&mut first, &mut second] {
        assert!(heard(player, world).await.is_empty());
    }

    for (what, refused) in [
        (
            "a Join member sent to another",
            first
                .send_to_host(world, Some(second_client), b"psst".to_vec())
                .await,
        ),
        (
            "a viewer sent to a Join member",
            viewer
                .send_to_host(world, Some(first_client), b"psst".to_vec())
                .await,
        ),
        (
            "the host sent to itself",
            owner.send_to_host(world, Some(host), b"psst".to_vec()).await,
        ),
        (
            "the host sent to no one",
            owner.send_to_host(world, None, b"psst".to_vec()).await,
        ),
        (
            "a Join member relayed to the session",
            first.relay(world, None, b"psst".to_vec()).await,
        ),
        ("a Join member claimed the lease", first.claim(world).await),
    ] {
        assert!(
            refused_as(&refused, ErrorCode::PermissionDenied),
            "{what}: {refused:?}"
        );
    }
    let submitted = first.submit(world, 1, 1, 0, b"op".to_vec()).await;
    assert!(
        refused_as(&submitted, ErrorCode::PermissionDenied),
        "a Join member submitted to the order: {submitted:?}"
    );
    assert!(heard(&mut second, world).await.is_empty());

    // What one member sends the host is held back past a burst.
    let mut sent = 0;
    let mut flooded = Vec::new();
    let limited = loop {
        let answer = second.send_to_host(world, None, vec![sent as u8]).await;
        match answer {
            Ok(()) => sent += 1,
            Err(error) => break error,
        }
        // The host reads as it goes, as a host does.
        if sent % 16 == 0 {
            flooded.extend(heard(&mut owner, world).await);
        }
        assert!(
            sent <= 2 * crate::limits::HOST_MESSAGE_BURST as usize,
            "a Join member sent the host {sent} messages at once"
        );
    };
    assert_eq!(limited.code, ErrorCode::SlowDown, "{limited:?}");
    assert!(sent >= crate::limits::HOST_MESSAGE_BURST as usize);
    assert!(heard(&mut first, world).await.is_empty());
    flooded.extend(heard(&mut owner, world).await);
    assert_eq!(
        flooded
            .iter()
            .filter(
                |notice| matches!(notice, Notice::Hosted { from, .. } if *from == second_client)
            )
            .count(),
        sent,
        "the host heard what was let through: {:?}",
        flooded
            .iter()
            .filter(|notice| !matches!(notice, Notice::Hosted { .. }))
            .collect::<Vec<_>>()
    );

    first.leave(world).await;
    let all = heard(&mut owner, world).await;
    let left: Vec<Notice> = all
        .iter()
        .filter(|notice| matches!(notice, Notice::Joiner { .. }))
        .cloned()
        .collect();
    assert_eq!(
        left,
        vec![Notice::Joiner {
            block: world,
            change: JoinerChange::Left(first_client),
        }],
        "the host hears a Join member leave: {all:?}"
    );

    owner.leave(world).await;
    assert!(
        heard(&mut second, world).await.contains(&Notice::Host {
            block: world,
            host: None
        }),
        "a Join member hears that no one hosts"
    );
    let sent = second.send_to_host(world, None, b"anyone?".to_vec()).await;
    assert!(
        refused_as(&sent, ErrorCode::InvalidRequest),
        "a message to no host: {sent:?}"
    );

    harness.stop().await;
}
