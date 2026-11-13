use super::*;

const TREES: &[&str] = &["ash", "birch", "cedar", "elm", "fir", "oak", "pine", "yew"];

struct Random(u32);

impl Random {
    fn below(&mut self, n: u32) -> u32 {
        self.0 = self.0.wrapping_mul(1103515245).wrapping_add(12345);
        (self.0 >> 16) % n
    }

    fn words(&mut self, most: u32) -> String {
        let count = self.below(most + 1);
        (0..count)
            .map(|_| TREES[self.below(TREES.len() as u32) as usize])
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn put(&mut self, n: u32, time: i64) -> Put {
        Put {
            doc: doc(n),
            commit: commit(time),
            source: None,
            text: Some(Text {
                name: self.words(3),
                body: self.words(25),
            }),
            partial: false,
        }
    }
}

fn batches(
    random: &mut Random,
    index: &mut Index,
    store: &mut Store,
    first: u32,
    count: u32,
    origin: u32,
) {
    for batch in 0..count {
        let time = i64::from(first + batch) * 10;
        let start = (first + batch) * 30;
        let mut put: Vec<Put> = (start..start + 30)
            .map(|n| {
                let offset = i64::from(random.below(5));
                random.put(n, time + offset)
            })
            .collect();
        let mut removed = Vec::new();
        if start > 0 {
            for _ in 0..8 {
                let n = random.below(start);
                put.push(random.put(n, time + 5));
            }
            removed.push((doc(random.below(start)), commit(time + 6)));
        }
        apply(
            index,
            store,
            id(origin + batch),
            Batch {
                put,
                removed,
                ..Batch::default()
            },
        );
    }
}

fn check(index: &Index, store: &Store) {
    let mut queries: Vec<String> = Vec::new();
    for (at, first) in TREES.iter().enumerate() {
        queries.push(format!("{first} "));
        queries.push(format!("\"{first} {}\"", TREES[(at + 1) % TREES.len()]));
        for second in &TREES[at + 1..] {
            queries.push(format!("{first} {second} "));
        }
    }
    for query in &queries {
        for limit in [1, 3, 10, 40] {
            agrees_with_brute_force(index, store, query, limit);
        }
    }
}

#[test]
fn ranked_results_match_a_brute_force_scorer() {
    let mut random = Random(7);
    let mut store = Store::default();
    let mut index = Index::default();
    batches(&mut random, &mut index, &mut store, 0, 8, 0);
    assert_eq!(index.segment_count(), 8);
    // Reindexing masks enough postings that a term's frequency can pass the
    // live document count.
    assert!(index.segments.iter().any(|meta| meta.deleted > 0));
    check(&index, &store);

    let mut compacted = index.clone();
    store.keep(&run(compacted.compact(&store, id(100))).unwrap());
    assert!(compacted.segment_count() < index.segment_count());
    check(&compacted, &store);

    let mut a = index.clone();
    batches(&mut random, &mut a, &mut store, 8, 3, 200);
    store.keep(&run(a.compact(&store, id(250))).unwrap());
    let mut b = index.clone();
    batches(&mut random, &mut b, &mut store, 11, 2, 300);
    let (merged, written) = run(a.merge(&b, &store, id(400))).unwrap();
    store.keep(&written);
    check(&merged, &store);
}
