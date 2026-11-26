use be_block_content::BlockContent;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// The laptop types into a text first, so the session's order is past anything this device
/// heard. This device then opens it online, its first join answered with the server's journal,
/// and keeps that journal: offline, what it types is numbered after the entries its replica
/// opened on, not from the start of an order of its own, which the replica would take for
/// entries it applied and pass over, holding the edit unjournaled for good.
#[test]
fn an_edit_made_offline_after_joining_a_session_another_device_numbered_is_taken() {
    with_another_device(|peer, laptop, shared, mut replicas| async move {
        let block = peer
            .create::<TextContent>(BlockParent::Root)
            .await
            .expect("the text is made");
        peer.save(block, &TextBlock::of(""), None)
            .await
            .expect("the empty text is published");

        let laptop = Arc::new(laptop);
        let laptop_shared = Arc::new(Mutex::new(Quiet::default()));
        let mut laptops = Replicas::new(test_modules(be_block_client::Clock::real()).await);
        laptops
            .open(
                (&laptop, &laptop_shared),
                block,
                TextContent::CONTENT_TYPE,
                true,
            )
            .await
            .expect("the laptop opens the text");
        while laptops.joining() {
            laptops.woken().await;
            laptops.pump((&laptop, &laptop_shared)).await;
        }
        let mut mirror =
            attach_mirror::<TextContent>(&laptop, &laptop_shared, &mut laptops, block, 7).await;
        type_at_start(&mut laptops, block, 7, &mut mirror, "socks");
        laptops.pump((&laptop, &laptop_shared)).await;
        // Numbered by the server once its submission is answered.
        while laptops
            .submissions_of(block)
            .is_none_or(|(submitting, _, kept)| submitting > 0 || kept.is_empty())
        {
            laptops.woken().await;
            laptops.pump((&laptop, &laptop_shared)).await;
        }

        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await
            .expect("the text opens");
        while replicas.joining() {
            step(&peer, &shared, &mut replicas).await;
        }
        let kept = peer
            .local_journal(block)
            .expect("the journal reads")
            .map(|journal| journal.entries.len());
        assert!(
            kept.is_some_and(|entries| entries > 0),
            "this device keeps the journal its first join opened on, not {kept:?}"
        );

        peer.detach();
        replicas.disconnected();
        let editor = 8;
        let mut mirror =
            attach_mirror::<TextContent>(&peer, &shared, &mut replicas, block, editor).await;
        type_at_start(&mut replicas, block, editor, &mut mirror, "boots ");
        replicas.pump((&peer, &shared)).await;
        while replicas
            .status(block)
            .is_none_or(|status| !status.journaled)
        {
            step(&peer, &shared, &mut replicas).await;
        }
        let bytes = replicas
            .content((&peer, &shared), block)
            .await
            .expect("the text is open");
        let text = TextBlock::text(&TextContent::decode(&bytes).expect("the text decodes"));
        assert_eq!(text, "boots socks", "the edit typed offline is taken");
    });
}
