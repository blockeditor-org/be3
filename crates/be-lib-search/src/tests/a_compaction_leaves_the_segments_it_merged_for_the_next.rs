use super::*;

/// The store, noting each part a compaction finished through it.
#[derive(Default)]
struct Noting {
    store: Store,
    finished: RefCell<Vec<PartName>>,
}

impl PartsOut for Noting {
    fn write(&self, part: &PartName, bytes: &[u8]) -> Result<(), Error> {
        self.store.write(part, bytes)
    }

    fn finish(&self, part: &PartName) -> Result<(), Error> {
        self.finished.borrow_mut().push(*part);
        self.store.finish(part)
    }
}

#[test]
fn a_compaction_leaves_the_segments_it_merged_for_the_next() {
    let mut out = Noting::default();
    let mut index = Index::default();
    // Three segments of four documents (a tier), and four of one (the tier below).
    let mut n = 0;
    for (batch, size) in [4, 4, 4, 1, 1, 1, 1].into_iter().enumerate() {
        let puts = (n..n + size).map(|n| put(n, 0, i64::from(n))).collect();
        n += size;
        apply(
            &mut index,
            &mut out.store,
            id(batch as u32),
            Batch {
                put: puts,
                ..Batch::default()
            },
        );
    }
    assert_eq!(index.segment_count(), 7);

    // The four small ones merge into a fourth of four; that one would make the tier above
    // whole, but a merge taking it would drop parts this pass sealed already.
    let written =
        run(index.compact(&out.store, &out, id(50), 0, usize::MAX, u64::MAX, |_| 0)).unwrap();
    out.store.keep(&written);
    assert_eq!(index.segment_count(), 4);
    let named = index.parts();
    for part in out.finished.borrow().iter() {
        assert!(named.contains(part), "{part:?} was written and dropped");
    }

    // The next pass merges it.
    let written =
        run(index.compact(&out.store, &out, id(51), 1, usize::MAX, u64::MAX, |_| 0)).unwrap();
    out.store.keep(&written);
    assert_eq!(index.segment_count(), 1);
    for n in 0..n {
        assert_eq!(
            found(&index, &out.store, &format!("uniq{n}v0x ")),
            [doc(n)].into()
        );
    }
}
