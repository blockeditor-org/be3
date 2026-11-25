use super::*;

/// A player holds no key of the block: it waits for a host, shows what the host tells it, and
/// digs by asking the host, which alone writes the dig.
#[test]
fn a_player_sees_only_what_the_host_tells_it_and_digs_by_asking() {
    let mut test = BeuiTest::<HuntRoot>::joined(Uuid::new_v4());
    assert_eq!(
        shown(&test, "hunt.status"),
        "\"Waiting for someone to host the hunt.\""
    );

    let host = Uuid::new_v4();
    test.hear_channel(ChannelEvent::Open(host.into_bytes()));
    assert_eq!(shown(&test, "hunt.status"), "\"Joining the hunt.\"");

    let board = Board::of(&world(test.host().account_id()).root(), test.host().account_id());
    test.hear_channel(ChannelEvent::Said(encode(&board)));
    assert_eq!(
        shown(&test, "hunt.status"),
        "\"1 of 6 gems found. You found 1.\""
    );
    test.snapshot("a_player_sees_only_what_the_host_tells_it_and_digs_by_asking");

    told(&mut test);
    test.click("hunt.cell.35");
    let asked: Vec<Option<Ask>> = told(&mut test)
        .into_iter()
        .map(|command| match command {
            BlockCommand::TellHost(payload) => decode(&payload),
            _ => None,
        })
        .collect();
    assert_eq!(asked, [Some(Ask::Dig(35))], "a press asks the host to dig");

    told(&mut test);
    test.click("hunt.cell.0");
    assert!(told(&mut test).is_empty(), "a dug cell asks nothing");

    test.hear_channel(ChannelEvent::Closed);
    assert_eq!(
        shown(&test, "hunt.status"),
        "\"Waiting for someone to host the hunt.\""
    );
}
