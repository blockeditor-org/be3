use super::*;

use be_block_graph::Access;
use be_block_keys::Level;
use be_block_protocol::MemberKind;

/// Access from a block above counts on both sides of a hosted block's channel
/// (plans/hosted_blocks.md): an editor whose Edit comes from the workspace the world lies in may
/// host it, a Join member checks that Edit in the workspace's log, by the world's signed parent
/// records, and a member who views the workspace is admitted with View, which holds Join.
#[tokio::test]
async fn a_host_and_a_member_whose_access_comes_from_above_meet() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let editor = harness.member(&owner, "editor@example.com").await;
    let viewer = harness.member(&owner, "viewer@example.com").await;
    let player = harness.member(&owner, "player@example.com").await;
    player.refresh_contacts().await.unwrap();
    let games = workspace(&owner, "Games").await;
    let world = owner
        .create::<TextContent>(BlockParent::Block(games))
        .await
        .unwrap();
    owner
        .grant(games, editor.account(), Access::Edit)
        .await
        .unwrap();
    owner
        .grant(games, viewer.account(), Access::View)
        .await
        .unwrap();
    owner
        .grant(world, player.account(), Access::Join)
        .await
        .unwrap();
    owner.tend().await.unwrap();
    for member in [&editor, &viewer] {
        member.refresh_keys(None).await.unwrap();
    }

    editor
        .connection()
        .join(editor.organization(), world, BUILD)
        .await
        .unwrap();
    editor.connection().claim(world).await.unwrap();
    for (member, kind) in [
        (&player, MemberKind::Join),
        (&viewer, MemberKind::Participant),
    ] {
        let joined = member
            .connection()
            .join(member.organization(), world, BUILD)
            .await
            .unwrap();
        assert_eq!(joined.joined_as, kind);
        assert_eq!(
            joined.host.map(|host| host.account),
            Some(editor.account())
        );
    }

    for (member, level) in [(&player, Level::Join), (&viewer, Level::View)] {
        let (joining, first) = member.join_host(world, editor.account()).await.unwrap();
        let (admitting, answer) = editor.admit(world, &first).await.unwrap();
        assert_eq!(
            (admitting.account(), admitting.level()),
            (member.account(), level)
        );
        let mut channel = joining.finish(&answer).unwrap();
        let (admitted, said) = admitting.confirm(&channel.seal(b"here")).unwrap();
        assert_eq!(said, b"here");
        assert_eq!(admitted.level, level);
    }

    // The owner, the world's creator, may host too; an account the logs give only View may not.
    assert!(player.join_host(world, owner.account()).await.is_ok());
    assert!(matches!(
        player.join_host(world, viewer.account()).await,
        Err(ClientError::Forged(_))
    ));

    harness.stop().await;
}
