use super::*;

#[test]
fn every_kind_of_value_reads_back() {
    let mut counts = HashMap::new();
    counts.insert("a".to_owned(), 1);
    counts.insert("b".to_owned(), 0);
    let everything = Everything {
        flag: true,
        small: 200,
        negative: -12345,
        letter: 'é',
        ratio: 0.25,
        id: Id(42),
        key: [9, 8, 7, 6],
        chunks: vec![vec![], vec![1], vec![2, 3]],
        names: BTreeMap::from([("x".to_owned(), vec![255]), ("y".to_owned(), vec![])]),
        counts,
        shape: ShapeV2::Square {
            side: -1.0,
            label: "s".into(),
        },
        boxed: Box::new(PersonV1 {
            name: String::new(),
            age: 0,
        }),
        nothing: Nothing,
    };
    assert_eq!(decode::<Everything>(&encode(&everything)), Ok(everything));
    let tree = Tree {
        value: 1,
        children: vec![
            Tree {
                value: 2,
                children: Vec::new(),
            },
            Tree {
                value: 0,
                children: vec![Tree {
                    value: -3,
                    children: Vec::new(),
                }],
            },
        ],
    };
    assert_eq!(decode::<Tree>(&encode(&tree)), Ok(tree));
    assert_eq!(decode::<Id>(&encode(&Id(300))), Ok(Id(300)));
    assert_eq!(encode(&-1i32), [1]);
}
