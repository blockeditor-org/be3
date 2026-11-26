use super::*;

/// A block reopened while its rejoin is held (as keys arriving reopen it) lets go of the join
/// as a close does, and the replica opened in its place, online, holds the journaled edits no
/// seal held, once: the hold is one the closing seal can take (a seal backing off, a reload on
/// its way), not a divergence.
#[test]
fn a_block_reopened_while_its_rejoin_is_held_opens_again_holding_its_journaled_edits() {
    let clock = be_block_client::Clock::manual();
    with_test_types_on(clock, |peer, shared, mut replicas| async move {
        let block = text_whose_rejoin_is_held(&peer, &shared, &mut replicas).await;

        let openings = replicas.openings.len();
        replicas.reopen(block);
        opened_again(&peer, &shared, &mut replicas, block, openings).await;
        assert_eq!(
            text_of(&peer, &shared, &mut replicas, block).await,
            "socks",
            "the edit made offline survives"
        );
    });
}
