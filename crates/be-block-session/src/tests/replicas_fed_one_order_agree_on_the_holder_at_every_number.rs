use be_block_protocol::{MembershipChange, MembershipEntry, SessionState};

use super::*;

/// What a participant hears of a session, in the one order the server sends it.
#[derive(Clone)]
enum Heard {
    Numbered(u64),
    Membership(MembershipEntry),
}

/// The holder each number was applied under.
fn holders(mut state: SessionState, heard: &[Heard]) -> Vec<(u64, Option<u64>)> {
    heard
        .iter()
        .filter_map(|heard| match heard {
            Heard::Numbered(sequence) => Some((*sequence, state.holder())),
            Heard::Membership(entry) => {
                state.apply(entry);
                None
            }
        })
        .collect()
}

/// The server writes membership between submissions; a replica that heard all of it and one
/// that joined part way, from the membership as of the number it joined at and hearing some
/// entries again, agree on who held the lease at every number both applied.
#[test]
fn replicas_fed_one_order_agree_on_the_holder_at_every_number() {
    let mut server = SessionState::default();
    let mut heard = Vec::new();
    let mut sequence = 0;
    let mut snapshot = None;
    let write = |server: &mut SessionState,
                     heard: &mut Vec<Heard>,
                     sequence: u64,
                     client,
                     change| {
        if let Some(entry) = server.write(sequence, client, change) {
            heard.push(Heard::Membership(entry));
        }
    };
    write(&mut server, &mut heard, sequence, 1, joined(true, TAG));
    write(&mut server, &mut heard, sequence, 2, joined(true, TAG));
    for step in 0..40u64 {
        sequence += 1;
        heard.push(Heard::Numbered(sequence));
        match step % 8 {
            1 => write(&mut server, &mut heard, sequence, 3, joined(step % 16 == 1, TAG)),
            3 => write(&mut server, &mut heard, sequence, 2, MembershipChange::Claimed),
            4 => write(&mut server, &mut heard, sequence, 1, MembershipChange::Access(false)),
            5 => write(&mut server, &mut heard, sequence, 2, MembershipChange::Left),
            6 => write(&mut server, &mut heard, sequence, 1, MembershipChange::Access(true)),
            7 => write(&mut server, &mut heard, sequence, 2, joined(true, TAG)),
            _ => {}
        }
        if step == 17 {
            snapshot = Some((heard.len(), server.clone()));
        }
    }
    let all = holders(SessionState::default(), &heard);
    let (from, state) = snapshot.unwrap();
    // The joiner hears what was written since its snapshot, and again the last membership
    // entries before it, as a stream that raced its join would bring them.
    let again = heard[..from]
        .iter()
        .filter(|heard| matches!(heard, Heard::Membership(_)))
        .rev()
        .take(2)
        .cloned()
        .collect::<Vec<_>>();
    let late = holders(
        state,
        &again.into_iter().rev().chain(heard[from..].iter().cloned()).collect::<Vec<_>>(),
    );
    assert!(late.len() > 10);
    for (sequence, holder) in &late {
        let (_, all_holder) = all.iter().find(|(at, _)| at == sequence).unwrap();
        assert_eq!(holder, all_holder, "holder at {sequence}");
    }
    assert!(all.iter().any(|(_, holder)| *holder == Some(1)));
    assert!(all.iter().any(|(_, holder)| *holder == Some(2)));
}
