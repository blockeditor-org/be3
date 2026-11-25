use std::{cell::Cell, collections::HashMap, rc::Rc};

use super::*;

/// A reader outside the session (`replay`) of a journal whose base state goes by a part (a
/// large session with a peer in it) and that holds an edit too large to relay reads both parts
/// and sees the journal's edits, as a replica opening on it does, rather than the head alone.
#[test]
fn a_reader_outside_the_session_replays_a_journal_whose_base_and_edit_go_by_parts() {
    crate::replica::LARGEST_INLINE.with(|largest| largest.set(64));
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.largest_base_state = 128;
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.type_at(0, 1, 0, "start\n");
    for round in 0..40 {
        sim.type_at(0, 1, 0, &format!("scratch {round} "));
        let typed = sim.typed(0, 1).find("start").expect("the text is there");
        sim.erase(0, 1, 0..typed);
    }
    sim.type_at(1, 1, 0, "b ");
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(sim.base_parts.len(), 1, "the base goes by its part");

    let long = "x".repeat(100);
    sim.type_at(1, 1, 0, &format!("{long} "));
    sim.type_at(1, 1, 0, "c ");
    sim.settle();
    let expected = format!("c {long} b start\n");
    assert_eq!(prose(&sim.content(0)), expected);

    let journal = sim.journal.clone().expect("a journal");
    let mut parts: Vec<be_block_store::PartRef> = crate::wire::BaseState::decode(&journal.base.state)
        .and_then(|state| state.part)
        .into_iter()
        .collect();
    parts.extend(journal.entries.iter().filter_map(|entry| {
        match crate::wire::Submitted::decode(&entry.payload)?.entry {
            crate::wire::Entry::Operation(be_block_session::Payload::Part(part)) => Some(part),
            _ => None,
        }
    }));
    assert_eq!(parts.len(), 2, "the base's state and the long edit go by parts");
    let stored: HashMap<be_block_store::PartRef, Vec<u8>> = parts
        .iter()
        .map(|part| {
            let bytes = sim
                .vault
                .read_part(*part, 0, part.length())
                .expect("the part was put");
            (*part, bytes)
        })
        .collect();
    let reads = Rc::new(Cell::new(0));
    let reader = {
        let reads = Rc::clone(&reads);
        be_block_content::PartReader::new(move |part, offset, length| {
            reads.set(reads.get() + 1);
            let read = stored
                .get(&part)
                .map(|bytes| bytes[offset as usize..][..length as usize].to_vec())
                .ok_or_else(|| "a part the journal does not name".to_owned());
            Box::pin(std::future::ready(read))
        })
    };
    let head = sim.commits[&sim.head.expect("a head")].bytes.clone();
    let replayed = pollster::block_on(crate::replica::replay::<ProseContent>(
        Some(head),
        journal,
        reader,
    ))
    .expect("the journal replays");
    assert_eq!(prose(&ProseContent::decode(&replayed).unwrap()), expected);
    assert_eq!(reads.get(), 2, "each part is read once");
}
