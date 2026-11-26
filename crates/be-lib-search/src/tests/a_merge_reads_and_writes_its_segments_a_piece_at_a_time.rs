use std::cell::Cell;

use super::*;

/// The most bytes one read of a segment's part may answer, and one write may hand the sink.
const BOUND: u64 = 16 << 10;

/// The store, refusing a read of a segment's part longer than `BOUND`.
struct Bounded<'a> {
    store: &'a Store,
    most: Cell<u64>,
}

impl Parts for Bounded<'_> {
    fn read<'a>(
        &'a self,
        part: &'a PartName,
        range: Range<u64>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + 'a>> {
        Box::pin(async move {
            let bytes = self.store.read(part, range).await?;
            // The table moves as an edit holds it, apart from the segments.
            if !matches!(part.kind, PartKind::Table(_)) {
                self.most.set(self.most.get().max(bytes.len() as u64));
                if bytes.len() as u64 > BOUND {
                    return Err(Error::Read(format!("{} bytes of {part:?}", bytes.len())));
                }
            }
            Ok(bytes)
        })
    }
}

/// Parts in memory, noting the longest write.
#[derive(Default)]
struct Sink {
    parts: MemoryParts,
    most: Cell<u64>,
}

impl PartsOut for Sink {
    fn write(&self, part: &PartName, bytes: &[u8]) -> Result<(), Error> {
        self.most.set(self.most.get().max(bytes.len() as u64));
        self.parts.write(part, bytes)
    }

    fn finish(&self, part: &PartName) -> Result<(), Error> {
        self.parts.finish(part)
    }
}

#[test]
fn a_merge_reads_and_writes_its_segments_a_piece_at_a_time() {
    let mut store = Store::default();
    let mut index = Index::default();
    let mut documents = BTreeMap::new();
    for batch in 0..4u32 {
        let time = i64::from(batch);
        let numbers = batch * 500..(batch + 1) * 500;
        for n in numbers.clone() {
            documents.insert(n, (0, time));
        }
        let puts = Batch {
            put: numbers.map(|n| put(n, 0, time)).collect(),
            ..Batch::default()
        };
        apply(&mut index, &mut store, id(batch), puts);
    }
    assert_eq!(index.segment_count(), 4);
    let inputs: u64 = index
        .parts()
        .iter()
        .filter(|name| !matches!(name.kind, PartKind::Table(_)))
        .map(|name| store.parts.parts.borrow()[name].len() as u64)
        .sum();
    assert!(inputs > 20 * BOUND, "the inputs are only {inputs} bytes");

    let bounded = Bounded {
        store: &store,
        most: Cell::new(0),
    };
    let sink = Sink::default();
    let written =
        run(index.compact(&bounded, &sink, id(50), 0, usize::MAX, u64::MAX, |_| 0)).unwrap();
    assert_eq!(index.segment_count(), 1);
    assert!(
        bounded.most.get() <= BOUND,
        "read {} bytes at once",
        bounded.most.get()
    );
    assert!(
        sink.most.get() <= BOUND,
        "wrote {} bytes at once",
        sink.most.get()
    );
    assert_eq!(written.saved, index.segments[0].parts());

    // What one batch of the same documents builds, byte for byte.
    let mut scratch_store = Store::default();
    let scratch = from_scratch(&documents, &mut scratch_store);
    let (merged, built) = (&index.segments[0], &scratch.segments[0]);
    assert_eq!(
        crate::segment::Meta {
            id: built.id,
            ..merged.clone()
        },
        *built
    );
    for kind in [
        PartKind::Dictionary,
        PartKind::Postings,
        PartKind::Positions,
        PartKind::Documents,
    ] {
        assert!(
            sink.parts.parts.borrow()[&merged.part(kind)]
                == scratch_store.parts.parts.borrow()[&built.part(kind)],
            "{kind:?} differs"
        );
    }
    store.keep(&written);
    for (name, bytes) in sink.parts.parts.take() {
        store.parts.parts.get_mut().insert(name, bytes);
    }
    same_answers((&index, &store), (&scratch, &scratch_store), 2000);
}
