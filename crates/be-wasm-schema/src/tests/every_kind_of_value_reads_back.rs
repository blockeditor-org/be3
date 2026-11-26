use super::*;

#[test]
fn every_kind_of_value_reads_back() {
    let everything = Everything {
        flag: true,
        small: 200,
        negative: -12345,
        huge: u128::MAX - 3,
        letter: 'é',
        ratio: 0.25,
        id: Id(42),
        key: [9, 8, 7, 6],
        chunks: vec![vec![], vec![1], vec![2, 3]],
        names: BTreeMap::from([("x".to_owned(), vec![255]), ("y".to_owned(), vec![])]),
        counts: HashMap::from([("a".to_owned(), 1), ("b".to_owned(), 0)]),
        seen: BTreeSet::from([3, 1, 2]),
        tags: HashSet::from(["red".to_owned(), "blue".to_owned()]),
        pair: (1, "one".into()),
        triple: (true, -2, None),
        pairs: Pair {
            first: "a".into(),
            second: "b".into(),
        },
        shape: ShapeV2::Square {
            side: -1.0,
            label: "s".into(),
        },
        boxed: Box::new(PersonV1 {
            name: String::new(),
            age: 0,
        }),
        shared: std::sync::Arc::new("shared".into()),
        nothing: Nothing,
        unit: (),
        nonzero: std::num::NonZeroU32::new(9).unwrap(),
        waited: Duration::new(3, 250),
        uuid: Uuid::from_u128(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef),
        letters: vec!['a', 'ß', '🦀'],
        flags: vec![false, true],
        wide: vec![1, u128::MAX],
    };
    assert_eq!(decode::<Everything>(&bytes(&everything)), Ok(everything));
    let tree = chain(5);
    assert_eq!(decode::<Tree>(&bytes(&tree)), Ok(tree));
    assert_eq!(decode::<Id>(&bytes(&Id(300))), Ok(Id(300)));
    assert_eq!(
        decode::<Pair<u32>>(&bytes(&Pair {
            first: 1,
            second: 2
        })),
        Ok(Pair {
            first: 1,
            second: 2
        })
    );
    assert_eq!(
        kind(decode::<std::num::NonZeroU32>(&0u32.to_le_bytes())),
        ErrorKind::OutOfRange
    );
}
