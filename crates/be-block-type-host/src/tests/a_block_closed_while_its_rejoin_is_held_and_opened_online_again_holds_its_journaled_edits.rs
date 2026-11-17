use be_block_content::BlockContent;
use be_type_text::TextContent;

use super::*;

/// A block closed while its rejoin is held lets go of the join rather than open again on the
/// server's journal. Its replica seals in its made-up session as it closes, so opened online
/// again, the text holds the journaled edits no seal held, once: the hold is one the closing seal
/// can take (a seal backing off, a reload on its way), not a divergence.
#[test]
fn a_block_closed_while_its_rejoin_is_held_and_opened_online_again_holds_its_journaled_edits() {
    let clock = be_block_client::Clock::manual();
    with_test_types_on(clock, |peer, shared, mut replicas| async move {
        let block = text_whose_rejoin_is_held(&peer, &shared, &mut replicas).await;

        replicas.close(block);
        while replicas.is_open(block) {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }

        let openings = replicas.openings.len();
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await
            .expect("the block opens again");
        opened_again(&peer, &shared, &mut replicas, block, openings).await;
        assert_eq!(
            text_of(&peer, &shared, &mut replicas, block).await,
            "socks",
            "the edit made offline survives"
        );
    });
}
