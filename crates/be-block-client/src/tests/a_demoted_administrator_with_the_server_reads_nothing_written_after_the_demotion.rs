use super::*;

use std::collections::HashSet;

use be_block_protocol::OrganizationRole;

/// Every key `holder` opens with the server's help: each key it holds, and each key the server
/// holds wrapped under one of them or sealed to its account, to a fixed point. The server
/// serves a demoted administrator whatever it likes, so this is what they read with it.
pub(super) fn opened_with_the_server(
    harness: &Harness,
    holder: &Peer<MemoryStore>,
) -> HashSet<String> {
    let database = harness.database();
    let mut keys = database.prepare("SELECT id FROM keys").unwrap();
    let mut opened: HashSet<String> = keys
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .filter(|id| holder.keys().has(id.parse().unwrap()))
        .collect();
    opened.insert(format!("account:{}", holder.account()));
    let mut wraps = database
        .prepare("SELECT key_id, wrapped_by FROM wrapped_keys")
        .unwrap();
    let wraps: Vec<(String, String)> = wraps
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    loop {
        let before = opened.len();
        for (key, by) in &wraps {
            if opened.contains(by) {
                opened.insert(key.clone());
            }
        }
        if opened.len() == before {
            return opened;
        }
    }
}

/// An administrator reads every block through escrow. Demoted, they keep every epoch they
/// reached that way, and the server can serve them whatever is sealed under those. So a
/// demotion outdates every epoch under the escrow made before it: a member's next write in a
/// scope gives it a new epoch first, from the highest scope above it that is outdated too, and
/// what they write after the demotion opens with no key the demoted administrator and the
/// server hold together (signed access 8).
#[tokio::test]
async fn a_demoted_administrator_with_the_server_reads_nothing_written_after_the_demotion() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let team = workspace(&member, "Team").await;
    let note = member
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let head = member
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    // A scope of its own inside the workspace: its next epoch is wrapped under the workspace's.
    let inner = member
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let inner_head = member
        .save(inner, &TextBlock::of("inner before"), None)
        .await
        .unwrap()
        .published();
    member
        .grant(inner, admin.account(), Access::View)
        .await
        .unwrap();
    member.tend().await.unwrap();
    assert_eq!(
        crate::peer::scope_of(&member.summary(inner).await.unwrap()).unwrap(),
        inner,
        "the inner block is no scope of its own, so the test shows less"
    );

    admin
        .set_role(deputy.account(), OrganizationRole::Administrator)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    let promoted = harness.second(&deputy).await;
    assert_eq!(text(&promoted, note).await, "before");
    assert_eq!(text(&promoted, inner).await, "inner before");
    let team_before = member.keys().scope_key(team).unwrap();

    admin
        .set_role(deputy.account(), OrganizationRole::Editor)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    // Told key work changed, as every member's devices are of a demotion: tending reads the
    // organization's log.
    member.tend().await.unwrap();
    let before = opened_with_the_server(&harness, &promoted);
    assert!(
        before.contains(&team_before.to_string()),
        "the deputy never reached the workspace through escrow, so the test shows nothing"
    );

    let after = member
        .save(inner, &TextBlock::of("inner after"), inner_head)
        .await
        .unwrap()
        .published()
        .unwrap();
    // One rotation from the workspace covers the scope inside it.
    let team_after = member.keys().scope_key(team).unwrap();
    assert_ne!(
        team_after, team_before,
        "the workspace above was not rotated with the scope inside it"
    );
    let saved = member
        .save(note, &TextBlock::of("after"), head)
        .await
        .unwrap()
        .published()
        .unwrap();
    let opened = opened_with_the_server(&harness, &promoted);
    for (commit, what) in [(after, "inner"), (saved, "note")] {
        let key = member.commits().vault().sealed_under(commit.hash()).unwrap();
        assert!(
            !opened.contains(&key.to_string()),
            "the demoted administrator, with the server, opens the {what} written after the demotion"
        );
    }
    assert!(!opened.contains(&team_after.to_string()));

    let laptop = harness.second(&admin).await;
    assert_eq!(text(&laptop, note).await, "after");
    assert_eq!(text(&laptop, inner).await, "inner after");

    harness.stop().await;
}
