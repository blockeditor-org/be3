use super::*;

#[test]
fn the_declared_layout_is_read_from_an_export() {
    let declared = Declared::of(["memory", "__be3_dylink_100_20_3_2"]).unwrap();
    assert_eq!(
        declared,
        Declared {
            memory_size: 100,
            memory_align: 16,
            table_size: 3,
            table_align: 2,
        }
    );
    assert_eq!(declared.pages(), 1);
    assert!(Declared::of(["__be3_dylink_1_2_3"]).is_err());
    assert!(Declared::of(["memory"]).is_err());
}
