use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use be_block::{Merge, PartReader};

use super::*;

thread_local! {
    /// The part the merge below reads, and what it read.
    static PART: Cell<Option<be_block::PartRef>> = const { Cell::new(None) };
    static READ: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A merge by parts that reads a saved part before it merges.
fn reading_merge(
    base: ChecklistContent,
    ours: ChecklistContent,
    theirs: ChecklistContent,
    parts: PartReader,
) -> Pin<Box<dyn Future<Output = Result<be_block_commit::MergeResult<ChecklistContent>, String>>>> {
    Box::pin(async move {
        let part = PART.with(Cell::get).ok_or("no part")?;
        let bytes = parts.read_all(part).await?;
        READ.with(|read| *read.borrow_mut() = bytes);
        Ok(ChecklistContent::merge3(&base, &ours, &theirs))
    })
}

fn merging_module() -> Module {
    let mut registration = Registration::of::<ChecklistContent>().undo::<ChecklistContent>();
    registration.replica_merge = Some(Rc::new(reading_merge as crate::replica::PartsMerge<_>));
    Module::new(vec![registration])
}

fn diverged() -> (Sim<ChecklistContent>, CommitId, CommitId) {
    let mut sim = Sim::<ChecklistContent>::new(1, merging_module);
    let mut base = ChecklistContent::default();
    let (first, created) = Checklist::add("first");
    base.apply(&created);
    let (second, created) = Checklist::add("second");
    base.apply(&created);
    let shared = sim.publish_elsewhere(&base);
    let edit = |item, text: &str| {
        let mut edited = base.clone();
        edited.apply(&Checklist::set_text(item, text));
        edited
    };
    let theirs = sim.publish_elsewhere(&edit(second, "second, revised"));
    let ours = sim.write(edit(first, "first, expanded").encode(), vec![shared]);
    let part = sim
        .vault
        .write(
            SEALING,
            Uuid::nil(),
            be_block::Part::from(&b"a closed segment"[..]),
        )
        .unwrap()
        .root
        .unwrap();
    PART.with(|held| held.set(Some(part)));
    (sim, ours, theirs)
}

#[test]
fn a_merge_by_parts_waits_for_the_parts_it_reads_and_never_resolves_without_them() {
    // A read that fails is tried again; the merge resolves once the part is read.
    let (mut sim, ours, theirs) = diverged();
    sim.fail_reads = 1;
    sim.open_at(0, Some(ours));
    assert_eq!(sim.part_reads, 2, "the part was read again after it failed");
    assert_eq!(READ.with(|read| read.borrow().clone()), b"a closed segment");
    assert_eq!(
        texts(&sim.head_content()),
        ["first, expanded", "second, revised"]
    );
    let head = sim.head.unwrap();
    assert_eq!(sim.commits[&head].parents, [theirs, ours]);
    assert!(sim.conflicts.is_empty());

    // A part that cannot be read leaves the merge unresolved, reported, and nothing published.
    let (mut sim, ours, theirs) = diverged();
    sim.fail_reads = 100;
    sim.open_at(0, Some(ours));
    assert_eq!(
        sim.head,
        Some(theirs),
        "the merge did not resolve without its part"
    );
    assert!(
        sim.errors
            .iter()
            .any(|(_, error)| error.contains("unreachable")),
        "the failure is reported: {:?}",
        sim.errors
    );
}
