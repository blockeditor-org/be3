use super::*;

#[test]
fn block_values_sort_by_resolved_label_then_reference() {
    let first = Uuid::from_u128(1);
    let second = Uuid::from_u128(2);
    let blocks = HashMap::from([
        (
            first,
            BlockLabel {
                block_type: Uuid::nil(),
                icon: None,
                name: "Zulu".into(),
                automatic: false,
                reach: Default::default(),
                deleted: false,
            },
        ),
        (
            second,
            BlockLabel {
                block_type: Uuid::nil(),
                icon: None,
                name: "Alpha".into(),
                automatic: false,
                reach: Default::default(),
                deleted: false,
            },
        ),
    ]);
    let labels = BlockLabels {
        blocks: blocks.clone(),
        rows: Default::default(),
        unreachable: Default::default(),
    };
    assert_eq!(
        compare_database_values(
            &DatabaseValue::Block(first),
            &DatabaseValue::Block(second),
            &field(DatabaseFieldType::Block),
            &labels,
        ),
        Ordering::Greater
    );

    let tied = BlockLabels {
        blocks: blocks
            .into_iter()
            .map(|(reference, mut label)| {
                label.name = "Same".into();
                (reference, label)
            })
            .collect(),
        rows: Default::default(),
        unreachable: Default::default(),
    };
    assert_eq!(
        compare_database_values(
            &DatabaseValue::Block(first),
            &DatabaseValue::Block(second),
            &field(DatabaseFieldType::Block),
            &tied,
        ),
        Ordering::Less
    );
}
